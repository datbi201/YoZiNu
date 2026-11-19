//! Browse, search and see the shared play queue on catalog pages.

use egui::{Color32, Rect, RichText, Sense, Ui, Vec2, pos2, vec2};

use crate::backend::Req;
use crate::catalog::Catalog;
use crate::model::{Target, Track};
use crate::theme;

use super::{Action, App, CatalogState, Page, Repeat, fmt_time, pages, widgets};

impl App {
    fn catalog_state(&self, source: Catalog) -> &CatalogState {
        match source {
            Catalog::Zing => &self.zing,
            Catalog::Nct => &self.nct,
        }
    }

    pub(super) fn catalog_state_mut(&mut self, source: Catalog) -> &mut CatalogState {
        match source {
            Catalog::Zing => &mut self.zing,
            Catalog::Nct => &mut self.nct,
        }
    }

    pub(super) fn load_catalog_home(&mut self, source: Catalog) {
        let state = self.catalog_state_mut(source);
        if !state.home_loading {
            state.home_loading = true;
            state.home_failed = false;
            self.backend.request(Req::CatalogHome(source));
        }
    }

    pub(super) fn load_catalog_search(&mut self, source: Catalog, query: &str) {
        let state = self.catalog_state_mut(source);
        if state.loading && state.searched == query {
            return;
        }
        state.searched = query.to_owned();
        state.loading = true;
        state.search_failed = false;
        state.results.clear();
        self.backend.request(Req::CatalogSearch { source, query: query.to_owned() });
    }
}

pub(super) fn docked_queue(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() >= 1100.0
}

pub(super) fn page(app: &mut App, ui: &mut Ui, now: f64, source: Catalog) {
    pages::page_title(ui, source.name());
    ui.add_space(12.0);

    if !docked_queue(ui.ctx()) && !app.queue_open && app.queue.current().is_some() {
        ui.add_space(14.0);
        egui::CollapsingHeader::new(RichText::new(crate::i18n::label("Up next", "Sắp phát")).font(theme::semibold(16.0)))
            .id_salt("catalog-queue-inline")
            .default_open(true)
            .show(ui, |ui| queue_rows(app, ui, 210.0));
    }

    let size = (ui.available_width() / 5.4).clamp(116.0, 166.0);
    let current = app.current_id().map(str::to_owned);
    let playing = app.playing();
    let state = app.catalog_state(source);
    let mut picked = None;
    let mut added = None;
    let mut retry_page = false;
    if matches!(app.page, Page::CatalogSearch(..)) {
        widgets::shelf_header(ui, crate::i18n::label("Search results", "Kết quả tìm kiếm"), None, None, false);
        if state.loading {
            widgets::skeleton(ui, now, 1, size);
        } else if state.search_failed {
            retry_page = retry(ui, crate::i18n::label("Could not load search results.", "Không tải được kết quả tìm kiếm."));
        } else if state.results.is_empty() {
            ui.label(RichText::new(crate::i18n::label("No songs found.", "Không tìm thấy bài hát.")).color(theme::text_dim()));
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(16.0, 14.0);
                for track in state.results.iter().filter(|t| !app.hide_explicit || !t.explicit) {
                    ui.vertical(|ui| {
                        ui.set_width(size);
                        let (play, add) = song_card_actions(ui, track, size, current.as_deref(), playing);
                        if play { picked = Some(track.clone()); }
                        if add { added = Some(track.clone()); }
                    });
                }
            });
        }
    } else {
        if state.home_failed {
            ui.add_space(22.0);
            retry_page = retry(ui, &format!("Không tải được trang chủ {}.", source.name()));
        }
        if let Some(shelves) = &state.home {
            for shelf in shelves {
                let tracks: Vec<_> = shelf.tracks.iter().filter(|t| !app.hide_explicit || !t.explicit).collect();
                if tracks.is_empty() {
                    continue;
                }
                let arrows = tracks.len() as f32 * (size + 16.0) - 16.0 > ui.available_width();
                let (left, right) = widgets::shelf_header(ui, &shelf.title, None, None, arrows);
                let nudge = i32::from(right) - i32::from(left);
                widgets::h_scroll(ui, &shelf.title, nudge, tracks.len(), size, 16.0, |ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    for track in tracks {
                        ui.vertical(|ui| {
                            ui.set_width(size);
                            let (play, add) = song_card_actions(ui, track, size, current.as_deref(), playing);
                            if play { picked = Some(track.clone()); }
                            if add { added = Some(track.clone()); }
                        });
                    }
                });
            }
        } else if state.home_loading {
            widgets::skeleton(ui, now, 2, size);
        }
    }
    if retry_page {
        app.actions.push(Action::Go(app.page.clone()));
    }
    if let Some(track) = added {
        app.actions.push(Action::AddToMixed(track));
    }
    if let Some(track) = picked {
        app.actions.push(Action::Open(Target::Song(track)));
    }
}

fn retry(ui: &mut Ui, message: &str) -> bool {
    ui.label(RichText::new(message).color(theme::text_dim()));
    widgets::pill(ui, crate::i18n::label("Retry", "Thử lại"), None, false).clicked()
}

fn song_card_actions(ui: &mut Ui, track: &Track, size: f32, current: Option<&str>, playing: bool) -> (bool, bool) {
    let play = song_card(ui, track, size, current, playing);
    let add = ui.small_button(crate::i18n::label("+ Add to playlist", "+ Thêm vào playlist"))
        .on_hover_text(crate::i18n::label("Save to the mixed playlist and Up next", "Lưu vào playlist tổng hợp và thêm vào Sắp phát")).clicked();
    (play, add)
}

fn song_card(ui: &mut Ui, track: &Track, size: f32, current: Option<&str>, playing: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size + 62.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let response = widgets::named(response, &track.title).on_hover_cursor(egui::CursorIcon::PointingHand);
    let active = current == Some(track.id.as_str());
    let cover = Rect::from_min_size(rect.min, Vec2::splat(size));
    let radius = theme::card_radius();
    widgets::paint_cover(ui, cover, &track.thumbs, radius, 1.0);
    widgets::focus_ring(ui, &response, cover, radius);
    if active || response.hovered() || response.has_focus() {
        let center = cover.right_bottom() - vec2(25.0, 25.0);
        ui.painter().circle_filled(center, 19.0, theme::shade(0.78));
        widgets::play_pause_glyph(ui, center, 16.0, Color32::WHITE, active && playing);
    }
    let title = widgets::galley_wrapped(ui, &track.title, theme::semibold(14.0), theme::text(), size, 2);
    let title_y = cover.bottom() + 8.0;
    let artist_y = title_y + title.size().y + 3.0;
    ui.painter().galley(pos2(rect.left(), title_y), title, theme::text());
    let artists = track.artist_line();
    widgets::text_at(ui, pos2(rect.left(), artist_y), &artists, theme::regular(12.5), theme::text_dim(), size);
    response.on_hover_text(format!("{} · {artists}", track.title)).clicked()
}

pub(super) fn queue_panel(app: &mut App, ui: &mut Ui) {
    ui.add_space(48.0);
    ui.label(RichText::new(crate::i18n::label("Up next", "Sắp phát")).font(theme::bold(23.0)).color(theme::text()));
    ui.add_space(12.0);
    let height = ui.available_height();
    queue_rows(app, ui, height);
}

fn queue_rows(app: &mut App, ui: &mut Ui, height: f32) {
    let Some(index) = app.queue.index.filter(|&i| i < app.queue.tracks.len()) else {
        ui.label(RichText::new("Chọn một bài để bắt đầu.").color(theme::text_dim()));
        return;
    };
    let repeat = app.queue.repeat;
    let len = app.queue.tracks.len();
    if repeat == Repeat::One || (repeat == Repeat::All && len == 1) {
        ui.label(RichText::new("Đang lặp lại bài hiện tại.").color(theme::text_dim()));
        return;
    }
    let wrap = if repeat == Repeat::All { index } else { 0 };
    let upcoming = len - index - 1 + wrap;
    if upcoming == 0 {
        let pending = app.catalog_recommending.as_deref() == app.current_id();
        ui.label(RichText::new(if pending { "Đang tải bài gợi ý…" } else { "Chưa có bài tiếp theo." }).color(theme::text_dim()));
        return;
    }
    let id = ui.scope_id().with("catalog-queue-head");
    let current = (index, app.queue.tracks[index].id.clone());
    let mut area = egui::ScrollArea::vertical().id_salt("catalog-upcoming-tracks").max_height(height).auto_shrink([false, true]);
    if ui.data(|data| data.get_temp::<(usize, String)>(id)).as_ref() != Some(&current) {
        ui.data_mut(|data| data.insert_temp(id, current));
        area = area.vertical_scroll_offset(0.0);
    }
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        area.show_rows(ui, 50.0, upcoming, |ui, rows| {
            for offset in rows {
                let index = (index + 1 + offset) % len;
                if queue_row(ui, &app.queue.tracks[index]) {
                    app.actions.push(Action::QueueJump(index));
                }
            }
        });
    });
}

fn queue_row(ui: &mut Ui, track: &Track) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::click());
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let response = widgets::named(response, &track.title).on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_filled(rect, 9.0, theme::glass(0.09));
    }
    widgets::focus_ring(ui, &response, rect, 9.0);
    let cover = Rect::from_min_size(rect.min + vec2(5.0, 5.0), Vec2::splat(40.0));
    widgets::paint_cover(ui, cover, &track.thumbs, theme::thumb_radius(), 1.0);
    let x = cover.right() + 9.0;
    let duration = track.duration.map(fmt_time).unwrap_or_default();
    let width = (rect.right() - x - if duration.is_empty() { 5.0 } else { 48.0 }).max(0.0);
    widgets::text_at(ui, pos2(x, rect.top() + 7.0), &track.title, theme::semibold(13.0), theme::text(), width);
    widgets::text_at(ui, pos2(x, rect.top() + 27.0), &track.artist_line(), theme::regular(12.0), theme::text_dim(), width);
    ui.painter().text(rect.right_center() - vec2(7.0, 0.0), egui::Align2::RIGHT_CENTER, duration, theme::regular(11.5), theme::text_dim());
    response.on_hover_text(format!("Phát {}", track.title)).clicked()
}
