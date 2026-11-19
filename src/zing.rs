//! Zing MP3 module (v0.1): a client for the Zing web client's internal API
//! (`zingmp3.vn/api/v2/...`), the same API the official Zing desktop app uses.
//!
//! Signing (verified live against the service): fetch the homepage once and
//! keep its cookies; every call then carries `ctime` (unix seconds), `version`
//! and `apiKey`, plus `sig = HMAC-SHA512(SECRET, path + SHA256_HEX(hash_input))`
//! where `hash_input` is a per-endpoint template. The `ctime` inside
//! `hash_input` must be the very value sent as the `ctime` parameter, or the
//! service answers err -104 "Incorect signature".
//!
//! The API key/secret below are the web client's own, as embedded in the
//! public open-source libraries that implement this client (Whoant/ZingMp3API,
//! zatamod/zingmp3). Streaming may be geo- or account-restricted: the service
//! answers err -1110 for songs this region/account may not play, and that
//! message is surfaced to the user as the play error.

use std::collections::HashSet;
use std::sync::Arc;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256, Sha512};

use crate::model::{Link, Thumb, Track};
use crate::catalog::SongShelf;

const BASE: &str = "https://zingmp3.vn";
const VERSION: &str = "1.6.34";
const API_KEY: Option<&str> = option_env!("YOZINU_ZING_API_KEY");
const SECRET_KEY: Option<&str> = option_env!("YOZINU_ZING_SECRET_KEY");
pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

type HmacSha512 = Hmac<Sha512>;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_hex(s: &str) -> String {
    hex(&Sha256::digest(s.as_bytes()))
}

fn sig(path: &str, hash_input: &str, secret: &str) -> String {
    let mut mac = HmacSha512::new_from_slice(secret.as_bytes()).expect("an HMAC key of any size works");
    mac.update(format!("{path}{}", sha256_hex(hash_input)).as_bytes());
    hex(&mac.finalize().into_bytes())
}

/// The hash-input template shared by every `id` endpoint.
fn hash_id(id: &str) -> String {
    format!("ctime={{ctime}}id={id}version={VERSION}")
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    /// The homepage cookies (`zmp3_app_version.1`, `zmp3_rqid`), as one Cookie header.
    cookies: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl Client {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http, cookies: Arc::new(tokio::sync::Mutex::new(None)) }
    }

    async fn ensure_cookies(&self) -> anyhow::Result<String> {
        if let Some(c) = self.cookies.lock().await.clone() {
            return Ok(c);
        }
        let resp = self.http.get(format!("{BASE}/")).header("User-Agent", UA).send().await?.error_for_status()?;
        let mut pairs = Vec::new();
        for v in resp.headers().get_all(reqwest::header::SET_COOKIE).iter() {
            if let Ok(s) = v.to_str()
                && let Some(kv) = s.split(';').next()
            {
                pairs.push(kv.trim().to_owned());
            }
        }
        let _ = resp.bytes().await;
        anyhow::ensure!(!pairs.is_empty(), "Zing sent no cookies");
        let joined = pairs.join("; ");
        *self.cookies.lock().await = Some(joined.clone());
        Ok(joined)
    }

    /// One signed API call; returns the envelope's `data`. A signature error
    /// (stale cookies) is retried once with fresh cookies.
    async fn call(&self, path: &str, params: &[(&str, String)], hash_tpl: &str) -> anyhow::Result<serde_json::Value> {
        let key = API_KEY.ok_or_else(|| anyhow::anyhow!("Zing MP3 build configuration is missing"))?;
        let secret = SECRET_KEY.ok_or_else(|| anyhow::anyhow!("Zing MP3 build configuration is missing"))?;
        for attempt in 0..2 {
            let cookies = self.ensure_cookies().await?;
            let ctime = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0).to_string();
            let hash_input = hash_tpl.replace("{ctime}", &ctime);
            let mut query: Vec<(&str, String)> = params.to_vec();
            query.push(("ctime", ctime));
            query.push(("version", VERSION.to_owned()));
            query.push(("apiKey", key.to_owned()));
            query.push(("sig", sig(path, &hash_input, secret)));
            let resp = self.http.get(format!("{BASE}{path}")).query(&query).header("User-Agent", UA).header("Referer", format!("{BASE}/")).header("Cookie", cookies).send().await?;
            let v: serde_json::Value = resp.json().await.map_err(|e| anyhow::anyhow!("Zing answered unreadably: {e}"))?;
            let err = v.get("err").and_then(serde_json::Value::as_i64).unwrap_or(-1);
            if err == 0 {
                return Ok(v.get("data").cloned().unwrap_or(serde_json::Value::Null));
            }
            let msg = v.get("msg").and_then(serde_json::Value::as_str).unwrap_or("").to_owned();
            if err == -104 && attempt == 0 {
                *self.cookies.lock().await = None;
                continue;
            }
            anyhow::bail!("Zing error {err}: {msg}");
        }
        unreachable!("the retry loop always returns")
    }

    /// Discovery shelves and weekly charts. Either feed can still show on its own.
    pub async fn home(&self) -> anyhow::Result<Vec<SongShelf>> {
        let limit = std::time::Duration::from_secs(6);
        let chart_signature = format!("ctime={{ctime}}version={VERSION}");
        let (chart, discovery) = tokio::join!(
            tokio::time::timeout(limit, self.call("/api/v2/page/get/chart-home", &[], &chart_signature)),
            tokio::time::timeout(limit, self.discovery()),
        );
        let mut shelves = discovery.ok().and_then(Result::ok).unwrap_or_default();
        if let Ok(Ok(data)) = chart {
            if !shelves.iter().any(|s| s.title.starts_with("Mới phát hành")) {
                push_shelf(&mut shelves, "Mới phát hành", &data["newRelease"]);
            }
            for (title, items) in [
                ("#zingchart", &data["RTChart"]["items"]),
                ("Nhạc Việt trong tuần", &data["weekChart"]["vn"]["items"]),
                ("Nhạc Âu Mỹ trong tuần", &data["weekChart"]["us"]["items"]),
                ("Nhạc Hàn trong tuần", &data["weekChart"]["korea"]["items"]),
            ] {
                push_shelf(&mut shelves, title, items);
            }
        }
        anyhow::ensure!(!shelves.is_empty(), "Zing returned no songs for the home page");
        Ok(shelves)
    }

    async fn discovery(&self) -> anyhow::Result<Vec<SongShelf>> {
        let data = self.call("/api/v2/page/get/home", &[("page", "1".to_owned()), ("count", "24".to_owned())], &format!("count=24ctime={{ctime}}page=1version={VERSION}")).await?;
        let sections = data["items"].as_array().map(Vec::as_slice).unwrap_or_default();
        let mut shelves = Vec::new();
        let mut featured = Vec::new();
        for section in sections {
            match section["sectionType"].as_str() {
                Some("new-release") => {
                    for (key, title) in [("vPop", "Mới phát hành · Nhạc Việt"), ("others", "Mới phát hành · Quốc tế"), ("all", "Mới phát hành")] {
                        if key == "all" && !shelves.is_empty() {
                            continue;
                        }
                        push_shelf(&mut shelves, title, &section["items"][key]);
                    }
                }
                Some("newReleaseChart") => push_shelf(&mut shelves, "BXH Nhạc Mới", &section["items"]),
                Some("playlist") if matches!(section["sectionId"].as_str(), Some("h100" | "hEditorTheme")) => {
                    if let Some((id, playlist_title)) = section["items"].as_array().and_then(|items| items.iter().find_map(|entry| Some((entry["encodeId"].as_str()?, entry["title"].as_str()?)))) {
                        let title = section["title"].as_str().unwrap_or("Playlist nổi bật");
                        featured.push((id, format!("{title} · {playlist_title}")));
                    }
                }
                _ => {}
            }
        }
        let jobs = futures_util::future::join_all(featured.into_iter().map(|(id, title)| async move {
            (title, self.playlist(id).await)
        })).await;
        for (title, result) in jobs {
            if let Ok(data) = result {
                push_shelf(&mut shelves, &title, &data["song"]["items"]);
            }
        }
        Ok(shelves)
    }

    async fn playlist(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        self.call("/api/v2/page/get/playlist", &[("id", id.to_owned())], &hash_id(id)).await
    }

    pub async fn lyrics(&self, id: &str) -> anyhow::Result<Option<crate::lyrics::Lyrics>> {
        let data = self.call("/api/v2/lyric/get/lyric", &[("id", id.to_owned())], &hash_id(id)).await?;
        Ok(crate::lyrics::zing_words(&data["sentences"]))
    }

    pub async fn song(&self, id: &str) -> anyhow::Result<Track> {
        let data = self.call("/api/v2/song/get/info", &[("id", id.to_owned())], &hash_id(id)).await?;
        track_from(&data).ok_or_else(|| anyhow::anyhow!("No Zing MP3 song"))
    }

    /// Songs matching `q`, best first.
    pub async fn search(&self, q: &str) -> anyhow::Result<Vec<Track>> {
        let data = self.call("/api/v2/search/multi", &[("q", q.to_owned())], &format!("ctime={{ctime}}version={VERSION}")).await?;
        let songs = data.get("songs").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        Ok(songs.iter().filter_map(track_from).collect())
    }

    /// A playable stream URL for a song (bare encodeId). Zing's own error
    /// (geo/VIP, err -1110 and friends) is the error returned.
    pub async fn streaming(&self, id: &str) -> anyhow::Result<String> {
        let data = self.call("/api/v2/song/get/streaming", &[("id", id.to_owned())], &hash_id(id)).await?;
        let obj = data.as_object().ok_or_else(|| anyhow::anyhow!("Zing streaming: unexpected answer"))?;
        for key in ["128", "320"] {
            if let Some(u) = obj.get(key).and_then(serde_json::Value::as_str).filter(|u| !u.is_empty()) {
                return Ok(u.to_owned());
            }
        }
        for v in obj.values() {
            if let Some(u) = v.as_str().filter(|u| !u.is_empty()) {
                return Ok(u.to_owned());
            }
        }
        anyhow::bail!("Zing streaming: no playable URL returned")
    }

    /// Songs similar to the seed (bare encodeId), for autoplay: the chain the
    /// official desktop app follows — the song's section-bottom playlists,
    /// then the first playlist's songs. Best-effort: any failure is an empty
    /// list, never an error.
    pub async fn autoplay(&self, seed: &str) -> Vec<Track> {
        let Ok(sections) = self.call("/api/v2/song/get/section-bottom", &[("id", seed.to_owned())], &hash_id(seed)).await else { return Vec::new() };
        let mut playlist_id = None;
        if let Some(sections) = sections.as_array() {
            for sec in sections {
                if let Some(items) = sec.get("items").and_then(serde_json::Value::as_array)
                    && let Some(first) = items.first()
                    && let Some(pid) = first.get("encodeId").and_then(serde_json::Value::as_str)
                {
                    playlist_id = Some(pid.to_owned());
                    break;
                }
            }
        }
        let Some(pid) = playlist_id else { return Vec::new() };
        let Ok(data) = self.playlist(&pid).await else { return Vec::new() };
        let items = data.get("song").and_then(|s| s.get("items")).and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        let mut seen = HashSet::new();
        items.iter().filter_map(track_from).filter(|t| t.id != format!("zing:{seed}") && seen.insert(t.id.clone())).collect()
    }
}

fn push_shelf(shelves: &mut Vec<SongShelf>, title: &str, items: &serde_json::Value) {
    let mut seen = HashSet::new();
    let tracks: Vec<_> = items.as_array().into_iter().flatten()
        .filter(|item| item["streamingStatus"].as_u64().is_none_or(|status| status == 1))
        .filter_map(track_from)
        .filter(|track| seen.insert(track.id.clone()))
        .take(24)
        .collect();
    if !tracks.is_empty() {
        shelves.push(SongShelf { title: title.to_owned(), tracks });
    }
}

/// One Zing song object (search / playlist shape) as a `Track` whose id is
/// namespaced `zing:<encodeId>`, so it can never collide with a YouTube id.
fn track_from(v: &serde_json::Value) -> Option<Track> {
    let id = v.get("encodeId")?.as_str().filter(|id| !id.is_empty())?;
    let title = v.get("title")?.as_str().filter(|title| !title.trim().is_empty())?.to_owned();
    let mut artists: Vec<Link> = v
        .get("artists")
        .and_then(serde_json::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    let name = x.get("name")?.as_str()?;
                    let id = x.get("id").and_then(serde_json::Value::as_str).or_else(|| x.get("alias").and_then(serde_json::Value::as_str)).map(str::to_owned);
                    Some(Link { name: name.to_owned(), id })
                })
                .collect()
        })
        .unwrap_or_default();
    if artists.is_empty()
        && let Some(names) = v.get("artistsNames").and_then(serde_json::Value::as_str)
    {
        artists.push(Link { name: names.to_owned(), id: None });
    }
    let album = v.get("album").filter(|a| !a.is_null()).and_then(|a| {
        let name = a.get("title")?.as_str()?;
        Some(Link { name: name.to_owned(), id: a.get("encodeId").and_then(serde_json::Value::as_str).map(str::to_owned) })
    });
    let mut thumbs = Vec::new();
    if let Some(u) = v.get("thumbnailM").and_then(serde_json::Value::as_str).filter(|u| !u.is_empty()) {
        thumbs.push(Thumb { url: u.to_owned(), width: 512, height: 512 });
    }
    if let Some(u) = v.get("thumbnail").and_then(serde_json::Value::as_str).filter(|u| !u.is_empty()) {
        thumbs.push(Thumb { url: u.to_owned(), width: 320, height: 320 });
    }
    Some(Track { id: format!("zing:{id}"), title, artists, album, duration: v.get("duration").and_then(serde_json::Value::as_u64).map(|d| d as u32), thumbs, ..Default::default() })
}
