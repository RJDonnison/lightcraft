//! Android gallery presentation. The platform owns permission checks and paged MediaStore queries;
//! browsing never copies an original or creates a catalog entry.

use egui::{Color32, Rect, Sense, TextureHandle, TextureOptions, pos2, vec2};
use lightcraft_catalog::PhotoId;
use lightcraft_engine::Selection;
use serde_json::json;

use crate::{DevicePage, LightcraftApp, state::ViewMode, widgets::register};

const PAGE_SIZE: usize = 48;

enum Notice {
    Permission,
    Failed(String),
}

struct Item {
    id: String,
    texture: Option<TextureHandle>,
    preview_available: bool,
}

#[derive(Default)]
pub struct State {
    items: Vec<Item>,
    requested_offset: usize,
    requested: bool,
    loaded: bool,
    more: bool,
    opening: bool,
    notice: Option<Notice>,
}

pub fn tabs(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let tab_width = ((ui.available_width() - 12.0) / 3.0).max(44.0);
        for (label, device, library) in [("Device Photos", true, false), ("My Edits", false, false), ("Library", false, true)] {
            let selected = if library { app.ui.library_home } else { !app.ui.library_home && app.ui.device_gallery == device };
            let response = ui.add_sized([tab_width, 44.0], egui::Button::new(label).selected(selected));
            register(ui.ctx(), format!("galleryTab:{label}"), response.rect);
            if response.clicked() {
                if library {
                    app.ui.device_gallery = false;
                    app.ui.library_home = true;
                } else if device {
                    if !app.ui.device_gallery {
                        app.caches.device_photos = State::default();
                    }
                    app.ui.device_gallery = true;
                    app.ui.library_home = false;
                    app.ui.view = ViewMode::PhotoGrid;
                } else if let Err(err) = app.run("library.source", json!({"kind":"myEdits"})) {
                    app.toast(ui.ctx(), err);
                } else {
                    app.ui.device_gallery = false;
                    app.ui.library_home = false;
                }
            }
        }
    });
}

pub fn show(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let page = app.services.device_gallery.as_mut().and_then(|g| (g.take_page)());
    if let Some(page) = page {
        let state = &mut app.caches.device_photos;
        state.requested = false;
        match page {
            DevicePage::Photos { offset, photos, more } => {
                if offset == state.requested_offset {
                    let appending = offset == state.items.len();
                    for (index, photo) in photos.into_iter().take(PAGE_SIZE).enumerate() {
                        let position = offset.saturating_add(index);
                        let preview_available = photo.preview.is_some();
                        let texture =
                            photo.preview.map(|image| ctx.load_texture(format!("device-photo:{}", photo.id), image, TextureOptions::LINEAR));
                        if let Some(item) = state.items.get_mut(position) {
                            *item = Item { id: photo.id, texture, preview_available };
                        } else if position == state.items.len() {
                            state.items.push(Item { id: photo.id, texture, preview_available });
                        }
                    }
                    if appending {
                        state.more = more;
                        if offset == 0 {
                            state.loaded = true;
                        }
                    }
                    state.notice = None;
                }
            }
            DevicePage::PermissionNeeded => {
                state.items.clear();
                state.more = false;
                state.loaded = false;
                state.notice = Some(Notice::Permission);
            }
            DevicePage::Error(err) => {
                state.items.clear();
                state.more = false;
                state.loaded = false;
                state.notice = Some(Notice::Failed(err));
            }
        }
    }
    let opened = app.services.device_gallery.as_mut().and_then(|g| (g.take_open)());
    if let Some(result) = opened {
        app.caches.device_photos.opening = false;
        match result {
            Ok(path) => match app.run("library.import", json!({"paths":[path],"mode":"add"})) {
                Ok(report) => {
                    let id = report["imported"]
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(serde_json::Value::as_u64)
                        .or_else(|| report["restored"].as_array().and_then(|a| a.first()).and_then(serde_json::Value::as_u64))
                        .or_else(|| report["duplicates"].as_array().and_then(|a| a.first()).and_then(|v| v["existing"].as_u64()));
                    if let Some(id) = id {
                        let _ = app.run("library.source", json!({"kind":"all"}));
                        app.session.selection = Selection::single(PhotoId(id));
                        app.ui.device_gallery = false;
                        app.ui.library_home = false;
                        app.ui.view = ViewMode::Detail;
                    } else {
                        app.toast(&ctx, "Photo is already in the library. Open it from All Photos.");
                    }
                }
                Err(err) => app.toast(&ctx, err),
            },
            Err(err) => app.toast(&ctx, err),
        }
    }
    let state = &mut app.caches.device_photos;
    if state.items.is_empty() || state.notice.is_some() {
        if let Some(notice) = &state.notice {
            match notice {
                Notice::Permission => {
                    ui.heading("Photo access needed");
                    ui.label("Allow access to browse Device Photos. Selected-photo access shows only those photos; your existing edits remain in My Edits.");
                }
                Notice::Failed(err) => {
                    ui.heading("Device Photos could not load");
                    ui.label(err);
                }
            }
            let retry =
                ui.add_sized([164.0, 44.0], egui::Button::new(if matches!(notice, Notice::Permission) { "Check photo access" } else { "Try again" }));
            register(ui.ctx(), "gallery:retry", retry.rect);
            if retry.clicked() {
                state.items.clear();
                state.more = false;
                state.loaded = false;
                state.notice = None;
                state.requested = false;
            }
        } else if state.requested {
            ui.spinner();
        } else {
            ui.heading("No device photos available");
            ui.label("This gallery shows the photos Android lets LightCraft access. Check photo access to refresh your selection.");
            let retry = ui.add_sized([164.0, 44.0], egui::Button::new("Check photo access"));
            register(ui.ctx(), "gallery:retry", retry.rect);
            if retry.clicked() {
                state.requested = false;
                state.items.clear();
                state.loaded = false;
            }
        }
    }
    let mut selected = None;
    let mut wanted_offset = None;
    egui::ScrollArea::vertical().id_salt("device-photos-scroll").show_viewport(ui, |ui, viewport| {
        let width = (ui.available_width() / 3.0 - 4.0).max(44.0);
        let row_h = width + 4.0;
        let start = (viewport.min.y.max(0.0) / row_h) as usize;
        let end = ((viewport.max.y.max(0.0) / row_h).ceil() as usize).saturating_add(1);
        let rows = state.items.len().div_ceil(3);
        // Hold only thumbnails near the visible rows. MediaStore's disk cache can
        // provide earlier pages again when the user scrolls back.
        let keep_start = start.saturating_mul(3).saturating_sub(PAGE_SIZE);
        let keep_end = end.saturating_mul(3).saturating_add(PAGE_SIZE);
        for (index, item) in state.items.iter_mut().enumerate() {
            if index < keep_start || index >= keep_end {
                item.texture = None;
            }
        }
        if !state.requested {
            if let Some(index) = (start.saturating_mul(3)..end.saturating_mul(3).min(state.items.len()))
                .find(|&index| state.items.get(index).is_some_and(|item| item.preview_available && item.texture.is_none()))
            {
                wanted_offset = Some(index / PAGE_SIZE * PAGE_SIZE);
            } else if state.more && viewport.max.y + row_h * 3.0 >= rows as f32 * row_h {
                wanted_offset = Some(state.items.len());
            }
        }
        ui.add_space(start.min(rows) as f32 * row_h);
        for row in start.min(rows)..end.min(rows) {
            ui.horizontal(|ui| {
                for col in 0..3 {
                    let Some(item) = state.items.get(row.saturating_mul(3).saturating_add(col)) else { break };
                    let (rect, response) = ui.allocate_exact_size(vec2(width, width), Sense::click());
                    register(ui.ctx(), format!("devicePhoto:{}", item.id), rect);
                    ui.painter().rect_filled(rect, 2.0, Color32::DARK_GRAY);
                    if let Some(texture) = &item.texture {
                        ui.painter().image(texture.id(), rect, cover_uv(texture.size_vec2()), Color32::WHITE);
                    }
                    if response.clicked() {
                        selected = Some(item.id.clone());
                    }
                }
            });
        }
        ui.add_space(rows.saturating_sub(end.min(rows)) as f32 * row_h);
        if state.more || state.requested {
            ui.add_space(row_h);
        }
    });
    if !state.requested && !state.loaded && state.notice.is_none() {
        request_page(app.services.device_gallery.as_mut(), state, 0);
    } else if state.notice.is_none()
        && let Some(offset) = wanted_offset
    {
        request_page(app.services.device_gallery.as_mut(), state, offset);
    }
    if let Some(id) = selected
        && !state.opening
    {
        state.opening = true;
        if let Some(gallery) = app.services.device_gallery.as_mut()
            && let Err(err) = (gallery.open)(&id)
        {
            state.opening = false;
            state.notice = Some(Notice::Failed(err));
        }
    }
    if state.opening {
        ui.spinner();
    }
    // A permission dialog or background decode completes asynchronously.
    if state.requested || state.opening {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}

// A gallery tile is square, but a portrait or landscape preview must keep its
// proportions. Crop the center of the source image instead of stretching it.
fn cover_uv(size: egui::Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    }
    if size.x > size.y {
        let margin = (1.0 - size.y / size.x) / 2.0;
        Rect::from_min_max(pos2(margin, 0.0), pos2(1.0 - margin, 1.0))
    } else {
        let margin = (1.0 - size.x / size.y) / 2.0;
        Rect::from_min_max(pos2(0.0, margin), pos2(1.0, 1.0 - margin))
    }
}

fn request_page(gallery: Option<&mut crate::DeviceGallery>, state: &mut State, offset: usize) {
    state.requested = true;
    state.requested_offset = offset;
    if let Some(gallery) = gallery {
        if let Err(err) = (gallery.request)(offset) {
            state.requested = false;
            state.notice = Some(Notice::Failed(err));
        }
    } else {
        state.requested = false;
        state.notice = Some(Notice::Failed("Device Photos is unavailable".into()));
    }
}

#[cfg(test)]
mod tests {
    use super::cover_uv;
    use egui::vec2;

    #[test]
    fn square_gallery_crops_portrait_and_landscape_without_distortion() {
        let landscape = cover_uv(vec2(192.0, 96.0));
        assert_eq!(landscape.min.x, 0.25);
        assert_eq!(landscape.max.x, 0.75);
        assert_eq!(landscape.min.y, 0.0);
        let portrait = cover_uv(vec2(96.0, 192.0));
        assert_eq!(portrait.min.y, 0.25);
        assert_eq!(portrait.max.y, 0.75);
        assert_eq!(portrait.min.x, 0.0);
    }
}
