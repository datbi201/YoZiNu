//! The current mixed playlist, stored in the user's YoZiNu profile.

use std::{collections::HashSet, fs, io, path::Path, sync::{Arc, Mutex}};

use egui::{Color32, Rect, RichText, Sense, Ui, pos2, vec2};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::{catalog::Catalog, model::Track, theme::{self, icon}};
use super::{Action, App, Page, fmt_time, layouts::Choice, pages, widgets};

const FILE: &str = "mixed-playlist.json";
pub(super) fn title() -> &'static str { crate::i18n::label("Mixed playlist", "Playlist tổng hợp") }
pub(super) fn add_label() -> &'static str {
    crate::i18n::label("Add to mixed playlist", "Thêm vào playlist tổng hợp")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    YouTube,
    Zing,
    Nct,
}

impl Source {
    pub(super) const ALL: [Self; 3] = [Self::YouTube, Self::Zing, Self::Nct];

    pub(super) fn for_catalog(source: Catalog) -> Self {
        match source {
            Catalog::Zing => Self::Zing,
            Catalog::Nct => Self::Nct,
        }
    }

    fn for_track(track: &Track) -> Self {
        match Catalog::split_track(&track.id) {
            Some((Catalog::Zing, _)) => Self::Zing,
            Some((Catalog::Nct, _)) => Self::Nct,
            None => Self::YouTube,
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::YouTube => "YouTube Music",
            Self::Zing => "Zing MP3",
            Self::Nct => "NhacCuaTui",
        }
    }

    pub(super) fn search(self, query: String) -> Page {
        match self {
            Self::YouTube => Page::Search(query),
            Self::Zing => Page::CatalogSearch(Catalog::Zing, query),
            Self::Nct => Page::CatalogSearch(Catalog::Nct, query),
        }
    }

    fn badge(self, ui: &mut Ui) {
        let (label, color) = match self {
            Self::YouTube => ("YT", Color32::from_rgb(154, 30, 55)),
            Self::Zing => ("ZING", Color32::from_rgb(103, 50, 173)),
            Self::Nct => ("NCT", Color32::from_rgb(0, 104, 135)),
        };
        let (rect, response) = ui.allocate_exact_size(vec2(46.0, 22.0), Sense::hover());
        ui.painter().rect_filled(rect, 5.0, color);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, theme::semibold(11.0), Color32::WHITE);
        response.on_hover_text(self.name());
    }
}

#[derive(Default)]
pub(super) struct State {
    tracks: Vec<Track>,
    read_failed: bool,
    search_results: Option<(String, Option<[Option<Vec<Track>>; 3]>)>,
    filter_source: Option<Source>,
    filter_text: String,
    file_work: bool,
    file_reply: Arc<Mutex<Option<FileReply>>>,
}

impl State {
    /// How many songs the playlist holds, for a layout's sidebar count.
    pub(super) fn len(&self) -> usize {
        self.tracks.len()
    }
}

enum FileReply {
    Import(Vec<String>, usize),
    Export(io::Result<()>),
    ReadError,
    Cancelled,
}

pub enum Edit {
    Swap(usize, usize),
    Remove(usize),
    Clear,
}

fn song_url(id: &str) -> Option<String> {
    let safe = |id: &str| !id.is_empty() && id.len() <= 80
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_');
    match Catalog::split_track(id) {
        Some((Catalog::Zing, key)) if safe(key) => Some(format!("https://zingmp3.vn/bai-hat/yozinu/{key}.html")),
        Some((Catalog::Nct, key)) if safe(key) => Some(format!("https://www.nhaccuatui.com/song/{key}")),
        None if id.len() == 11 && safe(id) => Some(format!("https://music.youtube.com/watch?v={id}")),
        _ => None,
    }
}

fn song_id(text: &str) -> Option<String> {
    let url = url::Url::parse(text).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.port().is_some()
        || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let path = url.path();
    let item = || path.rsplit('/').next().filter(|value| !value.is_empty());
    let id = match url.host_str()? {
        "music.youtube.com" | "www.youtube.com" | "youtube.com" | "m.youtube.com" => {
            if path == "/watch" {
                url.query_pairs().find(|(key, _)| key == "v")?.1.into_owned()
            } else if path.starts_with("/shorts/") || path.starts_with("/live/") {
                item()?.to_owned()
            } else {
                return None;
            }
        }
        "youtu.be" => item()?.to_owned(),
        "zingmp3.vn" | "www.zingmp3.vn" if path.starts_with("/bai-hat/") =>
            format!("zing:{}", item()?.strip_suffix(".html")?),
        "nhaccuatui.com" | "www.nhaccuatui.com" if path.starts_with("/song/") =>
            format!("nct:{}", item()?),
        "nhaccuatui.com" | "www.nhaccuatui.com" if path.starts_with("/bai-hat/") =>
            format!("nct:{}", item()?.strip_suffix(".html")?.rsplit_once('.')?.1),
        _ => return None,
    };
    song_url(&id).map(|_| id)
}

fn read_links(path: &Path) -> io::Result<(Vec<String>, usize)> {
    use std::io::Read;
    let mut text = String::new();
    fs::File::open(path)?.take(512 * 1024 + 1).read_to_string(&mut text)?;
    if text.len() > 512 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "playlist file exceeds 512 KiB"));
    }
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    let mut skipped = 0;
    for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(id) = song_id(line) {
            if seen.insert(id.clone()) {
                ids.push(id);
            }
        } else {
            skipped += 1;
        }
    }
    Ok((ids, skipped))
}

impl State {
    pub(super) fn load(dir: &Path) -> Self {
        match load_tracks(dir) {
            Ok(tracks) => Self { tracks, ..Self::default() },
            Err(error) => {
                log::warn!("Mixed playlist could not be loaded: {error}");
                Self { read_failed: true, ..Self::default() }
            }
        }
    }

    fn visible(&self, hide_explicit: bool) -> Vec<usize> {
        let query = fold(self.filter_text.trim());
        self.tracks.iter().enumerate().filter_map(|(index, track)| {
            let matches = (!hide_explicit || !track.explicit)
                && self.filter_source.is_none_or(|source| source == Source::for_track(track))
                && (query.is_empty() || fold(&track.title).contains(&query)
                    || track.artists.iter().any(|artist| fold(&artist.name).contains(&query)));
            matches.then_some(index)
        }).collect()
    }
}

fn fold(text: &str) -> String {
    text.nfd()
        .filter(|&c| !is_combining_mark(c))
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'đ' { 'd' } else { c })
        .collect()
}

fn load_tracks(dir: &Path) -> io::Result<Vec<Track>> {
    match fs::read(dir.join(FILE)) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn save_tracks(dir: &Path, tracks: &[Track]) -> io::Result<()> {
    use std::io::Write;
    fs::create_dir_all(dir)?;
    let path = dir.join(FILE);
    let temp = path.with_extension("json.tmp");
    let mut file = fs::File::create(&temp)?;
    file.write_all(&serde_json::to_vec(tracks)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, path)
}

impl App {
    pub(super) fn pick_mixed_file(&mut self, ctx: &egui::Context, exporting: bool) {
        self.mixed_playlist.file_work = true;
        let slot = self.mixed_playlist.file_reply.clone();
        let ctx = ctx.clone();
        let urls: Vec<_> = if exporting {
            self.mixed_playlist.tracks.iter().filter_map(|track| song_url(&track.id)).collect()
        } else {
            Vec::new()
        };
        let export_title = crate::i18n::label("Export playlist links", "Xuất liên kết playlist");
        let import_title = crate::i18n::label("Import playlist links", "Nhập liên kết playlist");
        std::thread::spawn(move || {
            let dialog = rfd::FileDialog::new().add_filter("TXT / M3U", &["txt", "m3u", "m3u8"]);
            let reply = if exporting {
                dialog.set_title(export_title).set_file_name("YoZiNu-playlist.txt").save_file()
                    .map(|path| FileReply::Export(fs::write(path, urls.join("\n") + "\n")))
            } else {
                dialog.set_title(import_title).pick_file().map(|path| match read_links(&path) {
                    Ok((ids, skipped)) => FileReply::Import(ids, skipped),
                    Err(_) => FileReply::ReadError,
                })
            };
            *slot.lock().unwrap() = Some(reply.unwrap_or(FileReply::Cancelled));
            ctx.request_repaint();
        });
    }

    pub(super) fn collect_mixed_file(&mut self) {
        let reply = self.mixed_playlist.file_reply.lock().unwrap().take();
        let Some(reply) = reply else { return; };
        self.mixed_playlist.file_work = false;
        match reply {
            FileReply::Import(mut ids, skipped) => {
                ids.retain(|id| !self.mixed_playlist.tracks.iter().any(|track| track.id == *id));
                if ids.is_empty() {
                    self.finish_mixed_import(Vec::new(), skipped);
                } else {
                    self.mixed_playlist.file_work = true;
                    self.backend.request(crate::backend::Req::ImportMixed { ids, skipped });
                }
            }
            FileReply::Export(Ok(())) => self.toast(crate::i18n::label("Playlist links exported", "Đã xuất liên kết playlist")),
            FileReply::Export(Err(_)) => self.toast(crate::i18n::label("Could not export the playlist", "Không xuất được playlist")),
            FileReply::ReadError => self.toast(crate::i18n::label("Could not read the playlist file", "Không đọc được tệp playlist")),
            FileReply::Cancelled => {}
        }
    }

    pub(super) fn finish_mixed_import(&mut self, incoming: Vec<Track>, skipped: usize) {
        self.mixed_playlist.file_work = false;
        if self.mixed_playlist.read_failed {
            self.toast(crate::i18n::label("Could not open the local playlist", "Không đọc được playlist trên máy"));
            return;
        }
        let mut tracks = self.mixed_playlist.tracks.clone();
        let old = tracks.len();
        for track in incoming {
            if !tracks.iter().any(|saved| saved.id == track.id) {
                tracks.push(track);
            }
        }
        let added = tracks.len() - old;
        if added > 0 {
            if save_tracks(self.backend.data_dir(), &tracks).is_err() {
                self.toast(crate::i18n::label("Could not save the playlist", "Không lưu được playlist"));
                return;
            }
            self.mixed_playlist.tracks = tracks;
        }
        let template = crate::i18n::label("Imported: {added}. Invalid or unavailable: {skipped}.", "Đã nhập: {added}. Link lỗi hoặc không còn bài: {skipped}.");
        self.toast(template.replace("{added}", &added.to_string()).replace("{skipped}", &skipped.to_string()));
    }

    pub(super) fn add_to_mixed_and_queue(&mut self, track: Track) {
        if self.mixed_playlist.read_failed {
            self.toast("Không đọc được playlist tổng hợp.");
            return;
        }
        let already_saved = self.mixed_playlist.tracks.iter().any(|saved| saved.id == track.id);
        if !already_saved {
            let mut tracks = self.mixed_playlist.tracks.clone();
            tracks.push(track.clone());
            if let Err(error) = save_tracks(self.backend.data_dir(), &tracks) {
                log::warn!("Mixed playlist could not be saved: {error}");
                self.toast("Không lưu được playlist tổng hợp.");
                return;
            }
            self.mixed_playlist.tracks = tracks;
        }
        if self.jam.guest() || (self.hide_explicit && track.explicit) {
            self.toast(if already_saved { "Bài đã có trong playlist tổng hợp" } else { "Đã lưu vào playlist tổng hợp" });
            return;
        }
        let id = track.id.clone();
        let queued = self.queue.tracks.iter().skip(self.queue.index.unwrap_or(0)).any(|item| item.id == id);
        if !queued {
            self.queue_mixed_track(track);
        }
        self.queue_open = true;
        self.queue_reveal = Some(id);
        self.toast(match (already_saved, queued) {
            (false, false) => "Đã thêm vào playlist tổng hợp và Sắp phát",
            (true, false) => "Đã thêm vào Sắp phát",
            (false, true) => "Đã lưu vào playlist tổng hợp",
            (true, true) => "Bài đã có trong playlist tổng hợp và Sắp phát",
        });
    }

    pub(super) fn load_mixed_search(&mut self, query: &str, refresh: bool) {
        if self.mixed_playlist.search_results.as_ref().is_some_and(|(active, found)| active == query && (found.is_none() || !refresh)) {
            return;
        }
        let query = query.to_owned();
        self.mixed_playlist.search_results = Some((query.clone(), None));
        self.backend.request(crate::backend::Req::MixedSearch(query));
    }

    pub(super) fn accept_mixed_search(&mut self, query: &str, groups: [Option<Vec<Track>>; 3]) {
        if let Some((active, found)) = self.mixed_playlist.search_results.as_mut()
            && active == query
        {
            *found = Some(groups);
        }
    }

    pub(super) fn edit_mixed(&mut self, edit: Edit) {
        if self.mixed_playlist.read_failed {
            self.toast("Không đọc được playlist tổng hợp.");
            return;
        }
        let mut tracks = self.mixed_playlist.tracks.clone();
        let message = match edit {
            Edit::Swap(a, b) if a < tracks.len() && b < tracks.len() => {
                tracks.swap(a, b);
                None
            }
            Edit::Remove(index) if index < tracks.len() => {
                tracks.remove(index);
                Some("Đã bỏ bài khỏi playlist tổng hợp")
            }
            Edit::Clear if !tracks.is_empty() => {
                tracks.clear();
                Some("Đã xóa tất cả bài khỏi playlist tổng hợp")
            }
            _ => return,
        };
        if let Err(error) = save_tracks(self.backend.data_dir(), &tracks) {
            log::warn!("Mixed playlist could not be saved: {error}");
            self.toast("Không lưu được playlist tổng hợp.");
            return;
        }
        self.mixed_playlist.tracks = tracks;
        if let Some(message) = message {
            self.toast(message);
        }
    }
}

pub(super) fn results(app: &mut App, ui: &mut Ui, query: &str) {
    pages::page_title(ui, crate::i18n::label("Search all three sources", "Tìm trên cả ba nguồn"));
    ui.add_space(12.0);
    if widgets::pill(ui, title(), Some(icon::LIST_MUSIC), false).clicked() {
        app.actions.push(Action::Go(Page::MixedPlaylist));
    }
    let Some((_, Some(groups))) = app.mixed_playlist.search_results.as_ref().filter(|(active, _)| active == query) else {
        ui.spinner();
        return;
    };
    let rows = pages::Rows::new(app, ui.input(|i| i.time));
    for (source, group) in Source::ALL.into_iter().zip(groups) {
        ui.push_id(source.name(), |ui| {
            widgets::shelf_header(ui, source.name(), None, None, false);
            match group {
                Some(tracks) if tracks.iter().any(|track| rows.shows(track)) => {
                    for track in tracks.iter().filter(|track| rows.shows(track)) {
                        ui.push_id(&track.id, |ui| {
                            let options = rows.opts(&app.liked, track, None, true);
                            if widgets::search_track_row(ui, track, &options, &mut app.actions).clicked() {
                                app.actions.push(Action::Open(crate::model::Target::Song(track.clone())));
                            }
                        });
                    }
                }
                Some(_) => { ui.label(crate::i18n::label("No songs found.", "Không tìm thấy bài hát.")); }
                None => { ui.label(crate::i18n::label("Could not load this source.", "Không tải được kết quả từ nguồn này.")); }
            }
        });
    }
    if groups.iter().any(Option::is_none) && widgets::pill(ui, crate::i18n::label("Retry", "Thử lại"), None, false).clicked() {
        app.actions.push(Action::Go(Page::MixedSearch(query.to_owned())));
    }
}

pub(super) fn page(app: &mut App, ui: &mut Ui) {
    pages::page_title(ui, title());
    // Studio Console reads the list as a multi-column table; Velvet Gallery as a cover grid.
    let table = matches!(app.layout_choice, Choice::Playlist | Choice::Console) && ui.available_width() >= 665.0;
    let grid = app.layout_choice == Choice::Velvet;
    let short = ui.ctx().content_rect().height() < 560.0;
    let busy = app.mixed_playlist.file_work;
    let can_export = app.mixed_playlist.tracks.iter().any(|track| song_url(&track.id).is_some());
    let has_tracks = !app.mixed_playlist.tracks.is_empty();
    ui.horizontal_wrapped(|ui| {
        let can_import = !busy && !app.mixed_playlist.read_failed;
        if ui.add_enabled(can_import, egui::Button::new(crate::i18n::label("Import links", "Nhập liên kết"))).clicked() {
            app.pick_mixed_file(ui.ctx(), false);
        }
        if ui.add_enabled(!busy && can_export, egui::Button::new(crate::i18n::label("Export links", "Xuất liên kết"))).clicked() {
            app.pick_mixed_file(ui.ctx(), true);
        }
        if ui.add_enabled(!busy && has_tracks, egui::Button::new(crate::i18n::label("Clear all", "Xóa tất cả"))).clicked() {
            app.actions.push(Action::EditMixed(Edit::Clear));
        }
        if busy { ui.spinner(); }
    });
    ui.add_space(if short { 4.0 } else { 12.0 });
    ui.add_space(if short { 4.0 } else { 18.0 });

    let accent = app.accent_color();
    let playing = app.playing();
    let state = &mut app.mixed_playlist;
    if state.read_failed {
        ui.label("Không đọc được dữ liệu playlist tổng hợp.");
        if widgets::pill(ui, "Tải lại playlist", None, false).clicked() {
            *state = State::load(app.backend.data_dir());
        }
        return;
    }
    if state.tracks.is_empty() {
        ui.label(crate::i18n::label("This playlist is empty. Find a song and choose Add to playlist.", "Playlist chưa có bài hát. Tìm bài và chọn biểu tượng thêm vào playlist."));
        return;
    }

    ui.horizontal_wrapped(|ui| {
        if widgets::chip(ui, crate::i18n::label("All", "Tất cả"), state.filter_source.is_none()).clicked() {
            state.filter_source = None;
        }
        for source in Source::ALL {
            if widgets::chip(ui, source.name(), state.filter_source == Some(source)).clicked() {
                state.filter_source = Some(source);
            }
        }
    });
    ui.add_space(8.0);
    ui.add(egui::TextEdit::singleline(&mut state.filter_text)
        .desired_width(ui.available_width().min(330.0)).hint_text(crate::i18n::label("Filter by song or artist", "Lọc theo tên bài hoặc nghệ sĩ")));

    let visible = state.visible(app.hide_explicit);
    if visible.is_empty() {
        ui.add_space(12.0);
        ui.label(crate::i18n::label("No matching songs.", "Không có bài hát phù hợp."));
        return;
    }

    let filtered = state.filter_source.is_some() || !state.filter_text.trim().is_empty();
    ui.add_space(12.0);
    let mut play = None;
    ui.horizontal_wrapped(|ui| {
        let label = if filtered { crate::i18n::label("Play filtered results", "Phát kết quả lọc") } else { crate::i18n::label("Play all", "Phát tất cả") };
        if widgets::pill(ui, label, Some(icon::PLAY), true).clicked() {
            play = Some(0);
        }
        ui.label(RichText::new(format!("{} bài hát", visible.len())).color(theme::text_dim()));
    });
    ui.add_space(12.0);

    let current = app.queue.current().map(|track| track.id.as_str()).filter(|_| table || grid || app.queue.mixed);

    // Velvet Gallery shows the playlist as a wall of covers, four across.
    if grid {
        let width = ui.available_width();
        let cols = if width >= 900.0 { 4 } else if width >= 640.0 { 3 } else { 2 };
        let mut start = 0;
        while start < visible.len() {
            let count = cols.min(visible.len() - start);
            ui.columns(cols, |columns| {
                for (col, ui) in columns.iter_mut().enumerate().take(count) {
                    let shown = start + col;
                    let track = &state.tracks[visible[shown]];
                    let cell = ui.available_width();
                    // A cover that leaves room under it for the title, on one screenful.
                    let side = cell.min(if short { 132.0 } else { 204.0 });
                    ui.push_id(&track.id, |ui| {
                        let active = current == Some(track.id.as_str());
                        let mut clicked = false;
                        ui.horizontal(|ui| {
                            ui.add_space((cell - side) * 0.5);
                            let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click());
                            if ui.is_rect_visible(rect) {
                                widgets::paint_cover(ui, rect, &track.thumbs, theme::card_radius(), 1.0);
                                if active || resp.hovered() {
                                    let at = rect.right_bottom() - vec2(18.0, 18.0);
                                    ui.painter().circle_filled(at, 15.0, theme::shade(0.72));
                                    widgets::play_pause_glyph(ui, at, 11.0, Color32::WHITE, active && playing);
                                }
                            }
                            clicked = resp.clicked();
                        });
                        if clicked {
                            play = Some(shown);
                        }
                        ui.add(egui::Label::new(RichText::new(&track.title).font(theme::semibold(13.0))).truncate());
                        ui.add(egui::Label::new(RichText::new(track.artist_line()).font(theme::regular(11.5)).color(theme::text_dim())).truncate());
                    });
                }
            });
            start += count;
            ui.add_space(16.0);
        }
        if let Some(start) = play {
            let tracks = visible.iter().map(|&index| state.tracks[index].clone()).collect();
            app.actions.push(Action::PlayMixed(tracks, start));
        }
        return;
    }

    let spacing = ui.spacing().item_spacing.x;
    let controls_width = 3.0 * (33.0 + spacing);
    let columns = table.then(|| {
        let width = ui.available_width();
        let artist = (width * 0.23).min(260.0);
        let duration = 86.0;
        let song = (width - artist - duration - 61.0 - controls_width - 4.0 * spacing).max(150.0);
        [song, artist, duration]
    });
    if let Some([song, artist, duration]) = columns {
        ui.horizontal(|ui| {
            table_text(ui, crate::i18n::label("Song", "Bài hát"), song, true);
            table_text(ui, crate::i18n::label("Artist", "Nghệ sĩ"), artist, true);
            table_text(ui, crate::i18n::label("Source", "Nguồn"), 54.0, true);
            table_text(ui, crate::i18n::label("Time", "Thời lượng"), duration, true);
        });
        ui.separator();
    }
    for (shown, &index) in visible.iter().enumerate() {
        let track = &state.tracks[index];
        ui.push_id(&track.id, |ui| {
            ui.horizontal(|ui| {
                if !table {
                    Source::for_track(track).badge(ui);
                }
                let width = columns.map_or_else(|| (ui.available_width() - controls_width).max(40.0), |c| c[0]);
                if song_row(ui, track, index, width, current == Some(track.id.as_str()), accent, table) {
                    play = Some(shown);
                }
                if let Some([_, artist, duration]) = columns {
                    table_text(ui, &track.artist_line(), artist, false);
                    ui.allocate_ui(vec2(54.0, 22.0), |ui| Source::for_track(track).badge(ui));
                    table_text(ui, &track.duration.map(fmt_time).unwrap_or_default(), duration, false);
                }
                for (target, glyph, label) in [
                    (shown.checked_sub(1).and_then(|i| visible.get(i)), icon::CHEVRON_UP, crate::i18n::label("Move up", "Chuyển lên")),
                    (visible.get(shown + 1), icon::CHEVRON_DOWN, crate::i18n::label("Move down", "Chuyển xuống")),
                ] {
                    let button = ui.add_enabled_ui(target.is_some(), |ui| widgets::icon_button(ui, glyph, 17.0, label, false));
                    if let Some(&target) = target && button.inner.clicked() {
                        app.actions.push(Action::EditMixed(Edit::Swap(index, target)));
                    }
                }
                if widgets::icon_button(ui, icon::X, 17.0, crate::i18n::label("Remove from playlist", "Bỏ khỏi playlist"), false).clicked() {
                    app.actions.push(Action::EditMixed(Edit::Remove(index)));
                }
            });
        });
        ui.separator();
    }

    if let Some(start) = play {
        let tracks = visible.iter().map(|&index| state.tracks[index].clone()).collect();
        app.actions.push(Action::PlayMixed(tracks, start));
    }
}

fn table_text(ui: &mut Ui, value: &str, width: f32, heading: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, if heading { 26.0 } else { 40.0 }), Sense::hover());
    let font = if heading { theme::semibold(13.0) } else { theme::regular(13.0) };
    widgets::text_at(ui, pos2(rect.left() + 4.0, rect.center().y - 8.0), value, font, theme::text_dim(), width - 8.0);
}

fn song_row(ui: &mut Ui, track: &Track, index: usize, width: f32, current: bool, accent: Color32, table: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 40.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let response = widgets::named(response, &track.title).on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() || (current && table) {
        let fill = if current && table { theme::with_alpha(accent, 0.13) } else { theme::glass(0.09) };
        ui.painter().rect_filled(rect, 6.0, fill);
    }
    widgets::focus_ring(ui, &response, rect, 8.0);
    let cover = Rect::from_min_size(rect.min + vec2(3.0, 3.0), vec2(34.0, 34.0));
    widgets::paint_cover(ui, cover, &track.thumbs, theme::thumb_radius(), 1.0);
    let x = cover.right() + 9.0;
    let text_width = (rect.right() - x - 4.0).max(0.0);
    let title = format!("{}. {}", index + 1, track.title);
    let ink = if current { accent } else { theme::text() };
    let y = if table { 10.0 } else { 2.0 };
    widgets::text_at(ui, pos2(x, rect.top() + y), &title, theme::semibold(14.0), ink, text_width);
    let artists = track.artist_line();
    if !table {
        widgets::text_at(ui, pos2(x, rect.top() + 21.0), &artists, theme::regular(12.0), theme::text_dim(), text_width);
    }
    response.on_hover_text(format!("{} · {artists}", track.title)).clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_round_trip_and_only_accept_supported_song_urls() {
        for id in ["qSay_8xzijg", "zing:SNF1Y2Bun4w", "nct:JzDZ5BW4RSTI"] {
            assert_eq!(song_id(&song_url(id).unwrap()).as_deref(), Some(id));
        }
        assert_eq!(song_id("https://youtu.be/qSay_8xzijg?t=2").as_deref(), Some("qSay_8xzijg"));
        assert_eq!(song_id("https://www.nhaccuatui.com/bai-hat/song.JzDZ5BW4RSTI.html").as_deref(), Some("nct:JzDZ5BW4RSTI"));
        assert!(song_id("https://music.youtube.com.evil.example/watch?v=qSay_8xzijg").is_none());
        assert!(song_id("https://zingmp3.vn/album/title/abc.html").is_none());
        assert!(song_id("https://music.youtube.com/playlist?list=123").is_none());
    }

    #[test]
    fn filters_vietnamese_text_and_source_without_changing_saved_order() {
        let mut state = State {
            tracks: vec![
                Track { id: "test-youtube".into(), title: "Đường về nhà".into(), ..Track::default() },
                Track { id: "zing:one".into(), title: "Đường về nhà".into(), ..Track::default() },
                Track { id: "nct:two".into(), title: "Đường về nhà".into(), explicit: true, ..Track::default() },
            ],
            filter_text: "duong ve".into(),
            ..State::default()
        };
        assert_eq!(state.visible(false), vec![0, 1, 2]);
        assert_eq!(state.visible(true), vec![0, 1]);
        state.filter_source = Some(Source::Zing);
        assert_eq!(state.visible(false), vec![1]);
    }

    #[test]
    fn replacing_a_saved_playlist_keeps_the_new_cross_source_order() {
        let dir = std::env::temp_dir().join(format!("yozinu-mixed-test-{}", std::process::id()));
        let mut tracks: Vec<_> = ["test-youtube", "zing:one", "nct:two"].into_iter()
            .map(|id| Track { id: id.into(), ..Track::default() }).collect();
        save_tracks(&dir, &tracks).unwrap();
        tracks.swap(0, 2);
        save_tracks(&dir, &tracks).unwrap();
        assert_eq!(load_tracks(&dir).unwrap(), tracks);
        fs::remove_file(dir.join(FILE)).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
