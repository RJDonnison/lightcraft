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

fn photo_swipe(h: &mut Headless, dx: f32) {
    let start = h.app.canvas_rect.expect("photo canvas").center();
    h.input([finger(egui::TouchPhase::Start, start, 31), egui::Event::PointerMoved(start), button(start, true)]);
    h.step();
    let mut end = start;
    for n in 1..=10 {
        end = start + egui::vec2(dx * n as f32 / 10.0, 2.0);
        h.input([finger(egui::TouchPhase::Move, end, 31), egui::Event::PointerMoved(end)]);
        h.step();
    }
    h.input([finger(egui::TouchPhase::End, end, 31), button(end, false)]);
    h.step();
    h.input([egui::Event::PointerGone]);
    h.step();
}

#[test]
fn phone_closed_editor_hides_filmstrip_and_swipes_only_catalog_source() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    h.app.run("library.source", json!({"kind":"myEdits"})).unwrap();
    h.step();
    let edited = h.app.session.visible_cloned();
    assert!(edited.len() >= 2, "demo has multiple edited photos");
    let current = h.app.session.active().expect("an edited photo");
    let at = edited.iter().position(|id| *id == current).expect("active in My Edits");
    let (direction, next) = if let Some(next) = edited.get(at + 1) { (-165.0, *next) } else { (165.0, edited[at - 1]) };
    photo_swipe(&mut h, direction);
    assert_eq!(h.app.session.active(), Some(current), "swipes with the sheet open do not navigate");
    let r = h.request("ui.clickWidget", json!({"id":"icon:edit"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(widget(&h, "panel:sheet").is_none());
    assert!(!h.app.widgets.iter().any(|(id, _)| id.starts_with("film:")), "no phone filmstrip with the sheet closed");
    photo_swipe(&mut h, direction);
    assert_eq!(h.app.session.active(), Some(next), "swipe follows the edited-only catalog source");
    photo_swipe(&mut h, -direction);
    assert_eq!(h.app.session.active(), Some(current));
    h.app.ui.device_gallery = true;
    h.step();
    photo_swipe(&mut h, direction);
    assert_eq!(h.app.session.active(), Some(current), "device browsing must not navigate catalog photos");
}

fn widget(h: &Headless, id: &str) -> Option<egui::Rect> {
    h.app.widgets.iter().find(|(w, _)| w == id).map(|(_, r)| *r)
}

#[test]
fn phone_header_keeps_editor_navigation_but_does_not_export_a_stale_gallery_selection() {
    let services = Services {
        device_gallery: Some(crate::DeviceGallery {
            request: Box::new(|_| Ok(())),
            take_page: Box::new(|| None),
            open: Box::new(|_| Ok(())),
            take_open: Box::new(|| None),
        }),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    app.ui.view = ViewMode::PhotoGrid;
    app.ui.device_gallery = true;
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    assert!(widget(&h, "menu:all").is_some());
    assert!(widget(&h, "icon:back").is_none(), "the gallery tabs are the root navigation");
    assert!(widget(&h, "icon:share").is_none(), "gallery browsing must not export a stale catalog selection");

    h.app.ui.device_gallery = false;
    h.app.ui.view = ViewMode::Detail;
    h.step();
    assert!(widget(&h, "icon:back").is_some());
    assert!(widget(&h, "icon:share").is_some());
    assert!(widget(&h, "icon:more").is_some());
    assert!(widget(&h, "menu:all").is_some());
    let top = widget(&h, "icon:back").unwrap();
    assert!(top.top() >= 0.0 && top.bottom() <= 44.0, "the editor keeps a compact safe header: {top:?}");
    let r = h.request("ui.clickWidget", json!({"id":"icon:more"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    assert!(widget(&h, "icon:info").is_some(), "secondary tools remain reachable");
}

#[test]
fn phone_histogram_toggles_on_photo_without_changing_desktop_preference() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    assert!(!h.app.ui.mobile_histogram, "phone histogram starts hidden");
    assert!(h.app.ui.histogram, "desktop histogram preference remains enabled");
    assert!(widget(&h, "mobileHistogram:toggle").is_some());
    assert!(widget(&h, "mobileHistogram:graph").is_none());
    h.app.run("view.infoOverlay", json!({"mode": "basic"})).unwrap();
    h.step();
    let r = h.request("ui.clickWidget", json!({"id":"mobileHistogram:toggle"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    let graph = widget(&h, "mobileHistogram:graph").expect("histogram over photo");
    let info = widget(&h, "canvas:infoOverlay").expect("info below histogram");
    assert!(graph.left() < 24.0 && graph.bottom() < info.top(), "graph and info do not overlap: {graph:?} {info:?}");
    assert!(graph.width() <= 135.0, "the default graph should leave the photo visible: {graph:?}");
    let r = h.request("ui.clickWidget", json!({"id":"mobileHistogram:expand"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    let detailed = widget(&h, "mobileHistogram:graph").expect("expanded histogram");
    let info = widget(&h, "canvas:infoOverlay").expect("info beneath expanded histogram");
    assert!(detailed.width() > graph.width() && detailed.bottom() < info.top(), "expanded graph and info: {detailed:?} {info:?}");
    let r = h.request("ui.clickWidget", json!({"id":"mobileHistogram:expand"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(widget(&h, "mobileHistogram:graph").is_some_and(|r| r.width() <= 135.0));
    assert!(h.app.ui.histogram);
    let r = h.request("ui.clickWidget", json!({"id":"mobileHistogram:graph"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(!h.app.ui.mobile_histogram);
    assert!(widget(&h, "mobileHistogram:toggle").is_some());
    assert!(widget(&h, "panel:sheet").is_some(), "overlay tap must not dismiss editor");
    let saved = serde_json::to_value(&h.app.ui).unwrap();
    assert_eq!(saved["mobileHistogram"], false);

    let mut desktop = Headless::new(LightcraftApp::new(lightcraft_engine::Session::with_demo(), Services::default()), [600.0, 900.0], 1.0);
    desktop.settle(SETTLE);
    assert!(widget(&desktop, "mobileHistogram:toggle").is_none());
    assert!(widget(&desktop, "histogram").is_some(), "desktop Edit histogram is unchanged");
}

#[test]
fn phone_histogram_expansion_keeps_mask_rail_and_photo_info_separate() {
    for size in [[360.0, 740.0], [844.0, 390.0]] {
        let mut h = touch_app(ViewMode::Detail, size);
        h.app.run("view.infoOverlay", json!({"mode": "basic"})).unwrap();
        h.app.run("panel.masking", json!({})).unwrap();
        h.app.run("mask.add", json!({"kind": "linear"})).unwrap();
        h.step();
        for control in ["mobileHistogram:toggle", "mobileHistogram:expand"] {
            let r = h.request("ui.clickWidget", json!({"id":control}), SETTLE);
            assert_eq!(r["ok"], true, "{control} at {size:?}: {r}");
            h.step();
        }
        let graph = widget(&h, "mobileHistogram:graph").expect("expanded graph");
        let info = widget(&h, "canvas:infoOverlay").expect("info");
        let rail = widget(&h, "mobileMaskRail:add").expect("mask rail");
        assert!(graph.bottom() < info.top(), "histogram must leave photo info legible at {size:?}");
        assert!(graph.right() < rail.left(), "histogram must not cover mask actions at {size:?}");
    }
}

#[test]
fn photo_long_press_temporarily_shows_original_without_opening_menu_or_closing_sheet() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    let pos = widget(&h, "canvas:image").expect("photo").center();
    let id = egui::TouchId(41);
    h.input([
        egui::Event::Touch { device_id: egui::TouchDeviceId(1), id, phase: egui::TouchPhase::Start, pos, force: None },
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
    ]);
    for _ in 0..42 {
        h.step();
    }
    assert!(widget(&h, "mobileOriginal:held").is_some(), "held photo previews the original");
    assert!(widget(&h, "panel:sheet").is_some());
    h.input([
        egui::Event::Touch { device_id: egui::TouchDeviceId(1), id, phase: egui::TouchPhase::End, pos, force: None },
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
    ]);
    h.step();
    assert!(widget(&h, "mobileOriginal:held").is_none(), "release restores edits");
    assert!(widget(&h, "panel:sheet").is_some(), "release doesn't dismiss the editor");
    assert_eq!(h.app.ui.before_after, crate::state::BeforeAfter::Off);
}

#[test]
fn phone_export_uses_small_chooser_and_mobile_publisher() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let published = Arc::new(AtomicUsize::new(0));
    let count = published.clone();
    let dir = std::env::temp_dir().join(format!("lc-phone-export-{}", std::process::id()));
    let services = Services {
        export_dir: Some(dir.to_string_lossy().into_owned()),
        write_shared: Some(Arc::new(|_, _| panic!("mobile export must not use the normal share writer"))),
        mobile_export: Some(Arc::new(move |_, bytes| {
            assert!(!bytes.is_empty());
            count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    app.ui.view = ViewMode::Detail;
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    let r = h.request("ui.clickWidget", json!({"id":"icon:share"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    assert!(widget(&h, "mobileExport:submit").is_some());
    assert!(h.app.ui.dialog.is_none(), "the desktop export dialog must remain closed");
    for id in ["mobileExport:creator", "mobileExport:copyright", "mobileExport:longEdge", "mobileExport:quality"] {
        assert!(widget(&h, id).is_some(), "phone export exposes {id}");
    }
    let r = h.request("ui.clickWidget", json!({"id":"mobileExport:creator"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    let r = h.request("ui.text", json!({"text":"Ada Camera"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    let r = h.request("ui.clickWidget", json!({"id":"mobileExport:copyright"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    let r = h.request("ui.text", json!({"text":"© Ada"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.view.ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("mobile-export-long-edge"), 64_i64);
        d.insert_temp(egui::Id::new("mobile-export-quality"), 72_i64);
    });
    h.step();
    let r = h.request("ui.clickWidget", json!({"id":"mobileExport:submit"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    let last = h.app.session.last_export.as_ref().expect("reusable mobile export settings");
    assert_eq!((last["mobileCreator"].as_str(), last["mobileCopyright"].as_str()), (Some("Ada Camera"), Some("© Ada")));
    assert_eq!((last["longEdge"].as_i64(), last["quality"].as_i64()), (Some(64), Some(72)));
    assert!(h.app.run("export.savePreset", json!({"name":"Phone test"})).is_ok(), "mobile attribution does not invalidate an export preset");
    assert!(h.app.export.is_some(), "export should run as a cancellable background task");
    for _ in 0..200 {
        h.step();
        if published.load(Ordering::SeqCst) > 0 || h.app.export.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        published.load(Ordering::SeqCst) > 0,
        "the Android publisher receives rendered bytes: export={:?}, toast={:?}",
        h.app.last_export_result,
        h.app.ui.toast
    );
}

#[test]
fn phone_export_reports_publication_failure_without_success() {
    use std::sync::Arc;

    let services = Services {
        export_dir: Some(std::env::temp_dir().join(format!("lc-phone-error-{}", std::process::id())).to_string_lossy().into_owned()),
        mobile_export: Some(Arc::new(|_, _| Err("Photos storage is full".into()))),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    app.ui.view = ViewMode::Detail;
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    let result = h.app.run("app.export", json!({"format": "jpeg", "mobileDestination": true, "background": true}));
    assert!(result.is_ok(), "{result:?}");
    for _ in 0..200 {
        h.step();
        if h.app.export.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let files = h.app.last_export_result.as_ref().and_then(|v| v["files"].as_array());
    assert!(files.is_some_and(|f| f.iter().any(|entry| entry["error"].as_str().is_some_and(|e| e.contains("Photos storage is full")))));
    assert!(!files.is_some_and(|f| f.iter().any(|entry| entry["path"].is_string())));
}

#[test]
fn small_phone_export_only_shows_controls_the_format_supports() {
    use std::sync::Arc;
    let services = Services { mobile_export: Some(Arc::new(|_, _| Ok(()))), ..Default::default() };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    app.ui.view = ViewMode::Detail;
    let mut h = Headless::new(app, [360.0, 740.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    assert_eq!(h.request("ui.clickWidget", json!({"id":"icon:share"}), SETTLE)["ok"], true);
    let button = widget(&h, "mobileExport:submit").expect("export action");
    assert!(button.bottom() <= 740.0, "export remains reachable on a small phone: {button:?}");
    assert!(widget(&h, "mobileExport:quality").is_some());
    assert!(widget(&h, "mobileExport:longEdge").is_some());

    assert_eq!(h.request("ui.clickWidget", json!({"id":"mobileExport:format"}), SETTLE)["ok"], true);
    assert_eq!(h.request("ui.clickWidget", json!({"id":"mobileExport:png"}), SETTLE)["ok"], true);
    assert!(widget(&h, "mobileExport:quality").is_none(), "PNG does not use lossy quality");
    assert!(widget(&h, "mobileExport:longEdge").is_some());
    assert_eq!(h.request("ui.clickWidget", json!({"id":"mobileExport:format"}), SETTLE)["ok"], true);
    assert_eq!(h.request("ui.clickWidget", json!({"id":"mobileExport:original"}), SETTLE)["ok"], true);
    assert!(widget(&h, "mobileExport:longEdge").is_none(), "originals are not resized");
    assert!(widget(&h, "mobileExport:quality").is_none());
}

#[test]
fn phone_device_photos_browse_previews_without_import_then_switch_to_edits() {
    use std::sync::{Arc, Mutex};

    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_seen = requests.clone();
    let mut first = true;
    let services = Services {
        device_gallery: Some(crate::DeviceGallery {
            request: Box::new(move |offset| {
                requests_seen.lock().unwrap().push(offset);
                Ok(())
            }),
            take_page: Box::new(move || {
                if !first {
                    return None;
                }
                first = false;
                Some(crate::DevicePage::Photos {
                    offset: 0,
                    more: false,
                    photos: vec![crate::DevicePhoto {
                        id: "42".into(),
                        name: "From phone".into(),
                        preview: Some(egui::ColorImage::filled([2, 2], egui::Color32::RED)),
                    }],
                })
            }),
            open: Box::new(|_| panic!("browsing must not import originals")),
            take_open: Box::new(|| None),
        }),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    app.ui.view = ViewMode::PhotoGrid;
    let album = app.run("album.create", json!({"name":"Phone albums"})).expect("create a catalog album")["id"].as_u64().expect("album id");
    let catalog_count = app.session.catalog.photos().count();
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    assert!(widget(&h, "galleryTab:Device Photos").is_some());
    assert!(widget(&h, "galleryTab:My Edits").is_some());
    assert!(widget(&h, "galleryTab:Library").is_some());
    let r = h.request("ui.clickWidget", json!({"id":"galleryTab:Device Photos"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    assert!(widget(&h, "icon:edit").is_none(), "device browsing cannot edit a stale catalog selection");
    assert!(widget(&h, "devicePhoto:42").is_some());
    let cell = widget(&h, "devicePhoto:42").expect("gallery cell");
    assert!((cell.width() - cell.height()).abs() < 1.0, "gallery previews are square: {cell:?}");
    assert!(widget(&h, "devicePhotoLabel:42").is_none(), "gallery has no filename rows");
    assert_eq!(h.app.session.catalog.photos().count(), catalog_count, "previews are not imported photos");
    let r = h.request("ui.clickWidget", json!({"id":"galleryTab:My Edits"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    assert!(!h.app.ui.device_gallery);
    assert_eq!(h.app.session.source, lightcraft_engine::LibrarySource::MyEdits);
    let r = h.request("ui.clickWidget", json!({"id":"galleryTab:Library"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    assert!(h.app.ui.library_home);
    assert!(widget(&h, "source:all").is_some(), "catalog sources are under Library");
    assert!(widget(&h, &format!("source:album:{album}")).is_some(), "albums are under Library");
    assert!(widget(&h, "button:libraryImport").is_some(), "manual import remains available");
    let r = h.request("ui.clickWidget", json!({"id":"source:all"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    assert!(!h.app.ui.library_home);
    assert_eq!(h.app.session.source, lightcraft_engine::LibrarySource::All);
    let r = h.request("ui.clickWidget", json!({"id":"galleryTab:Library"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    let r = h.request("ui.clickWidget", json!({"id":format!("source:album:{album}")}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    assert!(!h.app.ui.library_home);
    assert_eq!(h.app.session.source, lightcraft_engine::LibrarySource::Album(lightcraft_catalog::AlbumId(album)));
    assert!(requests.lock().unwrap().is_empty() || requests.lock().unwrap().as_slice() == [0]);
}

#[test]
fn phone_device_gallery_scrolls_continuously_and_reloads_earlier_thumbnails() {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    let replies = Arc::new(Mutex::new(VecDeque::new()));
    let offsets = Arc::new(Mutex::new(Vec::new()));
    let for_offsets = offsets.clone();
    let for_request = replies.clone();
    let for_poll = replies.clone();
    let services = Services {
        device_gallery: Some(crate::DeviceGallery {
            request: Box::new(move |offset| {
                for_offsets.lock().unwrap().push(offset);
                let photos = (offset..offset + 48)
                    .map(|id| crate::DevicePhoto {
                        id: id.to_string(),
                        name: format!("Photo {id}"),
                        preview: Some(egui::ColorImage::filled([3, 2], egui::Color32::RED)),
                    })
                    .collect();
                for_request.lock().unwrap().push_back(crate::DevicePage::Photos { offset, photos, more: offset < 96 });
                Ok(())
            }),
            take_page: Box::new(move || for_poll.lock().unwrap().pop_front()),
            open: Box::new(|_| panic!("paging does not import originals")),
            take_open: Box::new(|| None),
        }),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    app.ui.view = ViewMode::PhotoGrid;
    app.ui.device_gallery = true;
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    assert!(widget(&h, "devicePhoto:0").is_some());
    assert!(widget(&h, "gallery:older").is_none(), "there are no visible pages");
    let r = h.request("ui.hoverWidget", json!({"id":"devicePhoto:0"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    let r = h.request("ui.scroll", json!({"dx":0,"dy":-3000}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    assert!(offsets.lock().unwrap().contains(&48), "scrolling fetches the next bounded batch");
    let r = h.request("ui.scroll", json!({"dx":0,"dy":-1800}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    assert!(
        h.app.widgets.iter().any(|(id, _)| id.strip_prefix("devicePhoto:").and_then(|id| id.parse::<usize>().ok()).is_some_and(|id| id >= 48)),
        "newly fetched photos join the same scrollable grid"
    );
    assert!(widget(&h, "devicePhoto:0").is_none(), "far-away cells are virtualized");
    let r = h.request("ui.scroll", json!({"dx":0,"dy":6000}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    assert!(widget(&h, "devicePhoto:0").is_some());
    assert!(offsets.lock().unwrap().iter().filter(|&&offset| offset == 0).count() >= 2, "returning reloads released thumbnails");
}

#[test]
fn phone_device_photo_imports_once_on_open_and_reopens_existing_edit() {
    use std::sync::{Arc, Mutex};

    let dir = std::env::temp_dir().join(format!("lc-device-open-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("device-42.png");
    let image = lightcraft_raster::Rgba8 { width: 8, height: 8, data: vec![[90, 30, 10, 255]; 64] };
    let png = lightcraft_codecs::encode_png(&lightcraft_codecs::EncodeImage::rgba8(&image), &lightcraft_codecs::EncodeMeta::default()).unwrap();
    std::fs::write(&path, png).unwrap();
    let ready = Arc::new(Mutex::new(Vec::new()));
    let opened = ready.clone();
    let polled = ready.clone();
    let mut first = true;
    let services = Services {
        device_gallery: Some(crate::DeviceGallery {
            request: Box::new(|_| Ok(())),
            take_page: Box::new(move || {
                if !first {
                    return None;
                }
                first = false;
                Some(crate::DevicePage::Photos {
                    offset: 0,
                    more: false,
                    photos: vec![crate::DevicePhoto { id: "42".into(), name: "Phone photo".into(), preview: None }],
                })
            }),
            open: Box::new(move |id| {
                assert_eq!(id, "42");
                opened.lock().unwrap().push(Ok(path.to_string_lossy().to_string()));
                Ok(())
            }),
            take_open: Box::new(move || polled.lock().unwrap().pop()),
        }),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo().with_fs(), services);
    app.ui.view = ViewMode::PhotoGrid;
    app.ui.device_gallery = true;
    let n = app.session.catalog.len();
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    for attempt in 0..2 {
        let r = h.request("ui.clickWidget", json!({"id":"devicePhoto:42"}), SETTLE);
        assert_eq!(r["ok"], true, "{r}");
        h.step_until(SETTLE, |h| h.app.ui.view == ViewMode::Detail);
        assert_eq!(h.app.session.catalog.len(), n + 1, "opening {attempt} must not duplicate");
        assert!(h.app.ui.device_gallery, "Back must return to the same Device Photos listing");
        if attempt == 0 {
            h.app.run("develop.set", json!({"control":"light.exposure", "value":0.7})).unwrap();
            assert_eq!(h.request("engine.execute", json!({"command":"view.back"}), SETTLE)["ok"], true);
            assert!(widget(&h, "devicePhoto:42").is_some());
        }
    }
    assert_eq!(h.app.session.catalog.photo(h.app.session.active().unwrap()).unwrap().develop.light.exposure, 0.7);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn phone_returns_to_album_scroll_and_selection_after_editing() {
    for size in [[390.0, 844.0], [844.0, 390.0]] {
        let mut h = touch_app(ViewMode::PhotoGrid, size);
        let album = h.app.run("album.create", json!({"name":"Review"})).unwrap()["id"].as_u64().unwrap();
        let ids: Vec<_> = h.app.session.visible_cloned().iter().map(|id| id.0).collect();
        h.app.run("album.addPhotos", json!({"id": album, "ids": ids})).unwrap();
        h.app.run("library.source", json!({"kind":"album", "id":album})).unwrap();
        h.app.run("library.sort", json!({"key":"fileName"})).unwrap();
        h.app.run("library.filter", json!({"text":"."})).unwrap();
        h.request("ui.set", json!({"thumbSize": 320.0}), SETTLE);
        h.step();
        let canvas = h.app.canvas_rect.unwrap();
        h.request("ui.move", json!({"x":canvas.center().x, "y":canvas.center().y}), SETTLE);
        h.request("ui.scroll", json!({"dy": -310.0}), SETTLE);
        h.step();
        let source = h.app.session.source;
        let sort = h.app.session.sort;
        let filter = h.app.session.filter.clone();
        let selection = h.app.session.selection.clone();
        let scroll = h.app.grid_scroll.unwrap_or_default();
        assert!(scroll > 0.0, "grid must have moved before opening Edit at {size:?}");
        h.app.run("view.detail", json!({})).unwrap();
        h.step();
        h.app.run("develop.set", json!({"control":"light.exposure", "value":0.4})).unwrap();
        h.app.run("view.back", json!({})).unwrap();
        h.step();
        assert_eq!(h.app.session.source, source);
        assert_eq!(h.app.session.sort, sort);
        assert_eq!(h.app.session.filter, filter);
        assert_eq!(h.app.session.selection, selection);
        assert!((h.app.grid_scroll.unwrap_or_default() - scroll).abs() < 2.0, "grid scroll at {size:?}");
    }
}

#[test]
fn device_gallery_permission_revocation_clears_cached_previews() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let revoked = Arc::new(AtomicBool::new(false));
    let poll_revoked = revoked.clone();
    let mut received = false;
    let services = Services {
        device_gallery: Some(crate::DeviceGallery {
            request: Box::new(|_| Ok(())),
            take_page: Box::new(move || {
                if !received {
                    received = true;
                    Some(crate::DevicePage::Photos {
                        offset: 0,
                        photos: vec![crate::DevicePhoto { id: "7".into(), name: "Granted".into(), preview: None }],
                        more: false,
                    })
                } else if poll_revoked.swap(false, Ordering::Relaxed) {
                    Some(crate::DevicePage::PermissionNeeded)
                } else {
                    None
                }
            }),
            open: Box::new(|_| Ok(())),
            take_open: Box::new(|| None),
        }),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    app.ui.view = ViewMode::PhotoGrid;
    app.ui.device_gallery = true;
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    assert!(widget(&h, "devicePhoto:7").is_some());
    revoked.store(true, Ordering::Relaxed);
    h.step();
    assert!(widget(&h, "devicePhoto:7").is_none(), "revocation removes stale previews");
    assert!(widget(&h, "gallery:retry").is_some(), "permission state offers an actionable retry");
    let r = h.request("ui.clickWidget", json!({"id":"gallery:retry"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
}

#[test]
fn empty_device_gallery_waits_for_retry_instead_of_querying_each_frame() {
    use std::sync::{Arc, Mutex};

    let requests = Arc::new(Mutex::new(0usize));
    let seen = requests.clone();
    let mut delivered = false;
    let services = Services {
        device_gallery: Some(crate::DeviceGallery {
            request: Box::new(move |_| {
                *seen.lock().unwrap() += 1;
                Ok(())
            }),
            take_page: Box::new(move || {
                if delivered {
                    None
                } else {
                    delivered = true;
                    Some(crate::DevicePage::Photos { offset: 0, photos: Vec::new(), more: false })
                }
            }),
            open: Box::new(|_| Ok(())),
            take_open: Box::new(|| None),
        }),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    app.ui.view = ViewMode::PhotoGrid;
    app.ui.device_gallery = true;
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    let before = *requests.lock().unwrap();
    for _ in 0..10 {
        h.step();
    }
    assert_eq!(*requests.lock().unwrap(), before, "an empty gallery must not repeatedly query MediaStore");
    assert!(widget(&h, "gallery:retry").is_some(), "empty gallery offers one recovery action");
    let r = h.request("ui.clickWidget", json!({"id":"gallery:retry"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(*requests.lock().unwrap() > before, "retry starts a fresh query");
}

#[test]
fn failed_device_gallery_has_one_explicit_retry_without_background_retry_loop() {
    use std::sync::{Arc, Mutex};

    let requests = Arc::new(Mutex::new(0usize));
    let seen = requests.clone();
    let services = Services {
        device_gallery: Some(crate::DeviceGallery {
            request: Box::new(move |_| {
                let mut count = seen.lock().unwrap();
                *count += 1;
                Err("Gallery is temporarily unavailable".into())
            }),
            take_page: Box::new(|| None),
            open: Box::new(|_| Ok(())),
            take_open: Box::new(|| None),
        }),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::new(), services);
    app.ui.view = ViewMode::PhotoGrid;
    app.ui.device_gallery = true;
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    assert_eq!(*requests.lock().unwrap(), 1);
    assert!(widget(&h, "gallery:retry").is_some());
    for _ in 0..10 {
        h.step();
    }
    assert_eq!(*requests.lock().unwrap(), 1, "errors wait for an explicit retry");
    let r = h.request("ui.clickWidget", json!({"id":"gallery:retry"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert_eq!(*requests.lock().unwrap(), 2);
}

#[test]
fn empty_phone_edits_offer_a_direct_path_back_to_device_photos() {
    let services = Services {
        device_gallery: Some(crate::DeviceGallery {
            request: Box::new(|_| Ok(())),
            take_page: Box::new(|| None),
            open: Box::new(|_| Ok(())),
            take_open: Box::new(|| None),
        }),
        ..Default::default()
    };
    let mut app = LightcraftApp::new(lightcraft_engine::Session::new(), services);
    app.ui.view = ViewMode::PhotoGrid;
    app.run("library.source", json!({"kind":"myEdits"})).unwrap();
    let mut h = Headless::new(app, [390.0, 844.0], 1.0);
    h.input([finger(egui::TouchPhase::Cancel, egui::Pos2::ZERO, 0)]);
    h.settle(SETTLE);
    assert!(widget(&h, "gallery:fromEmptyEdits").is_some());
    let r = h.request("ui.clickWidget", json!({"id":"gallery:fromEmptyEdits"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(h.app.ui.device_gallery);
    assert!(widget(&h, "galleryTab:Device Photos").is_some());
}

#[test]
fn phone_presets_show_photo_variants_without_a_hover_or_setting() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    assert!(!h.app.ui.preset_thumbs, "desktop preference stays off");
    let r = h.request("ui.clickWidget", json!({"id":"icon:presets"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    let sheet = widget(&h, "panel:sheet").expect("Presets sheet");
    let (_, thumb) = h.app.widgets.iter().find(|(id, r)| id.starts_with("presetThumb:") && sheet.contains(r.center())).expect("visible preset image");
    assert!(thumb.width() >= 90.0 && thumb.height() >= 70.0, "photo-derived look is large enough to compare");
    assert!(!h.app.ui.preset_thumbs, "touch layout does not change the saved desktop setting");
}

#[test]
fn phone_sheet_has_one_height_and_dismisses_on_photo_tap() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    assert!(widget(&h, "panel:sheet").is_some());
    assert!(widget(&h, "mobileSheet:handle").is_none(), "no sheet drag bar");
    assert!(widget(&h, "icon:sheetClose").is_none(), "no duplicate heading or Hide button");
    let height = widget(&h, "panel:sheet").unwrap().height();
    for _ in 0..3 {
        h.step();
    }
    assert!((widget(&h, "panel:sheet").unwrap().height() - height).abs() < 2.0);

    let canvas = h.app.canvas_rect.expect("photo canvas");
    tap(&mut h, canvas.center());
    assert!(widget(&h, "panel:sheet").is_none(), "tapping the photo dismisses controls");
    assert_eq!(h.app.ui.zoom, crate::state::Zoom::Fit, "dismissal does not also zoom the photo");
}

#[test]
fn small_phone_shows_all_edit_mask_and_crop_choices_without_sideways_scrolling() {
    let mut h = touch_app(ViewMode::Detail, [360.0, 740.0]);
    let click = |h: &mut Headless, id: &str| {
        let result = h.request("ui.clickWidget", json!({"id": id}), SETTLE);
        assert_eq!(result["ok"], true, "{id}: {result}");
        h.step();
    };
    let all_fit = |h: &Headless, ids: &[&str]| {
        let sheet = widget(h, "panel:sheet").expect("sheet");
        for id in ids {
            let r = widget(h, id).expect("option exists");
            assert!(r.left() >= sheet.left() && r.right() <= sheet.right(), "{id} fits horizontally: {r:?} in {sheet:?}");
        }
    };
    all_fit(&h, &["mobileEdit:light", "mobileEdit:color", "mobileEdit:effects", "mobileEdit:detail", "mobileEdit:optics"]);
    assert!(widget(&h, "mobileEdit:calibration").is_none(), "Calibration belongs inside Color rather than a wrapped primary tab");
    click(&mut h, "icon:crop");
    all_fit(&h, &["mobileCrop:free", "mobileCrop:original", "mobileCrop:1x1", "mobileCrop:4x5", "mobileCrop:16x9"]);
    click(&mut h, "icon:masking");
    h.app.run("mask.add", json!({"kind": "linear"})).unwrap();
    h.settle(SETTLE);
    all_fit(&h, &["mobileMask:Light", "mobileMask:Color", "mobileMask:Effects", "mobileMask:Detail"]);
}

#[test]
fn phone_edit_categories_remain_above_scrolling_controls_and_preserve_edits() {
    for size in [[390.0, 844.0], [844.0, 390.0]] {
        let mut h = touch_app(ViewMode::Detail, size);
        let photo = h.app.session.active().unwrap();
        let sheet = widget(&h, "panel:sheet").unwrap();
        let category = widget(&h, "mobileEdit:light").unwrap();
        assert!(category.top() >= sheet.top() && category.bottom() < sheet.center().y);
        h.app.run("develop.set", json!({"control":"light.exposure", "value":0.7})).unwrap();
        h.request("ui.move", json!({"x":sheet.center().x, "y":sheet.bottom() - 54.0}), SETTLE);
        h.request("ui.scroll", json!({"dy":-300.0}), SETTLE);
        h.step();
        assert!((widget(&h, "mobileEdit:light").unwrap().top() - category.top()).abs() < 2.0);
        let selected = h.app.session.selection.clone();
        let image = h.app.image_rect.unwrap();
        for name in ["color", "effects", "detail", "optics", "light"] {
            assert_eq!(h.request("ui.clickWidget", json!({"id":format!("mobileEdit:{name}")}), SETTLE)["ok"], true);
        }
        assert_eq!(h.app.session.selection, selected);
        assert_eq!(h.app.session.active(), Some(photo));
        assert_eq!(h.app.ui.zoom, crate::state::Zoom::Fit);
        assert!((h.app.image_rect.unwrap().center().x - image.center().x).abs() < 2.0);
        assert!((h.app.session.develop_of(photo).unwrap().light.exposure - 0.7).abs() < 0.001);
    }
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
    let curve = widget(&h, "curve").expect("curve graph in the focused workspace");
    let workspace = widget(&h, "mobileCurve:workspace").expect("focused curve workspace");
    assert!(widget(&h, "panel:sheet").is_none(), "the short editor sheet must not compete with the Curve workspace");
    assert!(curve.height() >= 250.0, "the focused phone graph has room for precise points: {curve:?}");
    assert!(curve.top() >= workspace.top() && curve.top() < workspace.bottom(), "graph begins within the workspace");
    click(&mut h, "button:mobileCurveDone");
    assert!(widget(&h, "panel:sheet").is_some(), "Done returns to the ordinary Edit sheet");
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
fn landscape_phone_curve_leaves_the_photo_visible_beside_the_workspace() {
    let mut h = touch_app(ViewMode::Detail, [844.0, 390.0]);
    if widget(&h, "panel:sheet").is_none() {
        h.app.run("panel.edit", json!({})).unwrap();
        h.step();
    }
    let open = h.request("ui.clickWidget", json!({"id": "mobileEdit:curve"}), Duration::from_secs(20));
    assert_eq!(open["ok"], true, "{open}");
    h.step();
    h.step();
    let workspace = widget(&h, "mobileCurve:workspace").expect("landscape curve workspace");
    let photo = h.app.image_rect.expect("photo beside curve");
    assert!(photo.right() <= workspace.left(), "the curve workspace must not cover the photo: {photo:?} {workspace:?}");
    assert!(photo.width() >= 100.0 && photo.height() >= 150.0, "photo remains useful alongside the graph: {photo:?}");
    assert!(widget(&h, "curve").is_some());
    let done = h.request("ui.clickWidget", json!({"id": "button:mobileCurveDone"}), Duration::from_secs(20));
    assert_eq!(done["ok"], true, "{done}");
    h.step();
    assert!(widget(&h, "panel:sheet").is_some());
}

#[test]
fn phone_curve_channels_keep_independent_points_and_anchor_endpoints() {
    for size in [[390.0, 844.0], [844.0, 390.0]] {
        let mut h = touch_app(ViewMode::Detail, size);
        let photo = h.app.session.active().unwrap();
        h.app.run("develop.set", json!({"control":"light.exposure", "value":0.6})).unwrap();
        let open = h.request("ui.clickWidget", json!({"id":"mobileEdit:curve"}), SETTLE);
        assert_eq!(open["ok"], true, "{open}");
        h.settle(SETTLE);
        let graph = widget(&h, "curve").unwrap();
        assert_eq!(h.app.ui.curve_channel, "master");
        assert!(graph.top() >= widget(&h, "mobileCurve:workspace").unwrap().top());
        for (ch, y) in [("master", 0.35), ("red", 0.6), ("green", 0.45), ("blue", 0.75)] {
            let choose = h.request("ui.clickWidget", json!({"id":format!("curveChannel:{ch}")}), SETTLE);
            assert_eq!(choose["ok"], true, "{choose}");
            h.app.run("develop.curve", json!({"channel":ch,"points":[[0.0,0.0],[0.5,y],[1.0,1.0]]})).unwrap();
            h.step();
        }
        let drag = h.request("ui.dragWidget", json!({"id":"curve","fx":1.0,"fy":0.0,"dx":-30.0,"dy":35.0,"steps":12}), SETTLE);
        assert_eq!(drag["ok"], true, "{drag}");
        let drag = h.request("ui.dragWidget", json!({"id":"curve","fx":0.0,"fy":1.0,"dx":30.0,"dy":-35.0,"steps":12}), SETTLE);
        assert_eq!(drag["ok"], true, "{drag}");
        let d = h.app.session.develop_of(photo).unwrap();
        assert_eq!(d.curve.blue.first().unwrap().x, 0.0, "endpoint stays on the left edge");
        assert_eq!(d.curve.blue.last().unwrap().x, 1.0, "endpoint stays on the right edge");
        assert!((d.curve.master[1].y - 0.35).abs() < 0.001);
        assert!((d.curve.red[1].y - 0.6).abs() < 0.001);
        assert!((d.curve.green[1].y - 0.45).abs() < 0.001);
        drop(d);
        let workspace = widget(&h, "mobileCurve:workspace").unwrap();
        h.request("ui.move", json!({"x":workspace.center().x,"y":workspace.bottom()-35.0}), SETTLE);
        h.request("ui.scroll", json!({"dy":-400.0}), SETTLE);
        h.settle(SETTLE);
        let reset = h.request("ui.clickWidget", json!({"id":"mobileCurve:resetChannel"}), SETTLE);
        assert_eq!(reset["ok"], true, "{reset}");
        let d = h.app.session.develop_of(photo).unwrap();
        assert!(d.curve.blue.is_empty(), "only the selected Blue channel resets");
        assert_eq!(d.curve.red.len(), 3);
        drop(d);
        let reset_all = h.request("ui.clickWidget", json!({"id":"mobileCurve:resetAll"}), SETTLE);
        assert_eq!(reset_all["ok"], true, "{reset_all}");
        assert!(h.app.session.develop_of(photo).unwrap().curve.red.is_empty());
        let done = h.request("ui.clickWidget", json!({"id":"button:mobileCurveDone"}), SETTLE);
        assert_eq!(done["ok"], true, "{done}");
        assert!(widget(&h, "mobileEdit:curve").is_some(), "Done returns to Light");
        assert_eq!(h.app.session.active(), Some(photo));
        assert!((h.app.session.develop_of(photo).unwrap().light.exposure - 0.6).abs() < 0.001);
        assert!(!h.app.session.undo.is_empty());
    }
}

#[test]
fn phone_crop_uses_visual_aspects_separate_geometry_and_large_handles() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    h.app.run("panel.crop", json!({})).unwrap();
    h.settle(SETTLE);
    assert!(widget(&h, "mobileCropTab:Crop").is_some());
    for aspect in ["free", "original", "1x1", "4x5", "16x9"] {
        let rect = widget(&h, &format!("mobileCrop:{aspect}")).expect("visual aspect card");
        assert!(rect.width() >= 56.0 && rect.height() >= 52.0);
    }
    let photo = h.app.session.active().expect("photo");
    let corner = widget(&h, "cropHandle:0").expect("visible corner");
    assert!(corner.width() >= 44.0 && corner.height() >= 44.0);
    let r = h.request("ui.clickWidget", json!({"id": "mobileCropTab:Geometry"}), Duration::from_secs(20));
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(widget(&h, "mobileCropTab:Crop").is_some());
    assert!(widget(&h, "mobileCrop:1x1").is_none(), "aspect cards belong only in Crop");
    assert!(widget(&h, "slider:geometry.vertical").is_some() || widget(&h, "segmented:upright").is_some(), "Geometry tools are visible");
    let r = h.request("ui.clickWidget", json!({"id": "mobileCropTab:Crop"}), Duration::from_secs(20));
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    let r = h.request("ui.clickWidget", json!({"id": "mobileCrop:1x1"}), Duration::from_secs(20));
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert_eq!(h.app.session.develop_of(photo).expect("develop").crop.aspect, Some((100, 100)));

    let mut small = touch_app(ViewMode::Detail, [360.0, 740.0]);
    small.app.run("panel.crop", json!({})).unwrap();
    small.settle(SETTLE);
    let first = widget(&small, "mobileCrop:free").expect("first ratio card");
    let last = widget(&small, "mobileCrop:16x9").expect("last ratio card");
    assert!((first.top() - last.top()).abs() < 1.0, "all common ratios fit in one row on a small phone: first={first:?} last={last:?}");
}

#[test]
fn phone_crop_keeps_precision_controls_available_without_repeating_them_by_default() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    h.app.run("panel.crop", json!({})).unwrap();
    h.settle(SETTLE);
    assert!(widget(&h, "mobileCrop:1x1").is_some());
    assert!(widget(&h, "slider:crop.angle").is_some(), "straighten is immediately available");
    assert!(widget(&h, "dropdown:cropAspect").is_none(), "the common ratios are not repeated by default");
    assert!(widget(&h, "cropAngleField").is_none(), "exact entry stays behind the precision control");
    let more = h.request("ui.clickWidget", json!({"id": "button:mobileCropOptions"}), Duration::from_secs(20));
    assert_eq!(more["ok"], true, "{more}");
    h.step();
    assert!(widget(&h, "dropdown:cropAspect").is_some(), "custom ratios remain available");
    assert!(widget(&h, "cropAngleField").is_some(), "exact angles remain available");
    assert!(widget(&h, "button:cropRotateLeft").is_some(), "rotation remains reachable");
    let less = h.request("ui.clickWidget", json!({"id": "button:mobileCropOptions"}), Duration::from_secs(20));
    assert_eq!(less["ok"], true, "{less}");
    h.step();
    h.step();
    assert!(widget(&h, "dropdown:cropAspect").is_none());
    assert!(widget(&h, "slider:crop.angle").is_some());
}

#[test]
fn phone_masking_keeps_the_photo_visible_until_a_mask_has_adjustments() {
    for size in [[360.0, 740.0], [844.0, 390.0]] {
        let mut h = touch_app(ViewMode::Detail, size);
        h.app.run("panel.masking", json!({})).unwrap();
        h.settle(SETTLE);
        assert!(widget(&h, "mobileMaskRail:add").is_some());
        assert!(widget(&h, "panel:sheet").is_none(), "no selected mask has adjustments to show");
        let add = h.request("ui.clickWidget", json!({"id":"mobileMaskRail:add"}), SETTLE);
        assert_eq!(add["ok"], true, "{add}");
        assert!(widget(&h, "maskNew:linear").is_some(), "creation remains in the photo-side rail");
        h.app.run("mask.add", json!({"kind":"linear"})).unwrap();
        h.settle(SETTLE);
        assert!(widget(&h, "panel:sheet").is_some(), "selecting a mask reveals its adjustment sheet");
        assert!(widget(&h, "mobileMask:Light").is_some());
    }
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
    assert!(widget(&h, "button:maskAddComp").is_none(), "management is absent from the lower adjustments");
    assert!(widget(&h, "component:0").is_none(), "component tree is absent from the lower adjustments");
    let r = h.request("ui.clickWidget", json!({"id": format!("mask:{}", masks[0].id)}), Duration::from_secs(20));
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert_eq!(h.app.session.active_mask, Some(masks[0].id));
    let r = h.request("ui.clickWidget", json!({"id": "mobileMaskRail:add"}), Duration::from_secs(20));
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(widget(&h, "maskNew:linear").is_some(), "rail opens creation choices beside the photo");
    let r = h.request("ui.clickWidget", json!({"id": "mobileMaskRail:add"}), Duration::from_secs(20));
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(widget(&h, "mobileMaskRail:manage").is_none(), "management has no separate ellipsis control");
    let r = h.request("ui.clickWidget", json!({"id": format!("mask:{}", masks[0].id), "button": "right"}), Duration::from_secs(20));
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!(widget(&h, "button:maskAddComp").is_some(), "the side rail opens component management");
    assert!(widget(&h, "component:0").is_some(), "the component tree lives in rail management");
    h.settle(SETTLE);
    assert!(widget(&h, &format!("mobileMaskRail:preview:{}", masks[0].id)).is_some(), "effective mask has a white-on-black preview");
}

#[test]
fn holding_an_unselected_mask_opens_its_management_without_a_photo_gesture() {
    for size in [[390.0, 844.0], [844.0, 390.0]] {
        let mut h = touch_app(ViewMode::Detail, size);
        h.app.run("panel.masking", json!({})).unwrap();
        h.app.run("mask.add", json!({"kind": "linear"})).unwrap();
        h.app.run("mask.add", json!({"kind": "radial"})).unwrap();
        h.settle(SETTLE);
        let photo = h.app.session.active().expect("active photo");
        let masks = h.app.session.develop_of(photo).expect("develop settings").masks.clone();
        assert_eq!(h.app.session.active_mask, Some(masks[1].id));
        let tile = widget(&h, &format!("mask:{}", masks[0].id)).expect("first mask tile").center();
        h.input([finger(egui::TouchPhase::Start, tile, 78), egui::Event::PointerMoved(tile), button(tile, true)]);
        h.step();
        for _ in 0..90 {
            h.step();
        }
        assert_eq!(h.app.session.active_mask, Some(masks[0].id), "holding selects the mask to manage at {size:?}");
        let management = widget(&h, "button:maskAddComp").expect("management opens beside the held mask");
        assert!(management.intersects(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0], size[1]))));
        h.input([finger(egui::TouchPhase::End, tile, 78), button(tile, false)]);
        h.step();
        h.input([egui::Event::PointerGone]);
        h.step();
        assert_eq!(h.app.session.active_mask, Some(masks[0].id), "release must not select another mask");
        assert_eq!(h.app.session.develop_of(photo).expect("develop settings").masks.len(), 2);
    }
}

#[test]
fn scrolling_the_mask_rail_cancels_management_hold() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    h.app.run("panel.masking", json!({})).unwrap();
    h.app.run("mask.add", json!({"kind": "linear"})).unwrap();
    h.app.run("mask.add", json!({"kind": "radial"})).unwrap();
    h.step();
    let photo = h.app.session.active().expect("active photo");
    let masks = h.app.session.develop_of(photo).expect("develop settings").masks.clone();
    let tile = widget(&h, &format!("mask:{}", masks[0].id)).expect("first mask tile").center();
    let moved = tile + egui::vec2(0.0, 55.0);
    h.input([finger(egui::TouchPhase::Start, tile, 79), egui::Event::PointerMoved(tile), button(tile, true)]);
    h.step();
    h.input([finger(egui::TouchPhase::Move, moved, 79), egui::Event::PointerMoved(moved)]);
    h.step();
    for _ in 0..90 {
        h.step();
    }
    assert!(widget(&h, "button:maskAddComp").is_none(), "a rail scroll must not open management");
    h.input([finger(egui::TouchPhase::End, moved, 79), button(moved, false)]);
    h.step();
    h.input([egui::Event::PointerGone]);
    h.step();
    assert_eq!(h.app.session.active_mask, Some(masks[1].id), "scrolling does not select the touched mask");
    assert_eq!(h.app.session.develop_of(photo).expect("develop settings").masks.len(), 2);
}

#[test]
fn focused_mask_tile_opens_management_from_keyboard_without_a_hold() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    h.app.run("panel.masking", json!({})).unwrap();
    h.app.run("mask.add", json!({"kind": "linear"})).unwrap();
    h.step();
    let photo = h.app.session.active().expect("active photo");
    let mask = h.app.session.develop_of(photo).expect("develop settings").masks[0].id;
    h.view.ctx.memory_mut(|memory| memory.request_focus(egui::Id::new(("mobile-mask-tile", mask))));
    h.input([egui::Event::Key {
        key: egui::Key::F10,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers { shift: true, ..Default::default() },
    }]);
    h.step();
    assert!(widget(&h, "button:maskAddComp").is_some(), "keyboard users can manage the focused tile");
    assert!(widget(&h, "mobileMaskRail:manage").is_none());
}

#[test]
fn phone_remove_marks_target_and_source_for_touch_selection() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    h.app.run("panel.remove", json!({})).unwrap();
    for point in [[0.3, 0.4], [0.7, 0.6]] {
        h.app.run("spot.add", json!({"mode": "clone", "points": [point], "size": 0.025, "feather": 50.0, "opacity": 100.0})).unwrap();
    }
    h.app.run("spot.select", json!({"index": 0})).unwrap();
    h.settle(SETTLE);
    let target = widget(&h, "spotPin:0").expect("selected correction target");
    let source = widget(&h, "spotSource:0").expect("selected correction source");
    assert!(target.width() >= 44.0 && target.height() >= 44.0);
    assert!(source.width() >= 44.0 && source.height() >= 44.0);
    let other = widget(&h, "spotPin:1").expect("other correction target");
    assert!(other.width() >= 44.0 && other.height() >= 44.0);
    let result = h.request("ui.clickWidget", json!({"id": "spotPin:1"}), Duration::from_secs(20));
    assert_eq!(result["ok"], true, "{result}");
    h.step();
    assert_eq!(h.app.session.active_spot, Some(1));
    assert_eq!(h.app.session.develop_of(h.app.session.active().expect("photo")).expect("develop").spots.len(), 2);
}

#[test]
fn phone_remove_brush_shows_continuous_coverage_and_keeps_spot_commands() {
    let mut h = touch_app(ViewMode::Detail, [390.0, 844.0]);
    h.app.run("panel.remove", json!({})).unwrap();
    h.settle(SETTLE);
    assert!(widget(&h, "slider:ui.removeSize").is_some());
    assert!(widget(&h, "slider:ui.removeFeather").is_some());
    assert!(widget(&h, "slider:ui.removeOpacity").is_some());
    let r = h.request("ui.clickWidget", json!({"id":"button:removeMode-clone"}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    let photo = h.app.image_rect.expect("photo");
    let start = photo.center() + egui::vec2(-25.0, -30.0);
    let end = start + egui::vec2(44.0, 0.0);
    h.input([finger(egui::TouchPhase::Start, start, 12), egui::Event::PointerMoved(start), button(start, true)]);
    h.step();
    h.input([finger(egui::TouchPhase::Move, end, 12), egui::Event::PointerMoved(end)]);
    h.step();
    let stroke = h.app.brush_live.as_ref().expect("phone Remove previews continuous brush coverage while painting");
    let ([w, height], _) = stroke.size();
    assert!(w > 0 && height > 0);
    h.input([finger(egui::TouchPhase::End, end, 12), button(end, false)]);
    h.step();
    h.step();
    assert!(h.app.brush_live.is_none(), "preview is cleared after committing the correction");
    let id = h.app.session.active().expect("active photo");
    let develop = h.app.session.develop_of(id).expect("develop settings");
    assert_eq!(develop.spots.len(), 1);
    assert_eq!(develop.spots[0].mode, lightcraft_develop::SpotMode::Clone);
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

#[test]
fn small_phone_color_mixer_bands_are_finger_sized_and_selectable() {
    let mut h = touch_app(ViewMode::Detail, [360.0, 740.0]);
    h.app.ui.right = crate::state::RightPanel::Edit;
    h.app.ui.toggle_flyout("mixer");
    h.settle(SETTLE);
    let click = h.request("ui.clickWidget", json!({"id": "mobileEdit:color"}), SETTLE);
    assert_eq!(click["ok"], true, "{click}");
    h.step();
    let bands = lightcraft_develop::MIXER_BANDS;
    for band in bands {
        let rect = widget(&h, &format!("mixerBand:{band}")).expect("mixer color dot");
        assert!(rect.width() >= 36.0 && rect.height() >= 44.0, "{band} is finger-sized: {rect:?}");
        assert!(rect.right() <= 360.0, "{band} stays on screen: {rect:?}");
    }
    let target = bands[2];
    let sheet = widget(&h, "panel:sheet").expect("sheet");
    h.input([egui::Event::PointerMoved(sheet.center())]);
    h.step();
    let scroll = h.request("ui.scroll", json!({"dy": -300.0}), SETTLE);
    assert_eq!(scroll["ok"], true, "{scroll}");
    h.step();
    h.settle(SETTLE);
    let band_rect = widget(&h, &format!("mixerBand:{target}")).expect("band");
    assert!(band_rect.top() >= widget(&h, "mobileEdit:light").unwrap().bottom(), "band must be below the pinned navigation");
    assert!(band_rect.bottom() <= sheet.bottom(), "band must be visible for touch: {band_rect:?} in {sheet:?}");
    let r = h.request("ui.clickWidget", json!({"id": format!("mixerBand:{target}")}), SETTLE);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert_eq!(h.app.ui.mixer_mode, target);
}
