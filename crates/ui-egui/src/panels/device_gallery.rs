//! Android gallery presentation. The platform owns permission checks and paged MediaStore queries;
//! browsing never copies an original or creates a catalog entry.

use egui::{Color32, Rect, Sense, TextureHandle, TextureOptions, pos2, vec2};
use lightcraft_catalog::PhotoId;
use lightcraft_engine::Selection;
use serde_json::json;

use crate::{DevicePage, LightcraftApp, state::ViewMode, widgets::register};

const MAX_ITEMS: usize = 2000;

struct Item {
    id: String,
    name: String,
    texture: Option<TextureHandle>,
}

#[derive(Default)]
pub struct State {
    items: Vec<Item>,
    requested: bool,
    more: bool,
    opening: bool,
    message: Option<String>,
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
                if offset == 0 {
                    state.items.clear();
                }
                if offset == state.items.len() {
                    for photo in photos.into_iter().take(MAX_ITEMS.saturating_sub(state.items.len())) {
                        let texture =
                            photo.preview.map(|image| ctx.load_texture(format!("device-photo:{}", photo.id), image, TextureOptions::LINEAR));
                        state.items.push(Item { id: photo.id, name: photo.name, texture });
                    }
                    state.more = more && state.items.len() < MAX_ITEMS;
                    state.message = None;
                }
            }
            DevicePage::PermissionNeeded => {
                state.items.clear();
                state.more = false;
                state.message = Some("Allow photo access to browse Device Photos. Selected-only access shows only selected photos.".into());
            }
            DevicePage::Error(err) => {
                state.items.clear();
                state.more = false;
                state.message = Some(err);
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
    if state.items.is_empty() || state.message.is_some() {
        if let Some(message) = &state.message {
            ui.label(message);
            let retry = ui.add_sized([164.0, 44.0], egui::Button::new("Allow Photos / Retry"));
            register(ui.ctx(), "gallery:retry", retry.rect);
            if retry.clicked() {
                state.message = None;
                state.requested = false;
            }
        } else if state.requested {
            ui.spinner();
        } else {
            ui.label("No accessible photos yet. Allow access, or add photos using the system picker.");
        }
    }
    let mut selected = None;
    egui::ScrollArea::vertical().id_salt("device-photos-scroll").show_viewport(ui, |ui, viewport| {
        let width = (ui.available_width() / 3.0 - 4.0).max(44.0);
        let row_h = width + 27.0;
        let start = (viewport.min.y.max(0.0) / row_h) as usize;
        let end = ((viewport.max.y.max(0.0) / row_h).ceil() as usize).saturating_add(1);
        let rows = state.items.len().div_ceil(3);
        ui.add_space(start.min(rows) as f32 * row_h);
        for row in start.min(rows)..end.min(rows) {
            ui.horizontal(|ui| {
                for col in 0..3 {
                    let Some(item) = state.items.get(row.saturating_mul(3).saturating_add(col)) else { break };
                    let (rect, response) = ui.allocate_exact_size(vec2(width, row_h - 2.0), Sense::click());
                    register(ui.ctx(), format!("devicePhoto:{}", item.id), rect);
                    let image_rect = Rect::from_min_size(rect.min, vec2(width, width));
                    ui.painter().rect_filled(image_rect, 4.0, Color32::DARK_GRAY);
                    if let Some(texture) = &item.texture {
                        ui.painter().image(texture.id(), image_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    let label_rect = Rect::from_min_max(pos2(rect.left() + 2.0, image_rect.bottom() + 4.0), pos2(rect.right() - 2.0, rect.bottom()));
                    register(ui.ctx(), format!("devicePhotoLabel:{}", item.id), label_rect);
                    ui.painter().with_clip_rect(label_rect).text(
                        pos2(rect.left() + 2.0, image_rect.bottom() + 4.0),
                        egui::Align2::LEFT_TOP,
                        &item.name,
                        egui::FontId::proportional(11.0),
                        Color32::WHITE,
                    );
                    if response.clicked() {
                        selected = Some(item.id.clone());
                    }
                }
            });
        }
        ui.add_space(rows.saturating_sub(end.min(rows)) as f32 * row_h);
        if state.more && !state.requested && viewport.max.y + row_h * 3.0 >= rows as f32 * row_h {
            state.requested = true;
            if let Some(gallery) = app.services.device_gallery.as_mut()
                && let Err(err) = (gallery.request)(state.items.len())
            {
                state.requested = false;
                state.message = Some(err);
            }
        }
    });
    if !state.requested && state.items.is_empty() && state.message.is_none() {
        state.requested = true;
        if let Some(gallery) = app.services.device_gallery.as_mut()
            && let Err(err) = (gallery.request)(0)
        {
            state.requested = false;
            state.message = Some(err);
        }
    }
    if let Some(id) = selected
        && !state.opening
    {
        state.opening = true;
        if let Some(gallery) = app.services.device_gallery.as_mut()
            && let Err(err) = (gallery.open)(&id)
        {
            state.opening = false;
            state.message = Some(err);
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
