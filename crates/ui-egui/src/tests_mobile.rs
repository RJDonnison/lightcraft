//! Headless tests of what a touch screen offers in place of hovering, right-clicking with
//! modifiers and keys: everything the desktop does has a way in by finger (`docs/parity.md`).

use std::time::Duration;

use serde_json::json;

use crate::headless::Headless;
use crate::state::ViewMode;
use crate::{LightcraftApp, Services};

const SETTLE: Duration = Duration::from_secs(120);

/// The demo library in `view` on a touch screen of `size`, announced as the mobile shell does.
fn touch_app(view: ViewMode, size: [f32; 2]) -> Headless {
    let services = Services { png: None, ..Default::default() };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    app.ui.view = view;
    let mut h = Headless::new(app, size, 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    h
}

fn finger(phase: egui::TouchPhase, pos: egui::Pos2, id: u64) -> egui::Event {
    egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos, force: None }
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

/// A finger down and up at `at`, then lifted away.
fn tap(h: &mut Headless, at: egui::Pos2) {
    h.input([finger(egui::TouchPhase::Start, at, 1), egui::Event::PointerMoved(at), button(at, true)]);
    h.step();
    h.input([finger(egui::TouchPhase::End, at, 1), button(at, false)]);
    h.step();
    h.input([egui::Event::PointerGone]);
    h.step();
    // past a double tap
    for _ in 0..40 {
        h.step();
    }
}

fn widget(h: &Headless, id: &str) -> Option<egui::Rect> {
    h.app.widgets.iter().find(|(w, _)| w == id).map(|(_, r)| *r)
}

#[test]
fn phone_sheet_snaps_taller_and_dismisses_on_photo_tap() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    assert!(widget(&h, "panel:sheet").is_some());
    assert!(widget(&h, "mobileSheet:handle").is_some());
    assert!(widget(&h, "icon:sheetClose").is_none(), "no duplicate heading or Hide button");
    let short = widget(&h, "panel:sheet").unwrap().height();
    let handle = widget(&h, "mobileSheet:handle").unwrap().center();
    tap(&mut h, handle);
    let tall = widget(&h, "panel:sheet").unwrap().height();
    assert!(tall > short + 100.0, "two useful sheet heights: {short} / {tall}");
    let handle = widget(&h, "mobileSheet:handle").unwrap().center();
    tap(&mut h, handle);
    assert!((widget(&h, "panel:sheet").unwrap().height() - short).abs() < 2.0);

    let canvas = h.app.canvas_rect.expect("photo canvas");
    tap(&mut h, canvas.center());
    assert!(widget(&h, "panel:sheet").is_none(), "tapping the photo dismisses controls");
    assert_eq!(h.app.ui.zoom, crate::state::Zoom::Fit, "dismissal does not also zoom the photo");
}

/// Photos in the grid whose middle is on screen (between the header and `bottom`).
fn thumbs(h: &Headless, bottom: f32) -> Vec<(u64, egui::Rect)> {
    h.app
        .widgets
        .iter()
        .filter(|(w, r)| w.starts_with("thumb:") && r.center().y > 140.0 && r.center().y < bottom)
        .filter_map(|(w, r)| Some((w.trim_start_matches("thumb:").parse().ok()?, *r)))
        .collect()
}

/// A finger has no ⌘ or ⇧: Select Several makes taps add photos to the selection and take them
/// out again, and the grid shows a tick on each.
#[test]
fn select_several_makes_taps_add_and_remove_photos() {
    // (tall: the grid registers a row's first photo, and two rows have to fit)
    let mut h = touch_app(ViewMode::PhotoGrid, [1194.0, 1400.0]);
    let button = widget(&h, "button:selectMode").expect("Select in the bottom bar");
    tap(&mut h, button.center());
    assert!(h.app.ui.select_mode);
    // two photos not selected yet
    let t: Vec<(u64, egui::Rect)> =
        thumbs(&h, 1300.0).into_iter().filter(|(id, _)| !h.app.session.selection.contains(lightcraft_catalog::PhotoId(*id))).collect();
    let (a, b) = (t[0], t[1]);
    tap(&mut h, a.1.center());
    tap(&mut h, b.1.center());
    let sel: Vec<u64> = h.app.session.selection.ids.iter().map(|p| p.0).collect();
    assert!(sel.contains(&a.0) && sel.contains(&b.0), "both taps added: {sel:?}");
    let n = sel.len();
    tap(&mut h, a.1.center());
    assert!(!h.app.session.selection.contains(lightcraft_catalog::PhotoId(a.0)), "a second tap takes it out");
    assert_eq!(h.app.session.selection.ids.len(), n - 1);
    assert_eq!(h.app.ui.view, ViewMode::PhotoGrid, "taps in Select Several never open the photo");
    // Done: taps pick one photo again
    let done = widget(&h, "button:selectMode").unwrap();
    tap(&mut h, done.center());
    tap(&mut h, b.1.center());
    assert_eq!(h.app.session.selection.ids.len(), 1);
}

/// An iPad with a trackpad: the screen is a touch screen, but the trackpad's pointer drags photos
/// (to an album) as a mouse does; only a finger's drag scrolls.
#[test]
fn a_trackpad_on_a_touch_screen_still_drags_photos() {
    let mut h = touch_app(ViewMode::PhotoGrid, [1194.0, 834.0]);
    let (_, r) = thumbs(&h, 700.0)[0];
    let mut p = r.center();
    h.input([egui::Event::PointerMoved(p)]);
    h.step();
    h.input([button(p, true)]);
    h.step();
    for _ in 0..6 {
        p += egui::vec2(20.0, 10.0);
        h.input([egui::Event::PointerMoved(p)]);
        h.step();
    }
    assert!(h.app.ui.dragging_photos.is_some(), "the trackpad drags the photo");
    h.input([button(p, false)]);
    h.step();
}

/// Full screen and slideshows end with Esc and pause with Space; a finger gets Exit and Pause.
#[test]
fn full_screen_and_slideshows_have_exit_and_pause_on_a_touch_screen() {
    let mut h = touch_app(ViewMode::Detail, [834.0, 1194.0]);
    h.app.run("view.slideshow", json!({})).unwrap();
    h.step();
    h.step();
    let pause = widget(&h, "button:slideshowPause").expect("Pause on screen");
    tap(&mut h, pause.center());
    assert!(h.app.ui.slideshow.is_some_and(|(_, _, paused)| paused), "paused");
    let exit = widget(&h, "button:fullscreenExit").expect("Exit on screen");
    tap(&mut h, exit.center());
    assert!(!h.app.ui.fullscreen && h.app.ui.slideshow.is_none());
}

/// A section's on/off eye shows on hover with a mouse; on a touch screen it's always there.
#[test]
fn edit_section_eyes_show_without_hovering_on_a_touch_screen() {
    let mut h = touch_app(ViewMode::Detail, [1194.0, 834.0]);
    h.app.ui.right = crate::state::RightPanel::Edit;
    h.settle(SETTLE);
    let eyes = h.app.widgets.iter().filter(|(w, _)| w.starts_with("sectionEye:")).count();
    assert!(eyes > 0, "eyes without a pointer over the headers");
}

/// The phone layout shows Info (and Keywords, Versions, History) in the bottom sheet over the
/// grid, so the selected photos' metadata can be edited there as on the desktop.
#[test]
fn the_phone_grid_shows_info_in_the_sheet() {
    let mut h = touch_app(ViewMode::PhotoGrid, [390.0, 844.0]);
    h.app.run("panel.info", json!({})).unwrap();
    h.settle(SETTLE);
    assert_eq!(h.app.ui.view, ViewMode::PhotoGrid, "the grid stays");
    assert!(widget(&h, "panel:sheet").is_some(), "the sheet is up");
}

#[test]
fn phone_library_navigates_album_and_trash_without_losing_catalog_photos() {
    let mut h = touch_app(ViewMode::PhotoGrid, [390.0, 844.0]);
    let id = h.app.session.visible()[0].0;
    let album = h.app.run("album.create", json!({"name": "Trip"})).unwrap()["id"].as_u64().unwrap();
    h.step();

    let click = |h: &mut Headless, id: &str| {
        let r = h.request("ui.clickWidget", json!({"id": id}), Duration::from_secs(20));
        assert_eq!(r["ok"], true, "{id}: {r}");
        h.step();
        h.step();
    };
    click(&mut h, "icon:sidebar");
    assert!(widget(&h, "button:libraryImport").is_some());
    assert!(widget(&h, "button:libraryCreateAlbum").is_some());
    click(&mut h, "source:all");
    h.app.run("library.select", json!({"ids": [id]})).unwrap();
    click(&mut h, "icon:organize");
    click(&mut h, "button:organizeAdd");
    click(&mut h, &format!("button:organizeAlbum:{album}"));
    assert!(
        h.app.session.catalog.album(lightcraft_catalog::AlbumId(album)).is_some_and(|a| a.photos.contains(&lightcraft_catalog::PhotoId(id))),
        "the phone action adds the selected library photo to the album"
    );
    click(&mut h, "icon:sidebar");
    click(&mut h, &format!("source:album:{album}"));
    assert_eq!(h.app.session.source, lightcraft_engine::LibrarySource::Album(lightcraft_catalog::AlbumId(album)));
    assert!(!h.app.ui.left_panel);
    h.app.run("library.select", json!({"ids": [id]})).unwrap();
    click(&mut h, "icon:organize");
    click(&mut h, "button:organizeRemove");
    assert!(h.app.session.catalog.photo(lightcraft_catalog::PhotoId(id)).is_some(), "removing membership keeps the catalog photo");
    click(&mut h, "icon:sidebar");
    click(&mut h, "source:all");
    assert_eq!(h.app.session.source, lightcraft_engine::LibrarySource::All);
    click(&mut h, "icon:sidebar");
    click(&mut h, "source:all");
    assert!(!h.app.ui.left_panel, "selecting the current source also closes the drawer");
    h.app.run("library.select", json!({"ids": [id]})).unwrap();
    click(&mut h, "icon:organize");
    click(&mut h, "button:organizeDelete");
    click(&mut h, "icon:sidebar");
    click(&mut h, "source:recentlyDeleted");
    assert_eq!(h.app.session.source, lightcraft_engine::LibrarySource::RecentlyDeleted);
    assert!(h.app.session.catalog.photo(lightcraft_catalog::PhotoId(id)).is_some_and(|p| p.deleted));
    h.app.run("library.select", json!({"ids": [id]})).unwrap();
    click(&mut h, "icon:organize");
    click(&mut h, "button:organizeRestore");
    assert!(h.app.session.catalog.photo(lightcraft_catalog::PhotoId(id)).is_some_and(|p| !p.deleted));
}

#[test]
fn phone_library_import_opens_the_host_photo_picker() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let mut h = touch_app(ViewMode::PhotoGrid, [390.0, 844.0]);
    let asked = Arc::new(AtomicBool::new(false));
    let called = asked.clone();
    h.app.services.pick_files = Some(Box::new(move || {
        called.store(true, Ordering::Relaxed);
        Vec::new()
    }));
    let open = h.request("ui.clickWidget", json!({"id": "icon:sidebar"}), Duration::from_secs(20));
    assert_eq!(open["ok"], true, "{open}");
    h.step();
    let import = h.request("ui.clickWidget", json!({"id": "button:libraryImport"}), Duration::from_secs(20));
    assert_eq!(import["ok"], true, "{import}");
    assert!(asked.load(Ordering::Relaxed), "the phone library uses the host picker");
}

/// The phone grid has no thumbnail slider: pinching out shows fewer photos across, in more.
#[test]
fn pinching_the_phone_grid_changes_how_many_photos_go_across() {
    let mut h = touch_app(ViewMode::PhotoGrid, [390.0, 844.0]);
    let cols = h.app.ui.grid_columns;
    let pinch = |h: &mut Headless, from: f32, to: f32| {
        let c = egui::pos2(195.0, 400.0);
        let at = |d: f32| [c - egui::vec2(d, 0.0), c + egui::vec2(d, 0.0)];
        let [a, b] = at(from);
        h.input([finger(egui::TouchPhase::Start, a, 1), finger(egui::TouchPhase::Start, b, 2)]);
        h.step();
        for i in 1..=8 {
            let [a, b] = at(from + (to - from) * i as f32 / 8.0);
            h.input([finger(egui::TouchPhase::Move, a, 1), finger(egui::TouchPhase::Move, b, 2), egui::Event::PointerMoved(a)]);
            h.step();
        }
        let [a, b] = at(to);
        h.input([finger(egui::TouchPhase::End, a, 1), finger(egui::TouchPhase::End, b, 2), egui::Event::PointerGone]);
        h.step();
    };
    pinch(&mut h, 40.0, 70.0);
    assert!(h.app.ui.grid_columns < cols, "pinching out: fewer across ({cols} → {})", h.app.ui.grid_columns);
    let fewer = h.app.ui.grid_columns;
    pinch(&mut h, 120.0, 40.0);
    assert!(h.app.ui.grid_columns > fewer, "pinching in: more across");
}

/// ← / → step the candidate in Compare; the View menu has them for touch screens.
#[test]
fn compare_candidates_step_from_the_menu() {
    let mut h = touch_app(ViewMode::PhotoGrid, [1194.0, 834.0]);
    assert!(!crate::menus::ui_enabled(&h.app, "compare.next"), "only in Compare");
    h.app.run("view.compare", json!({})).unwrap();
    h.settle(SETTLE);
    assert!(crate::menus::ui_enabled(&h.app, "compare.next"));
    let before = crate::panels::compare::compare_pair(&mut h.app);
    h.app.run("compare.next", json!({})).unwrap();
    let after = crate::panels::compare::compare_pair(&mut h.app);
    assert_eq!(before.map(|p| p.0), after.map(|p| p.0), "the select stays");
    assert_ne!(before.map(|p| p.1), after.map(|p| p.1), "the candidate moves on");
    h.app.run("compare.previous", json!({})).unwrap();
    assert_eq!(crate::panels::compare::compare_pair(&mut h.app), before, "and back");
}

/// The editor bar and category strip expose the deep editing tools without scrolling past all
/// other controls. Switching panels does not change develop values until a control is used.
#[test]
fn phone_editor_bar_opens_crop_masks_curve_and_grading() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    let click = |h: &mut Headless, id: &str| {
        let r = h.request("ui.clickWidget", json!({"id": id}), Duration::from_secs(20));
        assert_eq!(r["ok"], true, "{id}: {r}");
        h.step();
        h.step();
    };

    assert!(widget(&h, "icon:crop").is_some());
    if widget(&h, "panel:sheet").is_none() {
        click(&mut h, "icon:edit");
    }
    assert!(widget(&h, "mobileEdit:curve").is_some());
    assert!(widget(&h, "button:auto").is_some(), "quick edits remain available on a phone");
    click(&mut h, "mobileEdit:color");
    assert!(widget(&h, "dropdown:profile").is_some(), "profile selection remains available on a phone");
    assert!(widget(&h, "mobileEdit:grading").is_some(), "Color offers direct Grading access");
    click(&mut h, "mobileEdit:light");
    click(&mut h, "mobileEdit:curve");
    let curve = widget(&h, "curve").expect("curve controls in the sheet");
    let sheet = widget(&h, "panel:sheet").expect("editor sheet");
    assert!(curve.top() < sheet.bottom() - 80.0, "curve canvas must be visible above the tool bar");
    click(&mut h, "mobileEdit:color");
    click(&mut h, "mobileEdit:grading");
    assert!(widget(&h, "icon:grading-3way").is_some(), "color grading controls in the sheet");

    click(&mut h, "icon:crop");
    assert!(widget(&h, "mobileCrop:1x1").is_some(), "aspect presets in Crop");
    click(&mut h, "mobileCrop:1x1");
    let id = h.app.session.active().expect("active photo");
    assert_eq!(h.app.session.develop_of(id).expect("develop settings").crop.aspect, Some((100, 100)));

    click(&mut h, "icon:masking");
    assert!(widget(&h, "mobileMask:Color").is_none(), "adjustments appear after selecting a mask");
    h.app.run("mask.add", json!({"kind": "linear"})).unwrap();
    h.settle(SETTLE);
    assert!(widget(&h, "mobileMaskRail:add").is_some(), "mask creation belongs beside the photo");
    assert!(widget(&h, "mobileMaskRail:overlay").is_some(), "overlay controls belong beside the photo");
    assert!(widget(&h, "mobileMaskRail:visible").is_some(), "selected mask visibility is reachable");
    assert!(widget(&h, "button:mobileMaskCreate").is_none(), "mask list is no longer in the adjustment sheet");
    assert!(widget(&h, "mobileMask:Color").is_some(), "local adjustments have categories");
    click(&mut h, "mobileMask:Color");
    assert!(widget(&h, "slider:hue").is_some(), "Color exposes its local adjustment sliders");
    assert!(widget(&h, "slider:exposure").is_none(), "Light sliders are outside the chosen category");
}

#[test]
fn phone_mask_rail_selects_masks_without_replacing_adjustments() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    h.app.run("panel.masking", json!({})).unwrap();
    h.app.run("mask.add", json!({"kind": "linear"})).unwrap();
    h.app.run("mask.add", json!({"kind": "radial"})).unwrap();
    h.settle(SETTLE);
    let id = h.app.session.active().expect("active photo");
    let masks = h.app.session.develop_of(id).expect("develop settings").masks.clone();
    assert_eq!(masks.len(), 2);
    for mask in &masks {
        assert!(widget(&h, &format!("mask:{}", mask.id)).is_some(), "mask tile in photo rail");
    }
    assert!(widget(&h, "mobileMask:Light").is_some(), "adjustments remain in the sheet");
    let r = h.request("ui.clickWidget", json!({"id": format!("mask:{}", masks[0].id)}), Duration::from_secs(20));
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert_eq!(h.app.session.active_mask, Some(masks[0].id));
    let r = h.request("ui.clickWidget", json!({"id": "mobileMaskRail:add"}), Duration::from_secs(20));
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(widget(&h, "maskNew:linear").is_some(), "rail opens creation choices in the sheet");
}

#[test]
fn small_phone_editor_tools_fit_and_more_opens_secondary_panels() {
    let mut h = touch_app(ViewMode::Detail, [360.0, 740.0]);
    let click = |h: &mut Headless, id: &str| {
        let r = h.request("ui.clickWidget", json!({"id": id}), Duration::from_secs(20));
        assert_eq!(r["ok"], true, "{id}: {r}");
        h.step();
        h.step();
    };
    for id in ["presets", "edit", "crop", "remove", "masking", "more"] {
        let rect = widget(&h, &format!("icon:{id}")).expect("primary tool");
        assert!(rect.left() >= 0.0 && rect.right() <= 360.0, "{id} fits: {rect:?}");
        assert!(rect.width() >= 44.0, "{id} is a finger-sized target");
    }
    click(&mut h, "icon:more");
    for id in ["redeye", "info", "keywords", "versions", "activity"] {
        assert!(widget(&h, &format!("icon:{id}")).is_some(), "{id} in More");
    }
    click(&mut h, "icon:info");
    assert_eq!(h.app.ui.right, crate::state::RightPanel::Info);
    click(&mut h, "icon:crop");
    assert_eq!(h.app.ui.right, crate::state::RightPanel::Crop);
    click(&mut h, "icon:crop");
    assert!(widget(&h, "panel:sheet").is_none(), "the active tool puts the sheet away");
}
