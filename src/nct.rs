//! NhacCuaTui guest catalog. The `/api/v1` routes were verified without
//! account credentials, signed requests or app-specific headers.

use std::collections::HashSet;

use anyhow::{Context, bail, ensure};
use serde_json::Value;

use crate::catalog::SongShelf;
use crate::model::{Link, Thumb, Track};

const BASE: &str = "https://graph.nct.vn/api/v1/";
pub const UA: &str = "YoZiNu/0.1";

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
}

impl Client {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    async fn get(&self, endpoint: &str, key: Option<&str>, query: &[(&str, &str)]) -> anyhow::Result<Value> {
        let mut url = url::Url::parse(BASE)?.join(endpoint)?;
        if let Some(key) = key {
            ensure!(key.len() <= 80 && !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'), "Mã bài hoặc playlist NhacCuaTui không hợp lệ");
            url.path_segments_mut().map_err(|_| anyhow::anyhow!("Invalid NhacCuaTui endpoint"))?.push(key);
        }
        let envelope = self.http.get(url)
            .header("User-Agent", UA)
            .header("Accept", "application/json")
            .query(query)
            .send().await
            .and_then(reqwest::Response::error_for_status)
            .map_err(reqwest::Error::without_url)?
            .json::<Value>().await.map_err(reqwest::Error::without_url)
            .context("Không đọc được phản hồi NhacCuaTui")?;
        let code = envelope["code"].as_i64().unwrap_or(-1);
        ensure!(code == 0, "NhacCuaTui trả lỗi {code}");
        Ok(envelope.get("data").cloned().unwrap_or_default())
    }

    pub async fn song(&self, key: &str) -> anyhow::Result<Track> {
        let item = self.get("song/detail", Some(key), &[]).await?;
        tracks(&Value::Array(vec![item]), None, false).pop().context("No NhacCuaTui song")
    }

    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<Track>> {
        let data = self.get("search/song", None, &[("keyword", query), ("pageindex", "1"), ("pagesize", "24"), ("correct", "true")]).await?;
        Ok(tracks(&data["songs"], None, false))
    }

    pub async fn home(&self) -> anyhow::Result<Vec<SongShelf>> {
        let limit = std::time::Duration::from_secs(7);
        let (charts, recommendations) = tokio::join!(
            tokio::time::timeout(limit, self.get("playlist/charts", None, &[])),
            tokio::time::timeout(limit, self.recommendations()),
        );
        let mut shelves = Vec::new();
        if let Ok(Ok(data)) = charts {
            for chart in data.as_array().into_iter().flatten().take(4) {
                let Some(title) = chart["title"].as_str() else { continue };
                let tracks = tracks(&chart["items"], None, true);
                if !tracks.is_empty() {
                    shelves.push(SongShelf { title: title.to_owned(), tracks });
                }
            }
        }
        if let Ok(Ok(recommendations)) = recommendations {
            shelves.extend(recommendations);
        }
        ensure!(!shelves.is_empty(), "NhacCuaTui chưa trả về danh sách bài hát");
        Ok(shelves)
    }

    async fn recommendations(&self) -> anyhow::Result<Vec<SongShelf>> {
        let data = self.get("app/home/recommend", None, &[]).await?;
        let entries = data["child"].as_array().into_iter().flatten()
            .filter(|item| item["type"] == "playlist").take(6);
        let shelves = futures_util::future::join_all(entries.map(|entry| async move {
            let key = entry["key"].as_str()?;
            let playlist = self.get("playlist/detail", Some(key), &[]).await.ok()?;
            let title = playlist["name"].as_str().or_else(|| entry["name"].as_str())?;
            let tracks = tracks(&playlist["listSong"], None, true);
            (!tracks.is_empty()).then(|| SongShelf { title: title.to_owned(), tracks })
        })).await;
        Ok(shelves.into_iter().flatten().collect())
    }

    pub async fn similar(&self, key: &str) -> anyhow::Result<Vec<Track>> {
        let item = self.get("song/similar", Some(key), &[("pageindex", "1"), ("pagesize", "16")]).await?;
        Ok(tracks(&item["list"], Some(key), true))
    }

    /// Stream URLs expire. Resolve a fresh one and honor the restrictions in the
    /// response each time a track plays.
    pub async fn streaming(&self, key: &str) -> anyhow::Result<String> {
        let item = self.get("song/detail", Some(key), &[]).await?;
        ensure!(can_select(&item), "NhacCuaTui giới hạn phát bài này");
        if let Some(url) = guest_stream(&item) {
            return Ok(url.to_owned());
        }
        if item["streamURL"].as_array().is_some_and(|urls| urls.iter().any(|url| url["onlyVIP"].as_bool() == Some(true))) {
            bail!("Bài này yêu cầu tài khoản NhacCuaTui VIP");
        }
        bail!("NhacCuaTui chưa cấp luồng phát cho bài này")
    }
}

fn can_select(item: &Value) -> bool {
    item["statusPlay"].as_i64() == Some(1) && item["forceShuffle"].as_bool() != Some(true)
}

fn guest_stream(item: &Value) -> Option<&str> {
    let urls = item["streamURL"].as_array()?;
    ["128", "320"].into_iter().find_map(|quality| {
        urls.iter()
            .filter(|s| s["type"].as_str() == Some(quality) && s["onlyVIP"].as_bool() == Some(false))
            .filter_map(|s| s["stream"].as_str())
            .find(|s| url::Url::parse(s).is_ok_and(|u| u.scheme() == "https" && u.host_str().is_some() && u.username().is_empty() && u.password().is_none()))
    })
}

fn tracks(items: &Value, seed: Option<&str>, guest_only: bool) -> Vec<Track> {
    let mut seen = HashSet::new();
    items.as_array().into_iter().flatten().filter_map(|v| {
        let key = v["key"].as_str().filter(|key| !key.is_empty())?;
        let title = v["name"].as_str().filter(|name| !name.trim().is_empty())?;
        if Some(key) == seed || !seen.insert(key) || (guest_only && (!can_select(v) || guest_stream(v).is_none())) {
            return None;
        }
        let artists = v["artistName"].as_str().filter(|name| !name.is_empty())
            .map(|name| vec![Link { name: name.to_owned(), id: None }]).unwrap_or_default();
        let thumbs = v["image"].as_str().filter(|url| url.starts_with("https://"))
            .map(|url| vec![Thumb { url: url.to_owned(), width: 400, height: 400 }]).unwrap_or_default();
        let duration = v["duration"].as_u64().and_then(|value| u32::try_from(value).ok()).filter(|d| *d > 0);
        Some(Track { id: format!("nct:{key}"), title: title.to_owned(), artists, thumbs, duration, ..Track::default() })
    }).take(24).collect()
}
