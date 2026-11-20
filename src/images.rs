//! egui loaders for `https://` thumbnails: the download (on the backend runtime), and
//! the textures made from them, kept under a memory budget.
//!
//! Reusing the backend's reqwest client avoids a second HTTP/TLS stack in the binary.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, ImagePoll, LoadError, SizeHint, SizedTexture, TextureLoadResult, TextureLoader, TexturePoll};
use egui::{TextureHandle, TextureOptions, Vec2};

use crate::backend::Backend;

/// A failed download is tried again when it's next needed after this long.
const RETRY_AFTER: Duration = Duration::from_secs(30);
/// Thumbnails up to this size are also kept on disk, so the next launch shows them at
/// once (the account picture, playlist covers, the home feed).
const DISK_MAX_FILE: usize = 256 * 1024;
/// The disk cache is trimmed to this, oldest first, at launch.
const DISK_BUDGET: u64 = 64 * 1024 * 1024;
/// A page full of covers doesn't ask for them all at once.
const MAX_DOWNLOADS: usize = 8;
/// Downloaded pictures not made into textures yet, kept in memory.
const BYTES_BUDGET: usize = 32 * 1024 * 1024;

enum Entry {
    Pending,
    Ready(Arc<[u8]>, Option<String>),
    Failed(String, Instant),
}

fn is_http(uri: &str) -> bool {
    uri.starts_with("https://") || uri.starts_with("http://")
}

pub struct HttpImages {
    backend: Backend,
    cache: Arc<Mutex<HashMap<String, Entry>>>,
    dir: std::path::PathBuf,
    /// Downloads wait for one of these.
    turns: Arc<tokio::sync::Semaphore>,
}

impl HttpImages {
    pub fn new(backend: Backend) -> Self {
        let dir = backend.data_dir().join("thumbs");
        let _ = std::fs::create_dir_all(&dir);
        trim(&dir);
        Self { backend, cache: Default::default(), dir, turns: Arc::new(tokio::sync::Semaphore::new(MAX_DOWNLOADS)) }
    }
}

/// A download's outcome, unless `uri` was forgotten while in flight. Pictures that never
/// become a texture (one that can't be decoded) would otherwise stay: past the budget,
/// others go. One still wanted just comes again, mostly from the disk.
fn store(cache: &Mutex<HashMap<String, Entry>>, uri: &str, entry: Entry) {
    let mut cache = cache.lock().unwrap();
    let Some(slot) = cache.get_mut(uri) else { return };
    *slot = entry;
    let mut total: usize = cache.values().map(|e| if let Entry::Ready(b, _) = e { b.len() } else { 0 }).sum();
    if total <= BYTES_BUDGET {
        return;
    }
    cache.retain(|k, e| {
        let Entry::Ready(b, _) = e else { return true };
        if total <= BYTES_BUDGET || k == uri {
            return true;
        }
        total -= b.len();
        false
    });
}

/// Where `uri` is kept on disk. Thumbnail URLs change when the picture does.
fn disk_path(dir: &std::path::Path, uri: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    uri.hash(&mut h);
    dir.join(format!("{:016x}", h.finish()))
}

/// Keep the disk cache under its budget, dropping the least recently used files.
fn trim(dir: &std::path::Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<_> = rd
        .flatten()
        .filter_map(|f| {
            let m = f.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), f.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort();
    for (_, len, path) in files {
        if total <= DISK_BUDGET {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            total -= len;
        }
    }
}

impl BytesLoader for HttpImages {
    fn id(&self) -> &str {
        egui::generate_loader_id!(HttpImages)
    }

    fn load(&self, _ctx: &egui::Context, uri: &str) -> BytesLoadResult {
        if !is_http(uri) {
            return Err(LoadError::NotSupported);
        }
        let mut cache = self.cache.lock().unwrap();
        match cache.get(uri) {
            Some(Entry::Ready(bytes, mime)) => {
                return Ok(BytesPoll::Ready {
                    size: None,
                    bytes: Bytes::Shared(bytes.clone()),
                    mime: mime.clone(),
                });
            }
            Some(Entry::Pending) => return Ok(BytesPoll::Pending { size: None }),
            Some(Entry::Failed(e, at)) if at.elapsed() < RETRY_AFTER => return Err(LoadError::Loading(e.clone())),
            Some(Entry::Failed(..)) | None => {}
        }
        cache.insert(uri.to_owned(), Entry::Pending);
        drop(cache);

        let (http, cache, uri, backend) =
            (self.backend.http(), self.cache.clone(), uri.to_owned(), self.backend.clone());
        let turns = self.turns.clone();
        let file = disk_path(&self.dir, &uri);
        self.backend.spawn(async move {
            if let Ok(bytes) = std::fs::read(&file) {
                // Touch it, so trimming drops pictures that aren't used any more.
                let _ = std::fs::File::options().write(true).open(&file).and_then(|f| f.set_modified(std::time::SystemTime::now()));
                store(&cache, &uri, Entry::Ready(Arc::from(bytes), None));
                backend.wake();
                return;
            }
            let res = async {
                let _turn = turns.acquire().await;
                let resp = http.get(&uri).send().await?.error_for_status()?;
                let mime = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                Ok::<_, reqwest::Error>((resp.bytes().await?, mime))
            }
            .await;
            let entry = match res {
                Ok((bytes, mime)) => {
                    if bytes.len() <= DISK_MAX_FILE {
                        let _ = std::fs::write(&file, &bytes);
                    }
                    Entry::Ready(Arc::from(&bytes[..]), mime)
                }
                Err(e) => Entry::Failed(e.to_string(), Instant::now()),
            };
            store(&cache, &uri, entry);
            backend.wake();
        });
        Ok(BytesPoll::Pending { size: None })
    }

    fn forget(&self, uri: &str) {
        self.cache.lock().unwrap().remove(uri);
    }

    fn forget_all(&self) {
        self.cache.lock().unwrap().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .unwrap()
            .values()
            .map(|e| match e {
                Entry::Ready(b, _) => b.len(),
                _ => 0,
            })
            .sum()
    }

    fn has_pending(&self) -> bool {
        self.cache.lock().unwrap().values().any(|e| matches!(e, Entry::Pending))
    }
}

/// GPU memory the thumbnails may keep before the least recently drawn are dropped.
const TEXTURE_BUDGET: usize = 64 * 1024 * 1024;
/// Feather Lite's cover cache: this many cover textures at most, the least recently drawn
/// let go as soon as one more arrives (see [`set_lite`]). A song's cover is a few hundred
/// kilobytes at the sizes its rows ask for, so the whole cache stays small.
const LITE_COVERS: usize = 48;
/// Whether the light layout is drawing: covers are then kept by count (above) rather than
/// by the byte budget, so browsing a long list can't grow the cache past it.
static LITE: AtomicBool = AtomicBool::new(false);
/// How many cover textures were let go since launch, and what the cache holds now.
static EVICTED: AtomicU64 = AtomicU64::new(0);
static HELD: AtomicU64 = AtomicU64::new(0);
static HELD_BYTES: AtomicU64 = AtomicU64::new(0);
/// How long Feather Lite keeps its small cover cache before dropping it by itself.
const LITE_CLEAR_EVERY: Duration = Duration::from_secs(5 * 60);
static LAST_LITE_CLEAR: Mutex<Option<Instant>> = Mutex::new(None);

/// Turn the cover cache's count limit on (Feather Lite) or off (every other layout).
pub fn set_lite(on: bool) {
    if !on {
        *LAST_LITE_CLEAR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    LITE.store(on, Ordering::Relaxed);
}

/// Called each frame while Feather Lite draws: its cache is dropped every five minutes,
/// without a button. Leaving the layout restarts the interval.
pub fn tick_lite(ctx: &egui::Context) {
    if !LITE.load(Ordering::Relaxed) {
        return;
    }
    let now = Instant::now();
    let mut last = LAST_LITE_CLEAR.lock().unwrap_or_else(|e| e.into_inner());
    if last.map_or(true, |at| now.duration_since(at) >= LITE_CLEAR_EVERY) {
        clear_covers(ctx);
        *last = Some(now);
    }
}

/// Drop every picture the textures came from, and the textures themselves. Feather Lite
/// does this on its five-minute timer; they come back from the disk cache when drawn again.
pub fn clear_covers(ctx: &egui::Context) {
    let loaders = ctx.loaders();
    for l in loaders.texture.lock().iter() {
        l.forget_all();
    }
    for l in loaders.image.lock().iter() {
        l.forget_all();
    }
    for l in loaders.bytes.lock().iter() {
        l.forget_all();
    }
    HELD.store(0, Ordering::Relaxed);
    HELD_BYTES.store(0, Ordering::Relaxed);
}

/// A big picture (now playing's cover, a banner: 3.5 MB at 960 px) not drawn for this
/// long is dropped even under the budget: one per song played would otherwise pile up
/// until it. It comes back from the disk cache when it's next shown.
const BIG: usize = 1024 * 1024;
const BIG_UNUSED: Duration = Duration::from_secs(30);

struct Texture {
    handle: TextureHandle,
    options: TextureOptions,
    size: Vec2,
    last_used: u64,
    /// When it was last drawn.
    drawn: Instant,
}

#[derive(Default)]
struct TextureState {
    pass: u64,
    cache: HashMap<String, Texture>,
}

/// Texture loader for `https://` images (and bundled previews). Unlike egui's default one it drops the
/// downloaded and decoded copies once the texture exists (only the GPU copy is
/// needed), and it keeps the textures under [`TEXTURE_BUDGET`] by evicting the ones
/// not drawn for longest, so browsing for hours doesn't grow without bound and nothing
/// on screen is ever thrown away.
#[derive(Default)]
pub struct Textures {
    state: Mutex<TextureState>,
}

impl TextureLoader for Textures {
    fn id(&self) -> &str {
        egui::generate_loader_id!(Textures)
    }

    fn load(&self, ctx: &egui::Context, uri: &str, options: TextureOptions, size_hint: SizeHint) -> TextureLoadResult {
        // Also the bundled theme covers and icon previews: egui's own loader would keep
        // each one decoded, as well as its texture, until the app closes.
        if !is_http(uri) && !uri.starts_with("bytes://theme-cover/") && !uri.starts_with("bytes://app-icon/") {
            return Err(LoadError::NotSupported);
        }
        {
            let mut state = self.state.lock().unwrap();
            let pass = state.pass;
            if let Some(t) = state.cache.get_mut(uri).filter(|t| t.options == options) {
                t.last_used = pass;
                t.drawn = Instant::now();
                return Ok(TexturePoll::Ready { texture: SizedTexture::new(t.handle.id(), t.size) });
            }
        }
        match ctx.try_load_image(uri, size_hint)? {
            ImagePoll::Pending { size } => Ok(TexturePoll::Pending { size }),
            ImagePoll::Ready { image } => {
                let size = image.source_size;
                let handle = ctx.load_texture(uri, image, options);
                let texture = SizedTexture::new(handle.id(), size);
                let loaders = ctx.loaders();
                for l in loaders.bytes.lock().iter() {
                    l.forget(uri);
                }
                for l in loaders.image.lock().iter() {
                    l.forget(uri);
                }
                let mut state = self.state.lock().unwrap();
                let last_used = state.pass;
                state.cache.insert(uri.to_owned(), Texture { handle, options, size, last_used, drawn: Instant::now() });
                Ok(TexturePoll::Ready { texture })
            }
        }
    }

    fn forget(&self, uri: &str) {
        self.state.lock().unwrap().cache.remove(uri);
    }

    fn forget_all(&self) {
        self.state.lock().unwrap().cache.clear();
    }

    fn end_pass(&self, pass_index: u64) {
        let mut state = self.state.lock().unwrap();
        state.pass = pass_index;
        state.cache.retain(|_, t| t.handle.byte_size() < BIG || t.drawn.elapsed() < BIG_UNUSED);
        // Feather Lite: covers are kept by count, not by bytes, so a long list browsed
        // through can't leave a cache behind it. What's drawn this pass is the newest.
        if LITE.load(Ordering::Relaxed) {
            let mut covers: Vec<(u64, String)> =
                state.cache.iter().filter(|(uri, _)| is_http(uri)).map(|(uri, t)| (t.last_used, uri.clone())).collect();
            let over = covers.len().saturating_sub(LITE_COVERS);
            if over > 0 {
                covers.sort_unstable();
                for (_, uri) in covers.into_iter().take(over) {
                    if state.cache.remove(&uri).is_some() {
                        EVICTED.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        let mut total = state.cache.values().map(|t| t.handle.byte_size()).sum::<usize>();
        HELD.store(state.cache.len() as u64, Ordering::Relaxed);
        HELD_BYTES.store(total as u64, Ordering::Relaxed);
        if total <= TEXTURE_BUDGET {
            return;
        }
        // Oldest first; anything drawn in the last couple of frames stays.
        let mut old: Vec<(u64, String)> = state.cache.iter().filter(|(_, t)| t.last_used + 2 < pass_index).map(|(k, t)| (t.last_used, k.clone())).collect();
        old.sort_unstable();
        for (_, uri) in old {
            if total <= TEXTURE_BUDGET * 3 / 4 {
                break;
            }
            if let Some(t) = state.cache.remove(&uri) {
                total -= t.handle.byte_size();
            }
        }
    }

    fn byte_size(&self) -> usize {
        self.state.lock().unwrap().cache.values().map(|t| t.handle.byte_size()).sum()
    }
}
