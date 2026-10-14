//! The compact layout: phones, and any window narrower than [`COMPACT_WIDTH`] (iPad Split View, a
//! phone browser) or shorter than [`COMPACT_HEIGHT`] and narrower than [`COMPACT_SHORT_WIDTH`]
//! (a phone held sideways). The desktop
//! layout's side columns and filmstrip leave such a window no room for the photo, so here:
//!
//! - the top bar keeps the sidebar toggle, Back, the menus (one "Menu" button), a search field
//!   that takes the free width, Filter and Export;
//! - the tool strip goes; its tools sit in a row atop a **bottom sheet** under the photo, which
//!   holds the right panel (Edit, Crop, Masking …) or the Presets column at full width; in a
//!   window wider than tall ([`beside`]) the sheet is a column at the right of the photo instead;
//! - My Photos (the left panel) opens as a full-width drawer (a column at the left in a wide
//!   window), closed again by picking a source;
//! - the bottom bar keeps the view modes, Sort and an Edit toggle.
//!
//! The panels themselves are the desktop ones: [`super::resizable_side`] and the Presets column
//! fill the region they're given while [`is_compact`] holds.

use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::LightcraftApp;
use crate::icons::{Icon, paint};
use crate::state::{RightPanel, ViewMode};
use crate::theme::Tokens;
use crate::widgets::{icon_button, register};

/// Content width (points, inside the safe area) below which the compact layout is used.
pub const COMPACT_WIDTH: f32 = 600.0;

/// Content height (points, inside the safe area) below which the compact layout is used: a phone
/// in landscape (an iPhone is about 400 pt tall, an Android phone about 410 pt less its status
/// bar). The smallest iPad in landscape is 744 pt tall.
pub const COMPACT_HEIGHT: f32 = 500.0;

/// A short window is compact only when it is also narrower than this: phones in landscape are at
/// most about 960 pt wide, while a short but wide desktop window keeps the desktop layout and its
/// menu bar.
pub const COMPACT_SHORT_WIDTH: f32 = 1000.0;

/// Width of the sheet and the My Photos drawer when they sit beside the photo ([`beside`]).
const SIDE_MIN: f32 = 300.0;
const SIDE_MAX: f32 = 420.0;

/// Side margin of the compact top bar, bottom bar and the sheet's tool row, so their icon
/// columns line up.
const BAR_MARGIN: i8 = 6;

/// Touch-friendly button size in the compact bars.
const BUTTON: egui::Vec2 = egui::vec2(40.0, 40.0);

fn flag_id() -> egui::Id {
    egui::Id::new("compact-layout")
}

/// Whether this frame uses the compact layout.
pub fn is_compact(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(flag_id())).unwrap_or(false)
}

/// Decide the layout for this frame from the content area (inside the safe area). Entering the
/// compact layout closes My Photos, which would otherwise cover the photo as a full-width
/// drawer. A content area that got shorter (iOS: the soft keyboard came up) starts a few frames
/// of [`keep_focus_visible`].
pub(crate) fn update(app: &mut LightcraftApp, ctx: &egui::Context, content: Rect) {
    let (bottom, frames) = ctx.data(|d| d.get_temp::<(f32, u8)>(shrink_id())).unwrap_or((content.bottom(), 0));
    let frames = if content.bottom() < bottom - 0.5 { 4 } else { frames.saturating_sub(1) };
    ctx.data_mut(|d| d.insert_temp(shrink_id(), (content.bottom(), frames)));
    let compact = content.width() < COMPACT_WIDTH || (content.height() < COMPACT_HEIGHT && content.width() < COMPACT_SHORT_WIDTH);
    if compact && !is_compact(ctx) {
        app.ui.left_panel = false;
    }
    ctx.data_mut(|d| d.insert_temp(flag_id(), compact));
    // a phone: controls a finger can hit
    crate::theme::finger_sizes(ctx, compact && crate::widgets::touch_screen(ctx));
}

/// In the compact layout, whether the window is wider than tall (a phone in landscape): the
/// sheet and the My Photos drawer then sit beside the photo as columns, not over or under it.
pub fn beside(ctx: &egui::Context) -> bool {
    let r = ctx.content_rect();
    is_compact(ctx) && r.width() > r.height()
}

/// The width of a column beside the photo ([`beside`]) in a region `avail` wide.
pub fn side_width(avail: f32) -> f32 {
    (avail * 0.45).clamp(SIDE_MIN, SIDE_MAX).min(avail)
}

/// The bottom sheet is up: Detail view with a right panel or Presets open, or a grid with a
/// panel that isn't an editing tool (Info, Keywords, Versions, History), as the desktop's grid
/// has. (Editing tools and Presets switch to Detail.)
pub fn sheet_open(app: &LightcraftApp) -> bool {
    match app.ui.view {
        ViewMode::Detail => app.ui.right != RightPanel::None || app.ui.presets,
        ViewMode::PhotoGrid | ViewMode::SquareGrid => {
            matches!(app.ui.right, RightPanel::Info | RightPanel::Keywords | RightPanel::Versions | RightPanel::Activity)
        }
        _ => false,
    }
}

/// The window's chrome in the compact layout; the caller fills the central area as usual.
pub fn show(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    topbar(app, ui);
    mobile_export_dialog(app, ui.ctx());
    super::library_problem::banner(app, ui);
    // Device browsing and the Library home do not operate on the catalog's current selection.
    // The catalog-specific Select/Edit/Organize bar would act on a different photo here.
    let device_home = app.services.device_gallery.is_some()
        && crate::widgets::touch_screen(ui.ctx())
        && matches!(app.ui.view, ViewMode::PhotoGrid | ViewMode::SquareGrid)
        && (app.ui.device_gallery || app.ui.library_home);
    if !device_home {
        bottombar(app, ui);
    }
    if app.ui.left_panel {
        // picking a source (album, folder, date …) closes the drawer onto its photos
        let source = app.session.source;
        // the drawer: the whole region, or a column beside the photos in a wide window
        let avail = ui.available_rect_before_wrap();
        let w = if beside(ui.ctx()) { side_width(avail.width()) } else { avail.width() };
        let r = egui::Panel::left("left_panel").frame(egui::Frame::NONE).resizable(false).exact_size(w).show(ui, |ui| super::left::show(app, ui));
        register(ui.ctx(), "panel:left_panel", r.response.rect);
        if app.session.source != source {
            app.ui.left_panel = false;
        }
    } else if sheet_open(app) {
        if super::edit::mobile_curve_active(app, ui.ctx()) {
            super::edit::mobile_curve_workspace(app, ui.ctx(), ui.available_rect_before_wrap());
        } else if !(crate::widgets::touch_screen(ui.ctx())
            && app.ui.view == ViewMode::Detail
            && app.ui.right == RightPanel::Masking
            && !app.ui.presets
            && app.session.active_mask.is_none())
        {
            // Until a mask is selected there are no adjustments: the photo-side Add/selection
            // rail is the only useful control. Leave the image visible instead of an empty sheet.
            sheet(app, ui);
        }
    }
}

fn topbar(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let editor = crate::widgets::touch_screen(ui.ctx()) && app.ui.view == ViewMode::Detail;
    let gallery_home =
        app.services.device_gallery.is_some() && crate::widgets::touch_screen(ui.ctx()) && (app.ui.device_gallery || app.ui.library_home);
    egui::Panel::top("top_bar")
        .exact_size(if editor { 44.0 } else { t.top_bar_h + 4.0 })
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: BAR_MARGIN, right: BAR_MARGIN, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if !editor
                    && !gallery_home
                    && icon_button(ui, "sidebar", Icon::Sidebar, BUTTON, app.ui.left_panel, true, "Show/hide My Photos panel").clicked()
                {
                    let _ = app.run("view.leftPanel", json!({}));
                }
                if !gallery_home && icon_button(ui, "back", Icon::Back, BUTTON, false, true, "Back").clicked() {
                    let _ = app.run("view.back", json!({}));
                }
                if !app.native_menu {
                    ui.add_space(4.0);
                    crate::menubar::show_in_window(app, ui, 0.0);
                    ui.add_space(8.0);
                }
                // right-to-left from the edge: Export, Filter, then the search field takes the rest
                let rest = ui.available_rect_before_wrap();
                let mut right = ui.new_child(egui::UiBuilder::new().max_rect(rest).layout(egui::Layout::right_to_left(egui::Align::Center)));
                right.spacing_mut().item_spacing.x = 0.0;
                if !gallery_home && icon_button(&mut right, "share", Icon::Share, BUTTON, false, true, "Export").clicked() {
                    if crate::widgets::touch_screen(right.ctx()) && app.services.mobile_export.is_some() {
                        right.ctx().data_mut(|d| d.insert_temp(egui::Id::new("mobile-export-open"), true));
                    } else {
                        let _ = app.run("dialog.export", json!({}));
                    }
                }
                if !editor && !gallery_home {
                    let filtering = app.session.filter != Default::default() || app.ui.filter_bar;
                    if icon_button(&mut right, "filter", Icon::Filter, BUTTON, filtering, true, "Filter bar").clicked() {
                        let _ = app.run("view.filterBar", json!({}));
                    }
                }
                // a failing save keeps its warning visible (the desktop's cloud icon)
                if let Some((n, e)) = app.session.unsaved() {
                    let (r, resp) = right.allocate_exact_size(BUTTON, Sense::hover());
                    paint(right.painter(), r.shrink(10.0), Icon::Cloud, t.caution);
                    register(right.ctx(), "indicator:unsaved", r);
                    resp.on_hover_text(format!("{n} unsaved: {e}"));
                }
                if !editor && !gallery_home {
                    right.add_space(4.0);
                    let field = right.available_rect_before_wrap();
                    if field.width() >= 80.0 {
                        search(app, &mut right, Rect::from_center_size(field.center(), vec2(field.width(), 32.0)));
                    }
                }
            });
        });
}

fn mobile_export_dialog(app: &mut LightcraftApp, ctx: &egui::Context) {
    if app.services.mobile_export.is_none() || !crate::widgets::touch_screen(ctx) {
        return;
    }
    let id = egui::Id::new("mobile-export-open");
    if !ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false) {
        return;
    }
    let format_id = egui::Id::new("mobile-export-format");
    let previous = app.session.last_export.as_ref();
    let mut format = ctx
        .data(|d| d.get_temp::<String>(format_id))
        .unwrap_or_else(|| previous.and_then(|p| p.get("format")).and_then(serde_json::Value::as_str).unwrap_or("jpeg").to_string());
    let quality_id = egui::Id::new("mobile-export-quality");
    let mut quality = ctx
        .data(|d| d.get_temp::<i64>(quality_id))
        .unwrap_or_else(|| previous.and_then(|p| p.get("quality")).and_then(serde_json::Value::as_i64).unwrap_or(90).clamp(1, 100));
    let edge_id = egui::Id::new("mobile-export-long-edge");
    let mut long_edge = ctx
        .data(|d| d.get_temp::<i64>(edge_id))
        .unwrap_or_else(|| previous.and_then(|p| p.get("longEdge")).and_then(serde_json::Value::as_i64).unwrap_or(0).clamp(0, 65535));
    let creator_id = egui::Id::new("mobile-export-creator");
    let mut creator = ctx
        .data(|d| d.get_temp::<String>(creator_id))
        .unwrap_or_else(|| previous.and_then(|p| p.get("mobileCreator")).and_then(serde_json::Value::as_str).unwrap_or("").to_string());
    let copyright_id = egui::Id::new("mobile-export-copyright");
    let mut copyright = ctx
        .data(|d| d.get_temp::<String>(copyright_id))
        .unwrap_or_else(|| previous.and_then(|p| p.get("mobileCopyright")).and_then(serde_json::Value::as_str).unwrap_or("").to_string());
    let selected = &app.session.selection.ids;
    let dng_available = if selected.is_empty() {
        app.session.active().and_then(|id| app.session.catalog.photo(id)).is_some_and(|photo| photo.develops_raw())
    } else {
        selected.iter().all(|id| app.session.catalog.photo(*id).is_some_and(|photo| photo.develops_raw()))
    };
    if format == "dng" && !dng_available {
        format = "jpeg".into();
    }
    let mut open = true;
    egui::Window::new(crate::i18n::tr("Export"))
        .collapsible(false)
        .resizable(false)
        .default_width(260.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr("Format"));
            let formats = egui::ComboBox::from_id_salt("mobile-export-format-select").selected_text(format.to_uppercase()).show_ui(ui, |ui| {
                for (value, label) in [
                    ("jpeg", "JPEG"),
                    ("png", "PNG"),
                    ("webp", "WebP"),
                    ("avif", "AVIF"),
                    ("tiff", "TIFF"),
                    ("psd", "PSD"),
                    ("jxl", "JPEG XL"),
                    ("original", "Original + XMP"),
                ] {
                    let response = ui.selectable_value(&mut format, value.into(), label);
                    register(ui.ctx(), format!("mobileExport:{value}"), response.rect);
                }
                if dng_available {
                    let response = ui.selectable_value(&mut format, "dng".into(), "DNG");
                    register(ui.ctx(), "mobileExport:dng", response.rect);
                }
            });
            register(ui.ctx(), "mobileExport:format", formats.response.rect);
            if !matches!(format.as_str(), "original" | "dng") {
                ui.label(crate::i18n::tr("Long edge (px; 0 = full size)"));
                let edge = ui.add_sized([ui.available_width(), 44.0], egui::DragValue::new(&mut long_edge).range(0..=65535).speed(16.0));
                register(ui.ctx(), "mobileExport:longEdge", edge.rect);
            }
            if matches!(format.as_str(), "jpeg" | "avif" | "jxl") {
                let slider = ui.add_sized([ui.available_width(), 44.0], egui::Slider::new(&mut quality, 1..=100).text(crate::i18n::tr("Quality")));
                register(ui.ctx(), "mobileExport:quality", slider.rect);
            }
            ui.label(crate::i18n::tr("Export attribution (optional)"));
            let byline = ui.add_sized([ui.available_width(), 44.0], egui::TextEdit::singleline(&mut creator).hint_text(crate::i18n::tr("Creator")));
            register(ui.ctx(), "mobileExport:creator", byline.rect);
            let rights =
                ui.add_sized([ui.available_width(), 44.0], egui::TextEdit::singleline(&mut copyright).hint_text(crate::i18n::tr("Copyright notice")));
            register(ui.ctx(), "mobileExport:copyright", rights.rect);
            if matches!(format.as_str(), "original" | "dng") {
                ui.small(crate::i18n::tr("Original bytes stay unchanged; attribution is written to its XMP sidecar or new DNG."));
            }
            let note = if format == "original" {
                "Original: Photos for JPEG/PNG/WebP, otherwise Downloads. XMP: Downloads".to_string()
            } else {
                let destination = if matches!(format.as_str(), "jpeg" | "png" | "webp") { "Photos" } else { "Downloads" };
                format!("Saved to {destination}; sidecars go to Downloads")
            };
            ui.label(note);
            let export = ui.add_sized([ui.available_width(), 44.0], egui::Button::new(crate::i18n::tr("Export")));
            register(ui.ctx(), "mobileExport:submit", export.rect);
            if export.clicked() {
                let mut params = json!({"format": format, "background": true, "mobileDestination": true,
                "metadata": "all", "mobileCreator": creator, "mobileCopyright": copyright});
                if !matches!(format.as_str(), "original" | "dng") {
                    params["longEdge"] = json!(long_edge);
                }
                if matches!(format.as_str(), "jpeg" | "avif" | "jxl") {
                    params["quality"] = json!(quality);
                }
                match app.run("app.export", params) {
                    Ok(_) => {
                        ctx.data_mut(|d| d.insert_temp(id, false));
                    }
                    Err(e) => app.toast(ctx, e),
                }
            }
        });
    ctx.data_mut(|d| {
        d.insert_temp(format_id, format);
        d.insert_temp(quality_id, quality);
        d.insert_temp(edge_id, long_edge);
        d.insert_temp(creator_id, creator);
        d.insert_temp(copyright_id, copyright);
        if !open {
            d.insert_temp(id, false);
        }
    });
}

/// The search field in `r` (same id and command as the desktop top bar's).
fn search(app: &mut LightcraftApp, ui: &mut egui::Ui, r: Rect) {
    let t = Tokens::get(ui.ctx());
    let id = crate::text_field::id("field:search");
    if std::mem::take(&mut app.ui.focus_search) {
        ui.memory_mut(|m| m.request_focus(id));
    }
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect(
        r,
        6.0,
        if focused { t.canvas } else { t.field },
        Stroke::new(1.0, if focused { t.accent } else { t.field_border }),
        StrokeKind::Inside,
    );
    let inner = r.shrink2(vec2(10.0, 4.0));
    if app.ui.search.is_empty() && !focused {
        paint(ui.painter(), Rect::from_min_size(pos2(inner.left(), r.center().y - 8.0), vec2(16.0, 16.0)), Icon::Search, t.text_dim);
    }
    let text = Rect::from_min_max(pos2(inner.left() + 22.0, inner.top()), inner.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text).layout(egui::Layout::left_to_right(egui::Align::Center)));
    // (registered as `field:search` by the field itself, like the desktop's)
    let resp = crate::text_field::TextField::singleline("field:search", &mut app.ui.search)
        .frame(false)
        .hint(crate::i18n::tr("Search"))
        .width(text.width())
        .font(t.font(14.0))
        .text_color(t.text)
        .show(&mut child)
        .response;
    if resp.changed() {
        let q = app.ui.search.clone();
        let _ = app.run("library.filter", json!({"text": q}));
    }
}

fn bottombar(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::bottom("bottom_bar")
        .exact_size(t.bottom_bar_h + 4.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: BAR_MARGIN, right: BAR_MARGIN, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            if crate::widgets::touch_screen(ui.ctx()) && app.ui.view == ViewMode::Detail {
                mobile_editor_bar(app, ui);
                return;
            }
            let full = ui.max_rect();
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (id, icon, mode, tip) in [
                    ("photoGrid", Icon::GridPhoto, ViewMode::PhotoGrid, "Photo Grid (G)"),
                    ("squareGrid", Icon::GridSquare, ViewMode::SquareGrid, "Square Grid (G toggles)"),
                    ("detail", Icon::Single, ViewMode::Detail, "Detail (D)"),
                ] {
                    if icon_button(ui, id, icon, BUTTON, app.ui.view == mode, true, tip).clicked() {
                        let _ = app.run(&format!("view.{id}"), json!({}));
                    }
                }
                let sort = icon_button(ui, "sort", Icon::Sort, BUTTON, false, true, "Sort");
                egui::Popup::menu(&sort).show(|ui| super::bottombar::sort_menu(app, ui));
                super::bottombar::select_mode_button(app, ui);
            });
            let mut right = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_max(pos2(full.center().x, full.top()), full.max))
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            right.spacing_mut().item_spacing.x = 2.0;
            let has_photo = app.session.active().is_some();
            let organize = icon_button(&mut right, "organize", Icon::More, BUTTON, false, has_photo, "Organize Photos");
            egui::Popup::menu(&organize).show(|ui| organize_menu(app, ui));
            // Edit: open the sheet on the Edit panel (from the grid too), or put it away
            if icon_button(&mut right, "edit", Icon::Sliders, BUTTON, sheet_open(app), has_photo, "Edit (E)").clicked() {
                if sheet_open(app) {
                    close_sheet(app);
                } else {
                    if app.ui.view != ViewMode::Detail {
                        let _ = app.run("view.detail", json!({}));
                    }
                    if app.ui.right == RightPanel::None && !app.ui.presets {
                        let _ = app.run("panel.edit", json!({}));
                    }
                }
            }
            if app.ui.view == ViewMode::Detail
                && icon_button(&mut right, "filmstrip", Icon::Filmstrip, BUTTON, app.ui.filmstrip, true, "Filmstrip").clicked()
            {
                app.ui.filmstrip = !app.ui.filmstrip;
            }
        });
}

/// Six fixed touch targets fit even a 360-point phone. Secondary tools live in the More grid.
fn mobile_editor_bar(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let has_photo = app.session.active().is_some();
    let width = ((ui.available_width() - 10.0) / 6.0).clamp(44.0, 68.0);
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (id, label, icon, panel) in [
            ("presets", "Presets", Icon::Presets, RightPanel::None),
            ("edit", "Edit", Icon::Sliders, RightPanel::Edit),
            ("crop", "Crop", Icon::Crop, RightPanel::Crop),
            ("remove", "Remove", Icon::Eraser, RightPanel::Remove),
            ("masking", "Masking", Icon::Mask, RightPanel::Masking),
        ] {
            let active = if id == "presets" { app.ui.presets } else { !app.ui.presets && app.ui.right == panel };
            let response = ui.add_enabled_ui(has_photo, |ui| mobile_tool(ui, id, label, icon, active, width, &t)).inner;
            if response.clicked() {
                if active {
                    close_sheet(app);
                } else if id == "presets" {
                    let _ = app.run("panel.presets", json!({}));
                } else {
                    app.ui.presets = false;
                    let _ = app.run(&format!("panel.{id}"), json!({}));
                }
            }
        }
        let more = mobile_tool(ui, "more", "More", Icon::More, false, width, &t);
        egui::Popup::menu(&more).show(|ui| {
            ui.set_min_width(204.0);
            for row in [
                [("redeye", "Red Eye", Icon::Eye), ("info", "Info", Icon::Info), ("keywords", "Keywords", Icon::Tag)],
                [("versions", "Versions", Icon::Versions), ("activity", "History", Icon::Activity), ("", "", Icon::More)],
            ] {
                ui.horizontal(|ui| {
                    for (id, label, icon) in row {
                        if id.is_empty() {
                            ui.add_space(64.0);
                            continue;
                        }
                        let selected = !app.ui.presets
                            && app.ui.right
                                == match id {
                                    "redeye" => RightPanel::RedEye,
                                    "info" => RightPanel::Info,
                                    "keywords" => RightPanel::Keywords,
                                    "versions" => RightPanel::Versions,
                                    _ => RightPanel::Activity,
                                };
                        let response =
                            ui.add_enabled_ui(has_photo || id == "activity", |ui| mobile_tool(ui, id, label, icon, selected, 64.0, &t)).inner;
                        if response.clicked() {
                            app.ui.presets = false;
                            if selected {
                                close_sheet(app);
                            } else if let Err(e) = app.run(&format!("panel.{id}"), json!({})) {
                                app.toast(ui.ctx(), e);
                            }
                            ui.close();
                        }
                    }
                });
            }
        });
    });
}

fn mobile_tool(ui: &mut egui::Ui, id: &str, label: &str, icon: Icon, active: bool, width: f32, t: &Tokens) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 44.0), Sense::click());
    register(ui.ctx(), format!("icon:{id}"), rect);
    if active {
        ui.painter().rect_filled(rect.shrink2(vec2(2.0, 1.0)), 8.0, t.cell_selected);
    }
    let color = if active { t.accent } else { t.text_dim };
    paint(ui.painter(), Rect::from_center_size(pos2(rect.center().x, rect.top() + 15.0), vec2(20.0, 20.0)), icon, color);
    ui.painter().text(pos2(rect.center().x, rect.bottom() - 3.0), egui::Align2::CENTER_BOTTOM, crate::i18n::tr(label), t.font(10.0), color);
    response.on_hover_text(label)
}

/// Phone-accessible catalog actions for the current selection. Albums organize library photos;
/// removing membership does not delete an original, while Delete moves it to Recently Deleted.
fn organize_menu(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let deleted = app.session.source == lightcraft_engine::LibrarySource::RecentlyDeleted;
    if deleted {
        let restore = ui.button(crate::i18n::tr("Restore Photos"));
        register(ui.ctx(), "button:organizeRestore", restore.rect);
        if restore.clicked() {
            if let Err(e) = app.run("photo.restore", json!({})) {
                app.toast(ui.ctx(), e);
            }
            ui.close();
        }
        return;
    }
    let local_only = super::grid::local_only(app, &app.session.targets(&serde_json::Value::Null));
    ui.add_enabled_ui(!local_only, |ui| {
        let add = ui.menu_button(crate::i18n::tr("Add to Album"), |ui| {
            let albums: Vec<_> = app.session.catalog.albums().filter(|a| !a.folder && !a.is_smart()).map(|a| (a.id.0, a.name.clone())).collect();
            for (id, name) in albums {
                let add = ui.button(name);
                register(ui.ctx(), format!("button:organizeAlbum:{id}"), add.rect);
                if add.clicked() {
                    match app.run("album.addPhotos", json!({"id": id})) {
                        Ok(r) => super::grid::toast_skipped(app, ui.ctx(), &r),
                        Err(e) => app.toast(ui.ctx(), e),
                    }
                    ui.close();
                }
            }
            if ui.button(crate::i18n::tr("New Album…")).clicked() {
                if let Err(e) = app.run("dialog.newAlbum", json!({})) {
                    app.toast(ui.ctx(), e);
                }
                ui.close();
            }
        });
        register(ui.ctx(), "button:organizeAdd", add.response.rect);
        add.response.on_disabled_hover_text(super::grid::albums_hint());
    });
    if let lightcraft_engine::LibrarySource::Album(id) = app.session.source
        && app.session.catalog.album(id).is_some_and(|a| !a.is_smart())
    {
        let remove = ui.button(crate::i18n::tr("Remove from Album"));
        register(ui.ctx(), "button:organizeRemove", remove.rect);
        if remove.clicked() {
            if let Err(e) = app.run("album.removePhotos", json!({"id": id.0})) {
                app.toast(ui.ctx(), e);
            }
            ui.close();
        }
    }
    ui.separator();
    let delete = ui.button(crate::i18n::tr("Move to Recently Deleted"));
    register(ui.ctx(), "button:organizeDelete", delete.rect);
    if delete.clicked() {
        if let Err(e) = app.run("photo.delete", json!({})) {
            app.toast(ui.ctx(), e);
        }
        ui.close();
    }
}

pub(super) fn close_sheet(app: &mut LightcraftApp) {
    app.ui.right = RightPanel::None;
    app.ui.presets = false;
}

/// The bottom sheet: a row of tools, then the right panel or Presets at full width. In a wide
/// window ([`beside`]) it is a column at the right of the photo.
fn sheet(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let avail = ui.available_rect_before_wrap();
    let panel = if beside(ui.ctx()) {
        egui::Panel::right("compact_sheet_side").exact_size(side_width(avail.width()))
    } else {
        // One stable height on a touch phone: leave room for the photo and the active tool.
        let touch = crate::widgets::touch_screen(ui.ctx()) && app.ui.view == ViewMode::Detail;
        let height = if touch {
            (avail.height() * 0.46).clamp(280.0, (avail.height() - 160.0).max(280.0))
        } else {
            (avail.height() * 0.5).clamp(220.0, (avail.height() - 160.0).max(220.0))
        };
        egui::Panel::bottom("compact_sheet").exact_size(height)
    };
    panel.resizable(false).frame(egui::Frame::NONE.fill(t.chrome).stroke(Stroke::new(1.0, t.divider))).show(ui, |ui| {
        register(ui.ctx(), "panel:sheet", ui.max_rect());
        if !(crate::widgets::touch_screen(ui.ctx()) && app.ui.view == ViewMode::Detail) {
            egui::Panel::top("compact_sheet_tools")
                .exact_size(BUTTON.y + 4.0)
                .frame(egui::Frame::NONE.inner_margin(egui::Margin { left: BAR_MARGIN - 1, right: BAR_MARGIN - 1, top: 2, bottom: 2 }))
                .show(ui, |ui| tools(app, ui));
        }
        if app.ui.presets {
            super::presets::show(app, ui);
        } else {
            register(ui.ctx(), "panel:right_panel", ui.available_rect_before_wrap());
            super::right::show(app, ui);
        }
    });
}

fn tools(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let has_photo = app.session.active().is_some();
    let full = ui.max_rect();
    egui::ScrollArea::horizontal().id_salt("compact-tools").auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if icon_button(ui, "presets", Icon::Presets, BUTTON, app.ui.presets, has_photo, "Presets (Shift+P)").clicked() {
                let _ = app.run("panel.presets", json!({}));
            }
            for (id, icon, panel, tip) in [
                ("edit", Icon::Sliders, RightPanel::Edit, "Edit (E)"),
                ("crop", Icon::Crop, RightPanel::Crop, "Crop & Rotate (C)"),
                ("remove", Icon::Eraser, RightPanel::Remove, "Remove (H)"),
                ("masking", Icon::Mask, RightPanel::Masking, "Masking (M)"),
                ("redeye", Icon::Eye, RightPanel::RedEye, "Red Eye"),
                ("info", Icon::Info, RightPanel::Info, "Info (I)"),
                ("keywords", Icon::Tag, RightPanel::Keywords, "Keywords (K)"),
                ("versions", Icon::Versions, RightPanel::Versions, "Versions (Shift+V)"),
                ("activity", Icon::Activity, RightPanel::Activity, "History & Activity (Y)"),
            ] {
                let on = !app.ui.presets && (app.ui.right == panel || (panel == RightPanel::Edit && app.ui.right == RightPanel::Profiles));
                if icon_button(ui, id, icon, BUTTON, on, has_photo || panel == RightPanel::Activity, tip).clicked() {
                    // one thing at a time in the sheet: a tool replaces Presets instead of toggling off
                    app.ui.presets = false;
                    if !on {
                        let _ = app.run(&format!("panel.{id}"), json!({}));
                    }
                }
            }
            // room to scroll the last tool out from under the Hide button
            ui.add_space(BUTTON.x);
        });
    });
    // put the sheet away (pinned to the row's right end, over the scrolling tools)
    let r = Rect::from_min_size(pos2(full.right() - BUTTON.x, full.center().y - BUTTON.y / 2.0), BUTTON);
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(r, 0.0, t.chrome);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
    if icon_button(&mut child, "sheetClose", Icon::ChevronDown, BUTTON, false, true, "Hide").clicked() {
        close_sheet(app);
    }
}

fn shrink_id() -> egui::Id {
    egui::Id::new("content-shrink")
}

/// Call at the end of a scroll area's contents: for a few frames after the content area got
/// shorter, the focused widget (a text field, typically, whose keyboard just came up) is scrolled
/// back into view if it's in this scroll area and now hidden below the content area.
pub fn keep_focus_visible(ui: &egui::Ui) {
    let ctx = ui.ctx();
    // not on the frame the area shrank: the focused widget's rectangle (from the previous frame)
    // is still where it was laid out before
    if ctx.data(|d| d.get_temp::<(f32, u8)>(shrink_id())).is_none_or(|(_, frames)| frames == 0 || frames == 4) {
        return;
    }
    let Some(focused) = ctx.memory(|m| m.focused()).and_then(|id| ctx.read_response(id)) else { return };
    let visible = ui.clip_rect().intersect(ctx.content_rect());
    if ui.min_rect().contains_rect(focused.rect) && !visible.contains_rect(focused.rect) {
        // at once: the layout is still settling, and an animation would restart every frame
        ui.scroll_to_rect_animation(focused.rect, Some(egui::Align::Center), egui::style::ScrollAnimation::none());
    }
}

/// Space kept between a compact-layout dialog and the safe area's edges.
const DIALOG_INSET: f32 = 8.0;

/// The widest a compact-layout dialog gets (outer size).
const DIALOG_MAX_W: f32 = 560.0;

/// A dialog's content width: `desktop`, or in the compact layout what fits across the screen
/// inside a frame with `margin_x` of inner margin on each side.
pub fn dialog_width(ctx: &egui::Context, desktop: f32, margin_x: f32) -> f32 {
    if !is_compact(ctx) {
        return desktop;
    }
    desktop.min((ctx.content_rect().width().min(DIALOG_MAX_W + 2.0 * DIALOG_INSET) - 2.0 * (DIALOG_INSET + margin_x)).max(120.0))
}

/// In the compact layout a dialog window spans the screen (inside the safe area) at a fixed width
/// and scrolls when it is taller than the screen; elsewhere `win` is returned unchanged (desktop
/// dialogs fit the window on their own, #781).
pub fn dialog<'a>(ctx: &egui::Context, win: egui::Window<'a>) -> egui::Window<'a> {
    if !is_compact(ctx) {
        return win;
    }
    let area = ctx.content_rect().shrink(DIALOG_INSET);
    // egui's window sizes are its outer size (frame margins included); a phone in landscape gets
    // a dialog of readable width, not one stretched across the screen
    let w = area.width().min(DIALOG_MAX_W);
    // auto-sized: as tall as its contents up to the screen, then it scrolls
    win.auto_sized().min_width(w).max_width(w).max_height(area.height()).vscroll(true).constrain_to(area)
}

/// A labelled row for dialogs: in the compact layout the label sits above its controls, which
/// wrap; elsewhere `add` comes back for the caller's usual fixed label column.
pub fn stacked_row<R, F: FnOnce(&mut egui::Ui) -> R>(ui: &mut egui::Ui, label: egui::RichText, add: F) -> Result<R, F> {
    if !is_compact(ui.ctx()) {
        return Err(add);
    }
    Ok(ui
        .vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            // (no blank line for a row without a label)
            if !label.text().is_empty() {
                ui.label(label);
            }
            ui.horizontal_wrapped(add).inner
        })
        .inner)
}
