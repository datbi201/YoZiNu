//! Alternate native layouts. Only the active layout draws; all controls use the same App.
use egui::{Color32, Frame, Margin, Rect, RichText, Sense, Ui, vec2};

use crate::catalog::Catalog;
use crate::i18n::label;
use crate::model::Track;
use crate::theme::{self, icon};

use super::{Action, App, NpTab, Page, Repeat, fmt_time, nowplaying, pages, shell, widgets};

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(super) enum Choice {
    #[default]
    Original,
    Covers,
    Wide,
    Studio,
    Mosaic,
    Playlist,
    Listening,
    /// Rev0.6: ambient light that follows the cover, a grouped sidebar, a right panel.
    Aurora,
    /// Rev0.6: a three-column Hi-Fi station with a now-playing inspector.
    Console,
    /// Rev0.6: warm editorial gallery, no fixed sidebar, floating dock.
    Velvet,
    /// Rev0.6: the flat, RAM-minded layout.
    Feather,
}

impl Choice {
    pub(super) const ALL: [Self; 11] = [
        Self::Original,
        Self::Covers,
        Self::Wide,
        Self::Studio,
        Self::Mosaic,
        Self::Playlist,
        Self::Listening,
        Self::Aurora,
        Self::Console,
        Self::Velvet,
        Self::Feather,
    ];

    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Covers => "layout-covers",
            Self::Wide => "layout-wide",
            Self::Studio => "layout-studio",
            Self::Mosaic => "layout-mosaic",
            Self::Playlist => "layout-playlist",
            Self::Listening => "layout-listening",
            Self::Aurora => "layout-aurora",
            Self::Console => "layout-console",
            Self::Velvet => "layout-velvet",
            Self::Feather => "layout-feather",
        }
    }

    pub(super) fn from_id(id: Option<&str>) -> Self {
        Self::ALL.into_iter().find(|choice| Some(choice.id()) == id).unwrap_or_default()
    }

    pub(super) fn preset(self) -> Option<&'static str> {
        (self != Self::Original).then(|| self.id())
    }

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Original => label("Original", "Giao diện gốc"),
            Self::Covers => label("Cover Gallery", "Bộ sưu tập bìa"),
            Self::Wide => label("Continuous List", "Danh sách liền mạch"),
            Self::Studio => label("Listening Console", "Bàn điều khiển"),
            Self::Mosaic => label("Cover Discovery", "Khám phá qua ảnh bìa"),
            Self::Playlist => label("Music Library", "Kho nhạc"),
            Self::Listening => label("Listening Room", "Phòng nghe"),
            // Rev0.6's four keep their own names in both languages.
            Self::Aurora => "Aurora Stage",
            Self::Console => "Studio Console",
            Self::Velvet => "Velvet Gallery",
            Self::Feather => label("Feather Lite", "Feather Lite"),
        }
    }

    pub(super) fn light(self) -> Option<bool> {
        match self {
            Self::Original => None,
            // Rev0.6's four follow Settings › Appearance, in either mode.
            Self::Aurora | Self::Console | Self::Velvet | Self::Feather => None,
            Self::Wide | Self::Listening => Some(false),
            _ => Some(true),
        }
    }

    fn vertical_nav(self) -> bool {
        matches!(self, Self::Covers | Self::Playlist)
    }

    fn preview_count(self) -> usize {
        match self {
            Self::Wide | Self::Listening => 3,
            Self::Mosaic => 1,
            Self::Playlist => 2,
            _ => 0,
        }
    }

    /// Rev0.6's layouts draw their own frame; the six from Rev0.5 share the one below.
    fn own_frame(self) -> bool {
        matches!(self, Self::Aurora | Self::Console | Self::Velvet | Self::Feather)
    }
}

impl App {
    pub(super) fn choose_layout(&mut self, choice: Choice, ctx: &egui::Context) {
        if self.layout_choice == choice {
            return;
        }
        self.layout_choice = choice;
        self.sync_look();
        self.art_key = None;
        self.request_art();
        ctx.request_repaint();
    }
}

fn mode_color(dark: Color32, light: Color32) -> Color32 {
    if theme::light() { light } else { dark }
}

fn surface(choice: Choice) -> Color32 {
    match choice {
        Choice::Covers => Color32::from_rgb(237, 233, 220),
        Choice::Wide => Color32::from_rgb(20, 40, 59),
        Choice::Studio => Color32::from_rgb(245, 226, 219),
        Choice::Mosaic => Color32::from_rgb(232, 219, 250),
        Choice::Playlist => Color32::from_rgb(231, 219, 203),
        Choice::Listening => Color32::from_rgb(62, 33, 43),
        // Rev0.6's four draw in both modes.
        Choice::Aurora => mode_color(Color32::from_rgb(12, 14, 20), Color32::from_rgb(244, 246, 251)),
        Choice::Console => mode_color(Color32::from_rgb(10, 11, 14), Color32::from_rgb(245, 247, 250)),
        Choice::Velvet => mode_color(Color32::from_rgb(19, 17, 16), Color32::from_rgb(248, 245, 240)),
        Choice::Feather => mode_color(Color32::from_rgb(14, 16, 19), Color32::from_rgb(255, 255, 255)),
        Choice::Original => theme::menu_fill_solid(),
    }
}

fn panel(choice: Choice, padding: Margin) -> Frame {
    let mut fill = surface(choice);
    // Rev0.6's panels sit a touch off the page's own colour, so the frame reads in both
    // modes without a border on every edge.
    if choice.own_frame() {
        fill = theme::lerp_color(fill, theme::text(), if theme::light() { 0.035 } else { 0.05 });
    }
    Frame::new().fill(fill).inner_margin(padding)
}

pub(super) fn draw(app: &mut App, ui: &mut Ui, now: f64, in_room: bool) {
    let choice = app.layout_choice;
    // Covers are fetched at their drawn size, unless the light layout caps them below.
    widgets::set_cover_px_cap(ui, None);
    // The light layout keeps a small cover cache, dropped on a five-minute timer.
    crate::images::set_lite(choice == Choice::Feather);
    crate::images::tick_lite(ui.ctx());
    if choice.own_frame() {
        match choice {
            Choice::Aurora => aurora(app, ui, now),
            Choice::Console => console(app, ui, now),
            Choice::Velvet => velvet(app, ui, now),
            Choice::Feather => feather(app, ui, now),
            _ => {}
        }
        return;
    }
    let width = ui.available_width();
    let studio = choice == Choice::Studio && width >= 1020.0;
    let in_room = in_room && app.current().is_some();
    let full_queue = app.queue_open && !in_room;
    let dock_queue = !studio && !in_room && width >= 1050.0 && choice == Choice::Covers;
    let compact_queue_page = full_queue && !studio && !dock_queue && app.page != Page::Settings;
    let preview_count = choice.preview_count();
    let show_preview = !in_room && !full_queue && preview_count > 0 && app.current().is_some();

    // The player and header reserve their own rows. A queue never overlays the header.
    if !in_room && !studio {
        egui::Panel::bottom("layout-player").resizable(false).exact_size(app.bar_height())
            .frame(panel(choice, Margin::ZERO))
            .show_separator_line(false)
            .show(ui, |ui| shell::player_bar(app, ui, now));
    }
    if show_preview {
        let height = if choice == Choice::Playlist { 64.0 } else { 55.0 };
        egui::Panel::bottom("layout-preview").resizable(false).exact_size(height)
            .frame(panel(choice, Margin::symmetric(16, 6)))
            .show_separator_line(false)
            .show(ui, |ui| upcoming(app, ui, preview_count, true));
    }

    let side_width = if width < 860.0 { 72.0 } else if choice == Choice::Covers { 194.0 } else { 178.0 };
    let top = super::titlebar::top_margin(8, 40.0) + super::titlebar::below_lights(ui.ctx()) as i8;
    egui::Panel::top("layout-header").resizable(false)
        .frame(panel(choice, Margin { left: 14, top, right: 10, bottom: 8 }))
        .show_separator_line(false)
        .show(ui, |ui| {
            shell::top_bar(app, ui);
            if !choice.vertical_nav() {
                ui.add_space(6.0);
                navigation(app, ui, true, in_room);
            }
        });

    if choice.vertical_nav() {
        let mut frame = panel(choice, Margin::symmetric(10, 12));
        if choice == Choice::Playlist {
            frame.fill = Color32::from_rgb(41, 39, 38);
        }
        egui::Panel::left("layout-nav").resizable(false).exact_size(side_width)
            .frame(frame).show_separator_line(false)
            .show(ui, |ui| {
                let light = theme::light();
                if choice == Choice::Playlist {
                    theme::set_mode(ui.ctx(), false);
                    ui.style_mut().visuals = ui.ctx().global_style().visuals.clone();
                }
                navigation(app, ui, false, in_room);
                theme::set_mode(ui.ctx(), light);
            });
    }
    if studio {
        egui::Panel::left("layout-studio").resizable(false).exact_size((width * 0.32).clamp(280.0, 365.0))
            .frame(panel(choice, Margin::same(16))).show_separator_line(false)
            .show(ui, |ui| {
                listener(app, ui, false);
                ui.add_space(16.0);
                ui.separator();
                if full_queue {
                    shell::queue_drawer(app, ui);
                } else {
                    catalog_queue(app, ui);
                }
            });
    } else if dock_queue && (full_queue || matches!(app.page, Page::CatalogHome(_) | Page::CatalogSearch(..))) {
        egui::Panel::right("layout-queue").resizable(false).exact_size((width * 0.25).clamp(238.0, 304.0))
            .frame(panel(choice, Margin::symmetric(14, 16))).show_separator_line(false)
            .show(ui, |ui| {
                if full_queue {
                    shell::queue_drawer(app, ui);
                } else {
                    catalog_queue(app, ui);
                }
            });
    }

    egui::CentralPanel::default().frame(Frame::new().inner_margin(Margin::symmetric(19, 13)))
        .show(ui, |ui| {
            app.page_insets = [0.0, 0.0];
            if in_room {
                room(app, ui, now);
            } else if compact_queue_page {
                shell::queue_drawer(app, ui);
            } else {
                ui.scope_builder(egui::UiBuilder::new().scope_id(egui::Id::unique("main-page")), |ui| pages::page(app, ui, now));
            }
        });
}

fn navigation(app: &mut App, ui: &mut Ui, horizontal: bool, in_room: bool) {
    let small = if horizontal { ui.available_width() < 820.0 } else { ui.available_width() < 100.0 };
    let nav = |app: &mut App, ui: &mut Ui| {
        if !horizontal {
            ui.label(RichText::new(if small { "YZ" } else { "YoZiNu" }).font(theme::bold(23.0)).color(theme::text()));
            ui.add_space(18.0);
        } else {
            ui.label(RichText::new("YoZiNu").font(theme::bold(19.0)).color(app.accent_color()));
            ui.add_space(8.0);
        }
        let playlist = label("Mixed playlist", "Playlist tổng hợp");
        let settings = label("Settings", "Cài đặt");
        let mixed = matches!(app.page, Page::MixedPlaylist | Page::MixedSearch(_));
        let items = [
            (icon::HOUSE, "YouTube", "YouTube", Page::Home, app.page == Page::Home),
            (icon::MUSIC, "Zing MP3", "Zing MP3", Page::CatalogHome(Catalog::Zing), matches!(app.page, Page::CatalogHome(Catalog::Zing) | Page::CatalogSearch(Catalog::Zing, _))),
            (icon::MUSIC, "NhacCuaTui", "NhacCuaTui", Page::CatalogHome(Catalog::Nct), matches!(app.page, Page::CatalogHome(Catalog::Nct) | Page::CatalogSearch(Catalog::Nct, _))),
            (icon::LIST_MUSIC, playlist, "Playlist", Page::MixedPlaylist, mixed),
            (icon::SETTINGS, settings, settings, Page::Settings, app.page == Page::Settings),
        ];
        let order = if !horizontal && app.layout_choice == Choice::Playlist { [3, 4, 0, 1, 2] } else { [0, 1, 2, 3, 4] };
        for (i, item) in order.into_iter().map(|index| &items[index]).enumerate() {
            if !horizontal && app.layout_choice == Choice::Playlist && i == 2 {
                ui.add_space(10.0);
                ui.separator();
                if !small {
                    ui.label(RichText::new(label("Music sources", "Nguồn nhạc")).color(theme::text_dim()));
                }
            }
            let (glyph, full, short, page, selected) = item;
            let text = if horizontal { if small { short } else { full } } else { short };
            let icon_only = small && (!horizontal || matches!(page, Page::Home | Page::Settings));
            if nav_button(ui, *glyph, text, full, icon_only, !in_room && *selected, horizontal, app.accent_color()) {
                app.actions.push(Action::Go(page.clone()));
            }
        }
        if app.layout_choice == Choice::Listening {
            let listen = label("Now playing", "Đang phát");
            if ui.add_enabled_ui(app.current().is_some(), |ui| nav_button(ui, icon::PLAY, listen, listen, false, in_room, horizontal, app.accent_color())).inner {
                app.actions.push(Action::OpenNowPlaying(!in_room));
            }
        }
    };
    if horizontal {
        ui.horizontal_wrapped(|ui| nav(app, ui));
    } else {
        nav(app, ui);
    }
}

fn nav_button(ui: &mut Ui, glyph: char, visible: &str, name: &str, icon_only: bool, active: bool, horizontal: bool, accent: Color32) -> bool {
    let font = theme::semibold(14.0);
    let label_width = ui.painter().layout_no_wrap(visible.to_owned(), font.clone(), theme::text()).size().x;
    let width = if horizontal {
        if icon_only { 37.0 } else { (43.0 + label_width).min(ui.available_width().max(43.0)) }
    } else {
        ui.available_width()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 33.0), Sense::click());
    let response = widgets::named(response, name).on_hover_text(name).on_hover_cursor(egui::CursorIcon::PointingHand);
    if active {
        ui.painter().rect_filled(rect, 7.0, accent);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 7.0, theme::glass(0.09));
    }
    let ink = if active {
        if theme::luma(accent) > 145.0 { Color32::from_gray(16) } else { Color32::WHITE }
    } else {
        theme::text()
    };
    let x = if icon_only { rect.center().x } else { rect.left() + 17.0 };
    widgets::icon_at(ui, egui::pos2(x, rect.center().y), glyph, 17.0, ink);
    if !icon_only {
        widgets::text_at(ui, egui::pos2(rect.left() + 33.0, rect.top() + 7.0), visible, font, ink, (rect.width() - 38.0).max(0.0));
    }
    widgets::focus_ring(ui, &response, rect, 7.0);
    response.clicked()
}

fn next_indices(app: &App, limit: usize) -> impl Iterator<Item = usize> {
    let len = app.queue.tracks.len();
    let current = app.queue.index.filter(|&index| index < len);
    let wrap = app.queue.repeat == Repeat::All;
    let count = current.map_or(0, |index| {
        if app.queue.repeat == Repeat::One {
            1
        } else if wrap {
            len.saturating_sub(1).max(1)
        } else {
            len - index - 1
        }
    });
    let offset = usize::from(app.queue.repeat != Repeat::One);
    (0..count.min(limit)).map(move |i| (current.unwrap_or(0) + offset + i) % len.max(1))
}

fn upcoming(app: &mut App, ui: &mut Ui, limit: usize, single_line: bool) {
    let title = label("Up next", "Sắp phát");
    let limit = limit.min(if single_line { ((ui.available_width() - 240.0) / 200.0).floor().max(1.0) as usize } else { limit });
    let indices = next_indices(app, limit).collect::<Vec<_>>();
    let width = if single_line { ((ui.available_width() - 230.0) / indices.len().max(1) as f32).clamp(120.0, 290.0) } else { ui.available_width() };
    let heading = |ui: &mut Ui| ui.label(RichText::new(title).font(theme::bold(16.0)).color(theme::text()));
    let open_text = label("Open queue", "Mở Sắp phát");
    if single_line {
        ui.horizontal_centered(|ui| {
            heading(ui);
            if indices.is_empty() {
                ui.label(RichText::new(label("No upcoming song", "Chưa có bài tiếp theo")).color(theme::text_dim()));
            }
            for (position, &index) in indices.iter().enumerate() {
                if queue_preview_row(ui, &app.queue.tracks[index], width, position == 0, app.accent_color()) {
                    app.actions.push(Action::QueueJump(index));
                }
            }
            if widgets::pill(ui, open_text, Some(icon::LIST_MUSIC), false).clicked() {
                app.queue_open = true;
            }
        });
    } else {
        ui.horizontal(|ui| {
            heading(ui);
            if widgets::pill(ui, open_text, Some(icon::LIST_MUSIC), false).clicked() {
                app.queue_open = true;
            }
        });
        if indices.is_empty() {
            ui.label(RichText::new(label("No upcoming song", "Chưa có bài tiếp theo")).color(theme::text_dim()));
        }
        for (position, &index) in indices.iter().enumerate() {
            if queue_preview_row(ui, &app.queue.tracks[index], width, position == 0, app.accent_color()) {
                app.actions.push(Action::QueueJump(index));
            }
        }
    }
}

fn queue_preview_row(ui: &mut Ui, track: &Track, width: f32, first: bool, accent: Color32) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 39.0), Sense::click());
    let response = widgets::named(response, &track.title).on_hover_cursor(egui::CursorIcon::PointingHand);
    let color = if first { theme::lerp_color(theme::bg(), accent, 0.14) } else { theme::glass(0.05) };
    if first || response.hovered() {
        ui.painter().rect_filled(rect, 6.0, color);
    }
    widgets::focus_ring(ui, &response, rect, 6.0);
    let cover = Rect::from_min_size(rect.min + vec2(5.0, 4.0), vec2(31.0, 31.0));
    widgets::paint_cover(ui, cover, &track.thumbs, 4.0, 1.0);
    let x = cover.right() + 8.0;
    let duration = track.duration.map(fmt_time).unwrap_or_default();
    let width = (rect.right() - x - if duration.is_empty() { 7.0 } else { 48.0 }).max(0.0);
    widgets::text_at(ui, egui::pos2(x, rect.top() + 3.0), &track.title, theme::semibold(13.0), theme::text(), width);
    let artist = track.artist_line();
    let sub = if first { format!("{} · {artist}", label("Next song", "Bài kế tiếp")) } else { artist };
    widgets::text_at(ui, egui::pos2(x, rect.top() + 20.0), &sub, theme::regular(11.5), if first { accent } else { theme::text_dim() }, width);
    ui.painter().text(rect.right_center() - vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, duration, theme::regular(11.5), theme::text_dim());
    response.on_hover_text(format!("{} · {}", track.title, track.artist_line())).clicked()
}

fn catalog_queue(app: &mut App, ui: &mut Ui) {
    upcoming(app, ui, 3, false);
}

fn listener(app: &mut App, ui: &mut Ui, large: bool) {
    let compact = ui.available_height() < 420.0;
    ui.label(RichText::new(label("Now playing", "Đang phát")).font(theme::bold(19.0)).color(theme::text()));
    ui.add_space(8.0);
    let Some(track) = app.current() else {
        ui.label(RichText::new(label("Choose a song to start", "Chọn một bài để bắt đầu")).color(theme::text_dim()));
        return;
    };
    let cover_size = if large && compact {
        (ui.available_height() - 224.0).clamp(45.0, 100.0)
    } else if large {
        (ui.available_height() - 222.0).clamp(110.0, 310.0)
    } else {
        (ui.available_height() * 0.29).clamp(82.0, 188.0)
    }.min(ui.available_width());
    let cover = ui.allocate_exact_size(vec2(cover_size, cover_size), Sense::hover()).0;
    widgets::paint_cover(ui, cover, &track.thumbs, theme::card_radius(), 1.0);
    ui.add_space(6.0);
    ui.add(egui::Label::new(RichText::new(&track.title).font(theme::bold(if large && !compact { 25.0 } else { 19.0 }))).truncate()).on_hover_text(&track.title);
    let artist = track.artist_line();
    ui.add(egui::Label::new(RichText::new(&artist).color(theme::text_dim())).truncate()).on_hover_text(&artist);
    if let Some((source, _)) = Catalog::split_track(&track.id) {
        ui.label(RichText::new(source.name()).font(theme::regular(12.0)).color(theme::text_dim()));
    }
    let id = track.id.clone();
    let Some(total) = app.audio.status.duration().map(|d| d.as_secs_f32()).filter(|d| *d > 0.0).or_else(|| track.duration.map(|d| d as f32).filter(|d| *d > 0.0)) else {
        listener_controls(app, ui, large);
        return;
    };
    ui.add_space(4.0);
    let mut pos = app.audio.status.position_smooth().as_secs_f32().clamp(0.0, total);
    let seek_id = ui.scope_id().with(("layout-seek", id));
    if let Some(value) = ui.data(|d| d.get_temp::<f32>(seek_id)) {
        pos = value;
    }
    let slider = ui.scope(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 10.0).max(40.0);
        ui.add(egui::Slider::new(&mut pos, 0.0..=total).show_value(false))
    }).inner.on_hover_text(label("Seek", "Tua bài"));
    if slider.dragged() && slider.changed() {
        ui.data_mut(|d| d.insert_temp(seek_id, pos));
    } else if slider.drag_stopped() || slider.changed() {
        ui.data_mut(|d| d.remove::<f32>(seek_id));
        app.actions.push(Action::Seek(pos));
    }
    ui.label(RichText::new(format!("{} / {}", fmt_time(pos as u32), fmt_time(total as u32))).font(theme::regular(12.0)).color(theme::text_dim()));
    listener_controls(app, ui, large);
}

fn listener_controls(app: &mut App, ui: &mut Ui, in_room: bool) {
    let playing = app.playing();
    let repeat = app.queue.repeat;
    ui.horizontal_wrapped(|ui| {
        if widgets::icon_button(ui, icon::SHUFFLE, 17.0, label("Shuffle", "Phát ngẫu nhiên"), app.queue.shuffled()).clicked() {
            app.toggle_shuffle();
        }
        if widgets::icon_button(ui, icon::SKIP_BACK, 20.0, label("Previous", "Bài trước"), false).clicked() {
            app.prev();
        }
        let play_label = if playing { label("Pause", "Tạm dừng") } else { label("Play", "Phát") };
        if widgets::pill(ui, play_label, Some(if playing { icon::PAUSE } else { icon::PLAY }), true).clicked() {
            app.actions.push(Action::TogglePlay);
        }
        if widgets::icon_button(ui, icon::SKIP_FORWARD, 20.0, label("Next", "Bài tiếp theo"), false).clicked() {
            app.next();
        }
        if widgets::icon_button(ui, if repeat == Repeat::One { icon::REPEAT_1 } else { icon::REPEAT }, 17.0, label("Repeat", "Lặp lại"), repeat != Repeat::Off).clicked() {
            app.queue.repeat = repeat.next();
        }
    });
    ui.horizontal_wrapped(|ui| {
        let mut volume = app.audio.volume();
        if widgets::icon_button(ui, if volume == 0.0 { icon::VOLUME_X } else { icon::VOLUME_2 }, 16.0, label("Mute", "Tắt tiếng"), false).clicked() {
            super::mini::toggle_mute(app, ui);
            volume = app.audio.volume();
        }
        ui.spacing_mut().slider_width = 70.0;
        if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false)).on_hover_text(label("Volume", "Âm lượng")).changed() {
            app.audio.set_volume(volume);
        }
        if !in_room && widgets::icon_button(ui, icon::MIC_VOCAL, 17.0, label("Lyrics", "Lời bài hát"), false).clicked() {
            app.np_tab = NpTab::Lyrics;
            app.actions.push(Action::OpenNowPlaying(true));
        }
        if widgets::icon_button(ui, icon::PICTURE_IN_PICTURE, 16.0, label("Mini player", "Trình phát thu nhỏ"), false).clicked() {
            app.actions.push(Action::Mini(true));
        }
        if !in_room && widgets::icon_button(ui, icon::COMPASS, 16.0, label("Now playing", "Đang phát"), false).clicked() {
            app.actions.push(Action::OpenNowPlaying(true));
        }
        if let Some(track) = app.current() {
            let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
            let more = widgets::more_button(ui, rect, ui.scope_id().with("listener-menu"), label("Song options", "Tùy chọn bài hát"));
            let mut actions = Vec::new();
            widgets::menu_popup(&more).show(|ui| {
                widgets::song_menu(ui, track, app.liked.contains(&track.id), None, &mut actions);
            });
            app.actions.extend(actions);
        }
    });
}

fn room(app: &mut App, ui: &mut Ui, now: f64) {
    let rows = if ui.available_height() < 420.0 { 1 } else { 3 };
    ui.columns_const::<2, _>(|[left, right]| {
        listener(app, left, true);
        right.horizontal(|ui| {
            ui.label(RichText::new(label("Lyrics", "Lời bài hát")).font(theme::bold(20.0)));
            if widgets::icon_button(ui, icon::X, 17.0, label("Return to browse", "Quay lại trang nhạc"), false).clicked() {
                app.actions.push(Action::OpenNowPlaying(false));
            }
        });
        let queue_open = app.queue_open;
        if queue_open {
            right.add_space(10.0);
            shell::queue_drawer(app, right);
        } else {
            let height = (right.available_height() - 58.0 - rows as f32 * 46.0).max(64.0);
            let (rect, _) = right.allocate_exact_size(vec2(right.available_width(), height), Sense::hover());
            let mut lyrics = right.new_child(egui::UiBuilder::new().max_rect(rect));
            lyrics.set_clip_rect(lyrics.clip_rect().intersect(rect));
            nowplaying::lyrics_panel(app, &mut lyrics, rect, now, false);
            right.separator();
            upcoming(app, right, rows, false);
        }
    });
}

// ==========================================================================
// Rev0.6 — four more layouts. They keep their own frame: a header, a body and
// the player bar, with the pages, the queue and the lyrics drawn inside.
// ==========================================================================

/// A header row of the window's own: the search bar, then whatever the layout adds.
fn top_area(app: &mut App, ui: &mut Ui, choice: Choice, height: f32, add: impl FnOnce(&mut App, &mut Ui)) {
    let top = super::titlebar::top_margin(8, 40.0) + super::titlebar::below_lights(ui.ctx()) as i8;
    egui::Panel::top("layout-header")
        .resizable(false)
        .exact_size(height)
        .frame(panel(choice, Margin { left: 14, top, right: 10, bottom: 8 }))
        .show_separator_line(false)
        .show(ui, |ui| add(app, ui));
}

/// A soft light in the playing cover's colour: rings of low alpha, so it fades out
/// instead of showing an edge. Two of them, as the stage's light.
fn ambient(ui: &Ui, rect: Rect, color: Color32, strength: f32) {
    const STEPS: usize = 14;
    let spots = [
        (egui::pos2(rect.left() + rect.width() * 0.2, rect.top() + rect.height() * 0.06), rect.width().max(rect.height()) * 0.46),
        (egui::pos2(rect.right() - rect.width() * 0.1, rect.bottom() - rect.height() * 0.1), rect.width().max(rect.height()) * 0.34),
    ];
    for (i, (at, r0)) in spots.into_iter().enumerate() {
        let mut r = r0;
        for step in 0..STEPS {
            let fade = 1.0 - step as f32 / STEPS as f32;
            let a = strength * fade * fade / STEPS as f32 * if i == 0 { 1.0 } else { 0.7 };
            ui.painter().circle_filled(at, r, theme::with_alpha(color, a));
            r *= 0.88;
        }
    }
}

/// A sidebar row: icon, label, an optional count at the right.
fn nav_row(ui: &mut Ui, glyph: char, text: &str, selected: bool, accent: Color32, badge: Option<usize>, icon_only: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let resp = widgets::named(resp, text).on_hover_cursor(egui::CursorIcon::PointingHand);
    if selected {
        ui.painter().rect_filled(rect, 8.0, theme::with_alpha(accent, 0.16));
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 8.0, theme::glass(0.06));
    }
    let ink = if selected { accent } else { theme::text_dim() };
    if icon_only {
        widgets::icon_at(ui, rect.center(), glyph, 18.0, ink);
    } else {
        widgets::icon_at(ui, egui::pos2(rect.left() + 16.0, rect.center().y), glyph, 17.0, ink);
        let font = if selected { theme::semibold(13.5) } else { theme::regular(13.5) };
        let width = (rect.width() - if badge.is_some() { 58.0 } else { 38.0 }).max(20.0);
        widgets::text_at(ui, egui::pos2(rect.left() + 34.0, rect.center().y - 8.0), text, font, if selected { theme::text() } else { theme::text_dim() }, width);
        if let Some(n) = badge {
            ui.painter().text(egui::pos2(rect.right() - 10.0, rect.center().y), egui::Align2::RIGHT_CENTER, n.to_string(), theme::regular(11.0), theme::text_faint());
        }
    }
    widgets::focus_ring(ui, &resp, rect, 8.0);
    resp.clicked()
}

fn group_label(ui: &mut Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(text).font(theme::semibold(10.0)).color(theme::text_faint()));
    ui.add_space(3.0);
}

/// The lyrics of the playing song, in `rect`: a hint while they load or when there are none.
/// On a light page they go on a dark card, since the lyrics' own colours are made for one.
fn lyrics_in(app: &mut App, ui: &mut Ui, rect: Rect, now: f64) {
    if app.current().is_none() {
        ui.label(RichText::new(label("Choose a song to see its lyrics", "Chọn một bài để xem lời")).color(theme::text_dim()));
        return;
    }
    if app.lyrics.is_none() {
        ui.horizontal(|ui| {
            widgets::loader(ui);
            ui.label(RichText::new(label("Loading lyrics…", "Đang tải lời…")).color(theme::text_dim()));
        });
        return;
    }
    let was_light = theme::light();
    if was_light {
        ui.painter().rect_filled(rect, 12.0, Color32::from_rgb(17, 17, 20));
        theme::set_mode(ui.ctx(), false);
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(10.0, 6.0))));
    child.set_clip_rect(child.clip_rect().intersect(rect));
    nowplaying::lyrics_panel(app, &mut child, rect.shrink2(vec2(10.0, 6.0)), now, true);
    if was_light {
        theme::set_mode(ui.ctx(), true);
    }
}

fn lyrics_block(app: &mut App, ui: &mut Ui, now: f64) {
    let rect = ui.available_rect_before_wrap();
    lyrics_in(app, ui, rect, now);
}

// ---------------------------------------------------------------- Aurora Stage

fn aurora(app: &mut App, ui: &mut Ui, now: f64) {
    let compact = ui.available_width() < 900.0;
    let glow = app.cover_colours().map(|a| a.accent).unwrap_or_else(|| app.accent_color());
    let strength = if theme::light() { 0.24 } else { 0.5 };

    egui::Panel::bottom("layout-player")
        .resizable(false)
        .exact_size(app.bar_height())
        .frame(panel(Choice::Aurora, Margin::ZERO))
        .show_separator_line(false)
        .show(ui, |ui| shell::player_bar(app, ui, now));

    top_area(app, ui, Choice::Aurora, 52.0, |app, ui| shell::top_bar(app, ui));

    egui::Panel::left("aurora-nav")
        .resizable(false)
        .exact_size(if compact { 72.0 } else { 214.0 })
        .frame(panel(Choice::Aurora, Margin::symmetric(10, 12)))
        .show_separator_line(false)
        .show(ui, |ui| aurora_nav(app, ui, compact));

    if app.aurora_right && !compact {
        egui::Panel::right("aurora-right")
            .resizable(false)
            .exact_size(286.0)
            .frame(panel(Choice::Aurora, Margin::symmetric(14, 12)))
            .show_separator_line(false)
            .show(ui, |ui| aurora_right(app, ui, now));
    }

    egui::CentralPanel::default()
        .frame(Frame::new().inner_margin(Margin::symmetric(18, 10)))
        .show(ui, |ui| {
            app.page_insets = [0.0, 0.0];
            let screen = ui.max_rect();
            ambient(ui, screen, glow, strength);
            if !compact && app.page != Page::Settings {
                hero_card(app, ui);
                ui.add_space(10.0);
            }
            ui.scope_builder(egui::UiBuilder::new().scope_id(egui::Id::unique("main-page")), |ui| pages::page(app, ui, now));
        });
}

fn aurora_nav(app: &mut App, ui: &mut Ui, compact: bool) {
    let accent = app.accent_color();
    let page = app.page.clone();
    let count = app.mixed_playlist.len();
    ui.label(RichText::new(if compact { "AS" } else { "Aurora Stage" }).font(theme::bold(if compact { 14.0 } else { 16.0 })).color(theme::text()));
    if !compact {
        group_label(ui, label("Discover", "Khám phá"));
    } else {
        ui.add_space(10.0);
    }
    if nav_row(ui, icon::HOUSE, "YouTube", page == Page::Home, accent, None, compact) {
        app.actions.push(Action::Go(Page::Home));
    }
    if !compact {
        group_label(ui, label("Charts", "Bảng xếp hạng"));
    }
    for source in [Catalog::Zing, Catalog::Nct] {
        let selected = matches!(&page, Page::CatalogHome(active) | Page::CatalogSearch(active, _) if *active == source);
        if nav_row(ui, icon::MUSIC, source.name(), selected, accent, None, compact) {
            app.actions.push(Action::Go(Page::CatalogHome(source)));
        }
    }
    if !compact {
        group_label(ui, label("Library", "Thư viện"));
    }
    if nav_row(ui, icon::LIST_MUSIC, label("Mixed playlist", "Playlist tổng hợp"), matches!(page, Page::MixedPlaylist | Page::MixedSearch(_)), accent, Some(count), compact) {
        app.actions.push(Action::Go(Page::MixedPlaylist));
    }
    ui.add_space(8.0);
    if nav_row(ui, icon::SETTINGS, label("Settings", "Cài đặt"), page == Page::Settings, accent, None, compact) {
        app.actions.push(Action::Go(Page::Settings));
    }
}

fn aurora_right(app: &mut App, ui: &mut Ui, now: f64) {
    let tab = app.aurora_tab;
    ui.horizontal(|ui| {
        if widgets::chip(ui, label("Up next", "Sắp phát"), tab == 0).clicked() {
            app.aurora_tab = 0;
        }
        if widgets::chip(ui, label("Lyrics", "Lời bài hát"), tab == 1).clicked() {
            app.aurora_tab = 1;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, icon::X, 15.0, label("Close", "Đóng"), false).clicked() {
                app.aurora_right = false;
            }
        });
    });
    ui.add_space(10.0);
    if tab == 0 {
        upcoming(app, ui, 12, false);
    } else {
        lyrics_block(app, ui, now);
    }
}

/// The stage's card: the song playing, or the first of the queue, with its cover.
fn hero_card(app: &mut App, ui: &mut Ui) {
    let Some(track) = app.current().cloned().or_else(|| app.queue.tracks.first().cloned()) else { return };
    let accent = app.accent_color();
    let playing = app.playing();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 118.0), Sense::hover());
    ui.painter().rect_filled(rect, 14.0, theme::with_alpha(accent, 0.10));
    ui.painter().rect_stroke(rect, 14.0, egui::Stroke::new(1.0, theme::glass(0.10)), egui::StrokeKind::Inside);
    let cover = Rect::from_min_size(rect.min + vec2(14.0, 13.0), egui::Vec2::splat(92.0));
    widgets::paint_cover(ui, cover, &track.thumbs, 10.0, 1.0);
    let body = Rect::from_min_max(egui::pos2(cover.right() + 14.0, rect.top() + 12.0), egui::pos2(rect.right() - 16.0, rect.bottom() - 10.0));
    let mut card = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    card.label(RichText::new(label("Today's pick", "Đề xuất hôm nay")).font(theme::semibold(10.5)).color(accent));
    card.add_space(2.0);
    card.add(egui::Label::new(RichText::new(&track.title).font(theme::bold(21.0)).color(theme::text())).truncate());
    card.label(RichText::new(track.artist_line()).font(theme::regular(12.5)).color(theme::text_dim()));
    card.add_space(8.0);
    card.horizontal(|ui| {
        let text = if playing { label("Playing", "Đang phát") } else { label("Play now", "Phát ngay") };
        if widgets::pill(ui, text, Some(if playing { icon::PAUSE } else { icon::PLAY }), true).clicked() {
            app.actions.push(Action::TogglePlay);
        }
        if widgets::pill(ui, label("Lyrics", "Xem lời"), Some(icon::MIC_VOCAL), false).clicked() {
            app.aurora_tab = 1;
            app.aurora_right = true;
        }
    });
}

// --------------------------------------------------------------- Studio Console

fn console(app: &mut App, ui: &mut Ui, now: f64) {
    let compact = ui.available_width() < 1000.0;

    egui::Panel::bottom("layout-player")
        .resizable(false)
        .exact_size(app.bar_height())
        .frame(panel(Choice::Console, Margin::ZERO))
        .show_separator_line(false)
        .show(ui, |ui| shell::player_bar(app, ui, now));

    egui::Panel::bottom("console-wave")
        .resizable(false)
        .exact_size(36.0)
        .frame(panel(Choice::Console, Margin::symmetric(18, 2)))
        .show_separator_line(false)
        .show(ui, |ui| waveform(app, ui));

    top_area(app, ui, Choice::Console, 46.0, |app, ui| shell::top_bar(app, ui));

    egui::Panel::left("console-nav")
        .resizable(false)
        .exact_size(if compact { 64.0 } else { 186.0 })
        .frame(panel(Choice::Console, Margin::symmetric(10, 12)))
        .show_separator_line(false)
        .show(ui, |ui| console_nav(app, ui, compact));

    if !compact {
        egui::Panel::right("console-inspector")
            .resizable(false)
            .exact_size(300.0)
            .frame(panel(Choice::Console, Margin::symmetric(14, 12)))
            .show_separator_line(false)
            .show(ui, |ui| console_inspector(app, ui, now));
    }

    egui::CentralPanel::default()
        .frame(Frame::new().inner_margin(Margin::symmetric(14, 8)))
        .show(ui, |ui| {
            app.page_insets = [0.0, 0.0];
            ui.scope_builder(egui::UiBuilder::new().scope_id(egui::Id::unique("main-page")), |ui| pages::page(app, ui, now));
        });
}

fn console_nav(app: &mut App, ui: &mut Ui, compact: bool) {
    let accent = app.accent_color();
    let page = app.page.clone();
    let count = app.mixed_playlist.len();
    ui.label(RichText::new(if compact { "SC" } else { "Studio Console" }).font(theme::bold(if compact { 13.0 } else { 14.0 })).color(theme::text()));
    if !compact {
        group_label(ui, label("Catalog", "Kho nhạc"));
    } else {
        ui.add_space(10.0);
    }
    if nav_row(ui, icon::HOUSE, "YouTube", page == Page::Home, accent, None, compact) {
        app.actions.push(Action::Go(Page::Home));
    }
    if nav_row(ui, icon::LIST_MUSIC, label("Mixed playlist", "Playlist tổng hợp"), matches!(page, Page::MixedPlaylist | Page::MixedSearch(_)), accent, Some(count), compact) {
        app.actions.push(Action::Go(Page::MixedPlaylist));
    }
    if !compact {
        group_label(ui, label("Sources", "Nguồn"));
    }
    for source in [Catalog::Zing, Catalog::Nct] {
        let selected = matches!(&page, Page::CatalogHome(active) | Page::CatalogSearch(active, _) if *active == source);
        if nav_row(ui, icon::MUSIC, source.name(), selected, accent, None, compact) {
            app.actions.push(Action::Go(Page::CatalogHome(source)));
        }
    }
    ui.add_space(8.0);
    if nav_row(ui, icon::SETTINGS, label("Settings", "Cài đặt"), page == Page::Settings, accent, None, compact) {
        app.actions.push(Action::Go(Page::Settings));
    }
}

/// The fixed right column: what's playing, how it's playing, its lyrics and the queue.
fn console_inspector(app: &mut App, ui: &mut Ui, now: f64) {
    let accent = app.accent_color();
    ui.label(RichText::new(label("Now playing", "Bảng Đang Phát")).font(theme::semibold(10.0)).color(theme::text_faint()));
    ui.add_space(6.0);
    let Some(track) = app.current().cloned() else {
        ui.label(RichText::new(label("Nothing playing", "Chưa phát bài nào")).color(theme::text_dim()));
        return;
    };
    ui.horizontal(|ui| {
        let (cover, _) = ui.allocate_exact_size(egui::Vec2::splat(78.0), Sense::hover());
        widgets::paint_cover(ui, cover, &track.thumbs, 6.0, 1.0);
        ui.add_space(2.0);
        ui.vertical(|ui| {
            let width = ui.available_width();
            ui.add(egui::Label::new(RichText::new(&track.title).font(theme::semibold(13.5))).truncate());
            ui.add(egui::Label::new(RichText::new(track.artist_line()).font(theme::regular(11.5)).color(theme::text_dim())).truncate());
            if let Some((source, _)) = Catalog::split_track(&track.id) {
                ui.label(RichText::new(source.name()).font(theme::regular(10.5)).color(accent));
            }
            let _ = width;
        });
    });
    ui.add_space(8.0);
    // The output, as the engine runs it: device, buffer, speed and level.
    let api = if cfg!(windows) { "WASAPI" } else { "Audio" };
    let ms = app.audio.status.latency().as_millis();
    let speed = app.audio.status.speed();
    let level = app.audio.status.level();
    let db = if level > 0.0001 { 20.0 * level.log10() } else { -60.0 };
    let device = app.output_device.as_ref().map(|(_, name)| name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| label("Default output", "Thiết bị mặc định").to_owned());
    ui.label(RichText::new(format!("{api} · {ms} ms · {speed:.2}×")).font(theme::regular(10.5)).color(theme::text_dim()));
    ui.add(egui::Label::new(RichText::new(format!("{device} · {db:.0} dB")).font(theme::regular(10.5)).color(theme::text_faint())).truncate());
    ui.add_space(8.0);
    ui.separator();
    ui.label(RichText::new(label("Synced lyrics", "Lời đồng bộ")).font(theme::semibold(10.0)).color(theme::text_faint()));
    ui.add_space(4.0);
    let lyrics_h = (ui.available_height() * 0.46).clamp(110.0, 320.0);
    let (lyrics_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), lyrics_h), Sense::hover());
    lyrics_in(app, ui, lyrics_rect, now);
    ui.add_space(6.0);
    ui.separator();
    ui.label(RichText::new(label("Up next", "Sắp phát")).font(theme::semibold(10.0)).color(theme::text_faint()));
    ui.add_space(2.0);
    upcoming(app, ui, 6, false);
}

/// A 36-bar waveform of the playing song: its shape is the same every time, its fill is
/// the position. A click seeks there.
fn waveform(app: &mut App, ui: &mut Ui) {
    const H: [f32; 36] = [
        28.0, 45.0, 70.0, 52.0, 85.0, 60.0, 40.0, 92.0, 74.0, 50.0, 64.0, 88.0, 95.0, 58.0, 44.0, 78.0, 86.0, 62.0, 48.0, 72.0, 90.0, 66.0,
        54.0, 80.0, 94.0, 68.0, 46.0, 76.0, 84.0, 58.0, 42.0, 65.0, 82.0, 52.0, 38.0, 60.0,
    ];
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    let accent = app.accent_color();
    let total_secs = app.current().and_then(|t| t.duration).map(|d| d as f32).unwrap_or(0.0);
    let pos = app.audio.status.position_smooth().as_secs_f32();
    let ratio = if total_secs > 0.0 { (pos / total_secs).clamp(0.0, 1.0) } else { 0.0 };
    let gap = 3.0;
    let bars = H.len() as f32;
    let bar_w = ((rect.width() - gap * (bars - 1.0)) / bars).clamp(2.0, 6.0);
    let total = bar_w * bars + gap * (bars - 1.0);
    let left = rect.left() + ((rect.width() - total) / 2.0).max(0.0);
    for (i, h) in H.iter().enumerate() {
        let height = (rect.height() - 6.0) * (h / 100.0);
        let x = left + i as f32 * (bar_w + gap);
        let bar = Rect::from_min_size(egui::pos2(x, rect.center().y - height / 2.0), vec2(bar_w, height));
        let on = (i as f32 + 0.5) / bars <= ratio;
        let color = if on { accent } else { theme::glass(0.16) };
        ui.painter().rect_filled(bar, bar_w / 2.0, color);
    }
    if resp.clicked()
        && total > 0.0
        && let Some(p) = resp.interact_pointer_pos()
    {
        let r = ((p.x - left) / total).clamp(0.0, 1.0);
        app.actions.push(Action::Seek(r * total_secs));
    }
}

// --------------------------------------------------------------- Velvet Gallery

fn velvet(app: &mut App, ui: &mut Ui, now: f64) {
    let compact = ui.available_width() < 900.0;
    let accent = app.accent_color();

    // The dock: a rounded bar with room around it, floating over the gallery.
    egui::Panel::bottom("layout-player")
        .resizable(false)
        .exact_size(app.bar_height() + 22.0)
        .frame(Frame::new().fill(Color32::TRANSPARENT).inner_margin(Margin { left: 18, right: 18, top: 0, bottom: 10 }))
        .show_separator_line(false)
        .show(ui, |ui| {
            let r = ui.max_rect();
            widgets::soft_shadow(ui, r, 30.0, 16.0, 0.30);
            // A dock has to read as one, in either mode: a shade off the page, not the page.
            let fill = theme::lerp_color(surface(Choice::Velvet), theme::text(), if theme::light() { 0.06 } else { 0.07 });
            ui.painter().rect_filled(r, 30.0, fill);
            ui.painter().rect_stroke(r, 30.0, egui::Stroke::new(1.0, theme::glass(0.12)), egui::StrokeKind::Inside);
            shell::player_bar(app, ui, now);
        });

    top_area(app, ui, Choice::Velvet, if compact { 76.0 } else { 84.0 }, |app, ui| {
        shell::top_bar(app, ui);
        ui.add_space(4.0);
        navigation(app, ui, true, false);
    });

    if app.velvet_room {
        egui::Panel::right("velvet-room")
            .resizable(false)
            .exact_size(if compact { 300.0 } else { 356.0 })
            .frame(panel(Choice::Velvet, Margin::symmetric(16, 12)))
            .show_separator_line(false)
            .show(ui, |ui| velvet_room(app, ui, now));
    }

    egui::CentralPanel::default()
        .frame(Frame::new().inner_margin(Margin::symmetric(22, 10)))
        .show(ui, |ui| {
            app.page_insets = [0.0, 0.0];
            if !compact {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Velvet").font(theme::bold(22.0)).color(accent));
                    ui.label(RichText::new(label("Gallery", "Bộ sưu tập")).font(theme::regular(12.0)).color(theme::text_faint()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let text = label("Listening room", "Phòng nghe");
                        if widgets::pill(ui, text, Some(icon::HEADPHONES), app.velvet_room).clicked() {
                            app.velvet_room = !app.velvet_room;
                        }
                    });
                });
                ui.add_space(6.0);
            }
            ui.scope_builder(egui::UiBuilder::new().scope_id(egui::Id::unique("main-page")), |ui| pages::page(app, ui, now));
        });
}

fn velvet_room(app: &mut App, ui: &mut Ui, now: f64) {
    let accent = app.accent_color();
    ui.horizontal(|ui| {
        ui.label(RichText::new(label("Listening room", "Phòng nghe")).font(theme::bold(19.0)).color(accent));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, icon::X, 15.0, label("Close", "Đóng"), false).clicked() {
                app.velvet_room = false;
            }
        });
    });
    ui.add_space(8.0);
    if let Some(track) = app.current().cloned() {
        ui.horizontal(|ui| {
            let (cover, _) = ui.allocate_exact_size(egui::Vec2::splat(84.0), Sense::hover());
            widgets::paint_cover(ui, cover, &track.thumbs, 12.0, 1.0);
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.add(egui::Label::new(RichText::new(&track.title).font(theme::bold(17.0))).truncate());
                ui.add(egui::Label::new(RichText::new(track.artist_line()).font(theme::regular(12.0)).color(theme::text_dim())).truncate());
                if let Some((source, _)) = Catalog::split_track(&track.id) {
                    ui.label(RichText::new(source.name()).font(theme::regular(10.5)).color(accent));
                }
            });
        });
        ui.add_space(10.0);
    }
    let tab = app.velvet_tab;
    ui.horizontal(|ui| {
        if widgets::chip(ui, label("Synced lyrics", "Lời đồng bộ"), tab == 0).clicked() {
            app.velvet_tab = 0;
        }
        if widgets::chip(ui, label("Up next", "Sắp phát"), tab == 1).clicked() {
            app.velvet_tab = 1;
        }
    });
    ui.add_space(8.0);
    if tab == 0 {
        lyrics_block(app, ui, now);
    } else {
        upcoming(app, ui, 10, false);
    }
}

// ------------------------------------------------------------------ Feather Lite

fn feather(app: &mut App, ui: &mut Ui, now: f64) {
    // The one layout that caps covers: fetched and kept at 80 px at most (see `paint_cover`).
    widgets::set_cover_px_cap(ui, Some(80));
    let compact = ui.available_width() < 860.0;

    egui::Panel::bottom("layout-player")
        .resizable(false)
        .exact_size(app.bar_height())
        .frame(panel(Choice::Feather, Margin::ZERO))
        .show_separator_line(false)
        .show(ui, |ui| shell::player_bar(app, ui, now));

    top_area(app, ui, Choice::Feather, 126.0, |app, ui| {
        shell::top_bar(app, ui);
        ui.add_space(3.0);
        feather_navigation(app, ui);
        ui.add_space(3.0);
        feather_tools(app, ui);
    });

    if app.feather_drawer != 0 && !compact {
        egui::Panel::right("feather-drawer")
            .resizable(false)
            .exact_size(268.0)
            .frame(panel(Choice::Feather, Margin::symmetric(12, 10)))
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let title = if app.feather_drawer == 1 { label("Up next", "Sắp phát") } else { label("Synced lyrics", "Lời đồng bộ") };
                    ui.label(RichText::new(title).font(theme::semibold(12.0)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, icon::X, 14.0, label("Close", "Đóng"), false).clicked() {
                            app.feather_drawer = 0;
                        }
                    });
                });
                ui.add_space(8.0);
                if app.feather_drawer == 1 {
                    upcoming(app, ui, 10, false);
                } else {
                    lyrics_block(app, ui, now);
                }
            });
    }

    egui::CentralPanel::default()
        .frame(Frame::new().inner_margin(Margin::symmetric(14, 6)))
        .show(ui, |ui| {
            app.page_insets = [0.0, 0.0];
            ui.scope_builder(egui::UiBuilder::new().scope_id(egui::Id::unique("main-page")), |ui| pages::page(app, ui, now));
        });
}

/// Feather's own destinations: the shared navigation is hidden in this layout, so the
/// pages need visible tabs here too. Chips keep the flat look and work in narrow windows.
fn feather_navigation(app: &mut App, ui: &mut Ui) {
    let page = app.page.clone();
    let count = app.mixed_playlist.len();
    let mixed = matches!(page, Page::MixedPlaylist | Page::MixedSearch(_));
    let go = |app: &mut App, ui: &mut Ui, text: &str, selected: bool, page: Page| {
        if widgets::chip(ui, text, selected).clicked() {
            app.actions.push(Action::Go(page));
        }
    };
    ui.horizontal_wrapped(|ui| {
        go(app, ui, "YouTube", page == Page::Home, Page::Home);
        go(app, ui, "Zing MP3", matches!(page, Page::CatalogHome(Catalog::Zing) | Page::CatalogSearch(Catalog::Zing, _)), Page::CatalogHome(Catalog::Zing));
        go(app, ui, "NhacCuaTui", matches!(page, Page::CatalogHome(Catalog::Nct) | Page::CatalogSearch(Catalog::Nct, _)), Page::CatalogHome(Catalog::Nct));
        go(app, ui, &format!("{} ({count})", label("Mixed playlist", "Playlist tổng hợp")), mixed, Page::MixedPlaylist);
        go(app, ui, label("Settings", "Cài đặt"), page == Page::Settings, Page::Settings);
    });
}

fn feather_tools(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if widgets::chip(ui, label("Up next", "Sắp phát"), app.feather_drawer == 1).clicked() {
            app.feather_drawer = if app.feather_drawer == 1 { 0 } else { 1 };
        }
        if widgets::chip(ui, label("Lyrics", "Lời"), app.feather_drawer == 2).clicked() {
            app.feather_drawer = if app.feather_drawer == 2 { 0 } else { 2 };
        }
    });
}

