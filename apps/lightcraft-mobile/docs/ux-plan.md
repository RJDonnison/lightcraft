# Mobile UX plan: library and a simpler phone editor

**Status:** MUX-0–MUX-9 implemented; MUX-10 integration checks complete where hardware is available. Android on-device gallery, permissions and publication verification remains open. **Scope:** Android first, with shared compact-touch presentation where appropriate. Desktop behaviour must not change.

Read this plan with the repository's `AGENTS.md` and `apps/lightcraft-mobile/README.md`. The work slices describe the implementation and its remaining on-device verification.

## Goal and approach

People should be able to browse photos on their phone without importing them first, find their LightCraft edits separately, and edit one-handed without confronting every tool at once. Keep the existing Rust engine, command architecture and egui UI. Use observed mobile-editor layout and interaction ideas as inspiration only: do not copy Adobe icons, images, screenshots, presets, fonts or wording.

Keep about five primary tools within thumb reach, put the remaining tools in one expandable view, and arrange adjustments from common to advanced. Use thumbnails generated from the person's photos and LightCraft's original code-drawn icons where they help identify actions.

Non-goals: a native SwiftUI/Compose rewrite, new develop-engine features for their own sake, desktop-panel changes, or new decorative assets.

## Product decisions

- **Library:** two tabs, **Device Photos** and **My Edits**. Device Photos browses accessible Android gallery media without copying it. My Edits lists catalog photos with saved develop changes (`Photo::is_edited()` is the existing predicate). Opening a device photo to edit imports it once, directly, without a separate Import Photos click. Keep albums, trash and explicit import accessible where useful; do not confuse album membership with catalog ownership.
- **Device access:** request Android photo access to browse the whole gallery, while respecting selected-photos-only grants and permission revocation. The existing system picker remains available where appropriate; browsing must not silently copy the whole library.
- **Primary tools:** Presets, Edit, Crop, Remove and Masking, plus **More**. Fit the controls at 360 pt without a scrolling primary row. More expands a labeled grid containing the other existing tools.
- **Edit order:** Light, Color, Effects, Detail, Optics, Calibration. Curve belongs with Light and Color Grading with Color, with direct one-tap entries for both. Retain section values and undo history when switching.
- **Masking:** a narrow, scrollable mask-selection rail beside the photo; only the selected mask's components and adjustments occupy the bottom sheet. Make creation, visibility and overlay controls reachable without obscuring the image.
- **Sheet:** two touch-snap heights, with the photo remaining visible. Tapping the active tool or a suitable area outside dismisses the sheet; image pan/zoom, crop, mask drawing and Curve gestures must not dismiss it accidentally.
- **Sliders:** compact-touch value labels do not open numeric entry. Double-tap the left or right edge to decrease or increase by one configured `ControlSpec.step`; double-tap the middle to reset. Preserve relative sideways adjustment and vertical panel scrolling. Desktop typing and mouse/keyboard input stay as they are.
- **Export:** a small selector for formats the engine actually supports. Publish gallery-compatible rendered images to Photos. Save formats the Android gallery cannot reliably display, and any required sidecars, to Downloads; report the destination and failures. Keep the desktop export form for desktop use.
- **Platforms:** Android native gallery browsing and saving first. Shared compact-touch improvements can benefit iOS and phone-sized web windows, but do not claim equivalent native gallery integration there.

## Verified starting point

- `crates/ui-egui/src/panels/compact.rs` already draws five labeled touch-editor tools and a half-height sheet, but the primary bar scrolls on narrow screens, the active tool does not close it, and a duplicate title/Hide header occupies the top of the touch sheet. Non-touch compact windows still have a separate, horizontally scrolling ten-tool row.
- `crates/ui-egui/src/panels/edit.rs` already has touch-focused Light, Curve, Color, Color Grading, Effects, Detail, Optics and Calibration categories. Color Mixer already has eight selectable bands and selected-band Hue/Saturation/Luminance controls; refine these rather than adding a second mixer.
- `crates/ui-egui/src/panels/masking.rs` places creation, masks, overlays, components and local adjustments in one scrollable bottom panel. `panels/detail.rs` owns the photo canvas and mask overlay; mask selection already uses `mask.select`.
- `crates/ui-egui/src/widgets.rs` already arbitrates touch slider drags against vertical scrolling and ignores touch taps on the track. Clicking the value currently starts exact-number entry; double-click on the track or label resets. Existing `nudged` logic uses the control's configured step.
- `panels/presets.rs` already has optional 128-pixel live photo-variant thumbnails; `panels/profiles.rs` already shows live thumbnails. Presets default to short text rows and some previews rely on hover.
- The Android shell currently opens a system picker and copies chosen content into `<library>/Picked` before import review. Its manifest lacks photo-read permissions; it has no MediaStore gallery browser. `LibrarySource` covers catalog and filesystem sources, not Android gallery records. Android exports currently land in private storage and open a share chooser, while the phone's Export button opens the full desktop dialog.
- The mobile README describes compact layout at widths below 600 pt, or short phone-landscape windows. The repository is alpha; an existing checklist feature does not establish mobile UX parity.

## Work slices

Use a single project task ID per commit (replace the MUX placeholders with the project's current task scheme). For each user-visible slice, add a test that would fail before the change, inspect before/after phone output, and pass `cargo xtask ci` before committing. Preserve existing unrelated work.

| Slice | Change | Starting points | Done when |
| --- | --- | --- | --- |
| **MUX-0 — Touch baseline** | Add a reproducible demo tour of the grid, editor, Masking, Curve, Crop, Presets and export. Capture 390×844, 844×390, 360×740 and 600×900. Ensure the capture session actually announces touch; a narrow window alone is not proof. Keep PNGs in scratch space, with only the script tracked. | `apps/lightcraft-mobile/scripts/`, `docs/control-protocol.md`, headless UI fixtures | Captures represent the touch UI and can be reproduced without committing media. |
| **MUX-1 — Device Photos and My Edits** | Query accessible Android MediaStore images with paging, stable identities and thumbnails; handle full, selected-only, denied and revoked access. Add the two tabs and an edited-only catalog view. When a device photo is opened for editing, copy/import it once, associate it with its device source and open Detail without requiring import review. Resolve how to persist that association within the catalog before adding a new field; avoid duplicate imports and unbounded media scans. | `apps/lightcraft-mobile/android/AndroidManifest.xml`, `LightCraftActivity.java`, `src/android_jni.rs`, `src/lib.rs`, catalog/engine source model, `panels/left.rs`, `panels/grid.rs` | Browsing needs no import; opening/reopening does not duplicate a photo; edits remain available if gallery access is revoked or the device item disappears. |
| **MUX-2 — Primary tools and overflow** | Fit five primary tools plus More at 360 pt without horizontal scrolling; ensure touch regions stay comfortable. More expands a labeled grid for Red Eye, Info, Keywords, Versions, History and any other existing secondary tools, then collapses on selection. Preserve panel commands and shortcuts. | `panels/compact.rs`, `state.rs` | Every tool is reachable, with no scrolling or overlap in the primary bar at 360 pt. |
| **MUX-3 — Sheet and headings** | Remove the duplicate touch sheet title/Hide header and repeated in-panel headings. Add two snap heights and gesture-safe active-tool/outside dismissal. Keep the photo visible and preserve portrait and landscape placement. | `panels/compact.rs`, `panels/right.rs`, affected panel headings | A second tap closes the active tool; outside taps close only when safe; the expanded height makes large tools usable. |
| **MUX-4 — Edit navigation** | Refine the existing category chooser into the chosen order, keeping direct Curve/Grading access without duplicating adjustments. Preserve section state, slider values and undo when switching. | `panels/edit.rs`, `panels/right.rs` | Only the focused group is in view on touch; desktop Edit remains unchanged. |
| **MUX-5 — Photo-side masking rail** | Move mask thumbnails/selection, visibility and the add-mask entry into a narrow image-side rail. Keep the selected mask's components and Light/Color/Effects/Detail adjustments in the sheet. Retain overlays, analysis and model-download progress. Account for image gestures and landscape space. | `panels/detail.rs`, `panels/masking.rs`, `panels/compact.rs` | Rail and sheet remain usable together; selecting a mask changes the relevant controls without covering essential image interactions. |
| **MUX-6 — Sliders and Curve** | Remove compact-touch value-field typing and implement edge double-tap step nudges with middle reset. Give Curve points and channel controls larger hit regions, distinguish curve editing from panel scrolling, and fit the graph to the sheet width and expanded height. Preserve deliberate add/move/remove-point operations. | `widgets.rs`, `panels/edit.rs` | Touch tests distinguish nudge, reset, drag, scroll and curve point operations; desktop exact entry is unchanged. |
| **MUX-7 — Presets and Profiles by touch** | Default phone Presets to active-photo thumbnail previews using the existing variant renderer. Make selection, apply and undo clear without hover; bound rendering work when scrolling or switching photos. Review Profile thumbnail and favorite touch targets. | `panels/presets.rs`, `panels/profiles.rs` | Previews follow the selected photo; touch-only application and undo work without stalling scrolling. |
| **MUX-8 — Color Mixer and touch audit** | Keep the existing eight bands and Hue/Saturation/Luminance sliders, but enlarge band hit regions and selected state. Audit other small phone buttons and make useful actions image-first where appropriate. Original code-drawn icons only. | `panels/edit.rs`, `widgets.rs`, phone panels, `icons.rs` | Controls are hittable at 360 pt; selection is unambiguous; no Adobe assets enter the repository. |
| **MUX-9 — Mobile export** | Replace the primary phone export form with a format selector and one Export action. Reuse `app.export` and its progress/cancellation path; publish suitable files through Android MediaStore and send unsupported gallery formats/sidecars to Downloads. Keep other share/export commands available for their own purposes. | `panels/compact.rs`, export UI/task path, mobile `src/lib.rs`, `src/android_jni.rs`, `LightCraftActivity.java` | Every offered format works, files appear at their reported destinations, and a failed or partial write never reports success. |
| **MUX-10 — Integration and docs** | Review all flows in portrait, landscape, small Android and at the 600-pt breakpoint; check the desktop snapshot too. Update the mobile README and relevant parity rows, gap tracker and progress numbers only when the associated behavior actually lands. | mobile tests/scripts, README, applicable parity docs | Android device and screenshot checks pass, with `cargo xtask parity` and `cargo xtask ci` green. |

MUX-1 establishes the device/catalog boundary before cross-source thumbnails and export integration. MUX-2 precedes MUX-3; MUX-3's snap heights should precede final Curve and masking layout tuning. MUX-4, MUX-7 and the audit part of MUX-8 can be developed independently once the baseline exists; merge one branch and rerun CI before the next. Do not delegate or create parallel worktrees unless the task owner explicitly requests parallel agents.

### Implementation and verification record

- **MUX-0–MUX-9:** implemented in commits `0ac6083c`, `b82278df`, `36ca63db`, `d5b59423`, `aa5179f0`, `319597cb`, `d67b6cf0`, `e8a31524`, `ae628cf6`, and `b10d0004` (slice order). Every slice passed `cargo xtask ci` before its commit.
- **MUX-10:** the tracked `ux-tour.jsonl` completes with `snapshot --demo --touch` at 390×844, 360×740 and 844×390. At 600×900 the desktop layout appears; a separate snapshot confirms that boundary. Screenshots are inspected in scratch space, never tracked. Android-target Rust and Java compilation pass, and the gallery and export host boundaries have headless UI tests.
- **Open hardware gate:** no Android device or emulator was attached. On-device verification of full/selected/denied/revoked MediaStore access, first-edit import, exports appearing in Photos/Downloads, Android 8/9 public-folder writes, system media scanning, and gesture ergonomics remains necessary before claiming the mobile app is release-ready. The source-built mobile shell is not packaged or released.

## Verification and guardrails

- **Look at it:** use a touch-enabled headless script or a running app driven through its control channel; capture and inspect before/after 390×844, 844×390, 360×740 and 600×900 output. Include an Android device pass for permissions, device-gallery changes, gallery export and touch gestures. A registered offscreen widget does not count as usable.
- **Tests:** add focused catalog, bridge, touch-gesture, mask-rail, preset and export regressions as each slice lands. Test permission denial/revocation, missing originals, duplicate opening, format-specific destinations and disk/write errors. Run `cargo xtask ci` before every commit; `cargo xtask parity` checks the tracker references.
- **Touch:** enlarge small hit regions rather than preserving the old 30-pt controls and 40-pt compact buttons simply because they already exist. Preserve accessibility labels, desktop mouse/keyboard paths, horizontal-slider drag and vertical-panel scroll. Never use OS-level synthetic clicks or keys to drive the app.
- **Clean room:** imitate behaviour and layout only. No Adobe asset, screenshot, preset, font, profile, icon or copied wording in the repository. Reference screenshots belong only in gitignored `plan/`; generated test captures stay in scratch space. An asset added to the repository needs an `assets/ATTRIBUTION.md` entry and its required licence text.
- **Reliability and layering:** malformed media, permission changes and full disks must produce actionable errors, not panics. No `unwrap`, `expect`, `panic!` or unapproved `unsafe` in non-test code. Route actions through existing or new commands; no egui dependencies below L5. Gate mobile presentation on compact touch; verify the desktop UI remains unchanged.

## Slice handoff template

```text
Read AGENTS.md and apps/lightcraft-mobile/docs/ux-plan.md.
Implement slice <MUX-n> only, keeping its dependencies and desktop-isolation guardrails.
Before editing, capture and inspect touch-enabled baseline views at 390x844 and 844x390.
Make the change; add a regression test that would have failed beforehand.
Capture and inspect the result at all relevant sizes, including 360x740 and 600x900.
Run cargo xtask ci before a commit; update relevant docs and tracker rows when the behavior lands.
Use one task id per commit and do not add Adobe assets or copied wording.
```
