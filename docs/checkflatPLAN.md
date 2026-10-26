# Checkflat — Project Plan

Offline-first app for real-estate site visits: load PDF plans, drop numbered pins, attach photo + description, export a PDF report. Scope reference: a lightweight Aproplan / Fieldwire "punch list" workflow for a single user.

## 0. Client answers (2026-09-24)
- Devices: Android phone + Android tablet; Windows laptop for office work.
- Internet on site: yes (app still works offline; no dependency on network).
- 10–50 observations per visit.
- Follow-up visits: yes — open items carry over, get marked resolved.
- Plans: A3 and A1, one plan per PDF file.
- No status/category/contractor/priority fields (beyond open/resolved needed for follow-ups).
- Photo annotation (circles/arrows): yes.
- Ref numbering: continuous across the project (to confirm — answer was ambiguous).
- Report layout: as in the Aproplan sample.
- Languages: English + Portuguese (UI and report).
- Report recipients: contractor + final client (one report per visit).
- Single user.
- Must move data from phone to computer.

### Client answers (2026-10-01) — these override the lines above where they differ
- **Pins:** no tap-to-place. A "+" button enters an add-pin mode; the next tap places a draft pin and leaves the mode; Confirm/Cancel as before (ref only on confirm). Outside the mode a tap on the empty plan does nothing; a tap on a pin selects it. Android back leaves the mode. (D-018)
- **Ref numbering:** not one continuous number. Per-project configurable ref format built from project code + fraction + sequence. **Exact format still pending client confirmation** — it only changes the per-project defaults. Applied in Sprint 3a (migration 2, D-020): `project.code`; `observation.fraction` + `observation.seq`; per-project template with tokens `{PROJ}` `{FRAC}` `{SEQ:n}` (default `{PROJ}-{FRAC}-{SEQ:2}`); sequence scope per fraction or per project (default per project, locked once a number is issued); the displayed ref is computed from the template; fractions come from a per-project list (typing a new one adds it); common areas use a code such as "PC". This replaced `project.next_ref_no` and `UNIQUE(project_id, ref_no)`.
- **Report:** open observations newest first; a separate section with **all** closed observations since the project start, **without photos**; the cover summary lists the observations closed in this visit. Attendees optional (shown only if filled). **No logo.** Typical length 20–50 pages.
- **Plan revisions:** a new revision replaces the plan in place, pins carry over, no history kept. Backlog, after v0.2.
- **Devices:** Xiaomi 15 (Android 15, HyperOS) + Windows laptop. **No tablet for now.** Acceptance stays on the P30 Pro with scaled time targets (D-014). HyperOS kills background apps aggressively → tile generation must stay resumable.

### Client answers (2026-10-13) — report layout (applied in Sprint 5b, D-025)
1. **No page per observation:** a continuous list, several observations per page, with a clear separation between them. An observation's header + ID + plan row (crop and overview) stay together; the description and each photo with its caption are never split; photos may flow to the next page.
2. **Photos one below the other** (single column), not a two-column grid. *Replaced on 2026-10-26 by Martín's decision: two per row, for a more compact report (D-029).*
3. **Max 3 photos per observation**, enforced in the app (core validation on create and edit; the sheet stops offering the camera/picker at 3).
4. **Pin in the plan crop shows the marker and the observation number:** the same short label as the app pin (`3`, or `PC-01` per fraction).
5. **Header:** the plan title next to the project name.
6. Unchanged and confirmed: captions `n. dd.mm.yyyy hh:mm`, newest first by creation date, cover contents, crop ≈ 20 % of the sheet.
- **Still open:** whether the crop needs more context than ≈ 20 % (the reply was only "sim", ambiguous); the Aproplan sample pages (not sent yet); the printed sample for the Sprint 6 size options (D-024) — Martín printed the two sample PDFs (D-029) and chose the smaller images ("no visible difference in print"); the client confirms in the preview.

## 1. Concept
- **Project** (a building/site) → contains **Plans** (one PDF each) and **Visits** (dated sessions).
- During a **Visit**, the user opens a plan, taps a spot → creates an **Observation**: auto ref number, pin, photo(s) with annotation, description.
- **Follow-up visit**: starts with all still-open observations from previous visits; user marks each resolved or keeps it open, and adds new ones.
- End of visit → **Report** PDF: cover page + one block per observation in a continuous list (ref, zoomed plan crop with the labelled pin, overview plan with red box, description, photos with timestamp, open/resolved).

## 2. Requirements

### Must (MVP)
- M1 Create/rename/delete projects.
- M2 Import PDF plan (one page per file, A3/A1); plan title.
- M3 Smooth pan/zoom on A1 plans on Android phone and tablet.
- M4 Add-pin mode ("+", then tap) places a pin; drag to adjust; stored as normalised x/y (0–1).
- M5 Observation: ref (per-project format: project code + fraction + sequence, §0 2026-10-01, D-020; exact format pending — a settings default), description, 1–3 photos (camera or gallery; §0 2026-10-13, D-025), timestamp.
- M6 Photo annotation: ellipse, arrow, freehand; red; saved non-destructively (original kept).
- M7 Observation list; tap to jump to pin.
- M8 Visits + follow-ups: carry over open observations; mark resolved (with date).
- M9 PDF report per visit in EN or PT; share (Android) / save (Windows).
- M10 Export project as one archive file (.zip) and import it on another device (phone → laptop).
- M11 Responsive UI: phone, Windows (touch + mouse/keys). Tablet layout: backlog (no tablet for now).
- M12 Works fully offline.

### Should
- S1 Report header: project name, address, visit date, attendees (only if filled). No logo.
- S2 Summary table at start of report (ref, plan, open/resolved).
- S3 Filter report: new only / open only / all.

### Could (later)
- C1 Automatic sync phone ↔ laptop (e.g. via a cloud folder or small server) — replaces manual archive transfer.
- C2 Voice-to-text descriptions.
- C3 iOS build.
- C4 Multi-user.

### Won't (for now)
- Categories, contractors, priorities, user accounts, BIM, real-time collaboration.

## 3. Architecture
| Layer | Choice | Why |
|---|---|---|
| Shell | **Tauri v2** (Rust core + system WebView) | One codebase → Android + Windows |
| UI | Svelte + TypeScript | Light, good touch handling |
| PDF viewing | **PDF.js** in the WebView renders an import-time **tile pyramid** once per plan (512 px WebP, levels 1024–8192 px); the viewer shows image tiles only (D-014) | No native PDFium bundling; an open PDF.js document of the A1 plan costs ~780 MB, the tile viewer ~220 MB total |
| Plan snapshots | Rust stitches the crop and the overview from the tiles (`image` crate) → PNG; crops always greyscale with 16 levels, the overview in colour when it has any (D-029); the pin and the red box are vector overlays in the Typst template (D-024) | No WebView or PDF.js at report time; same on Windows (D-014); the overview is embedded once per plan |
| Photo annotation | Full-screen editor with an **SVG overlay** (ellipse, arrow, freehand; red; one stroke width as a fraction of the long side); shapes stored as JSON in `photo.annotation_json` with 0..1 coordinates, validated in the core; drawn onto the 800 px report copy by `annotate::flatten` with `tiny-skia` (already in the tree via typst-pdf) (D-027) | Non-destructive edits; the photo file never changes; same look in the app and the PDF |
| Storage | SQLite (`rusqlite`) + files in app data dir | Offline, portable |
| Images | Rust `image` crate: EXIF orientation applied, resize to ≤ 1600 px JPEG q85, EXIF date kept as `taken_at`, stored without EXIF (D-022) | Small reports |
| Report | Rust: embedded **Typst** through `typst-as-lib` (D-004, closed in D-024); photos downscaled to 640 px / JPEG q75 before embedding (D-029); timestamps converted per instant with the device's IANA zone (`jiff`, system tzdata on Android); the latest report per project kept with its metadata (`reports/<project>.json`) and a readable name `<project> - <yyyy-mm-dd> - <EN|PT>.pdf` (D-028) | Templated layout, EN/PT with hyphenation; bounded memory; DST-safe captions |
| Transfer | Project archive: `.zip` with SQLite export + files | Simple phone → laptop move |
| Camera / gallery / report hand-off | Tauri mobile plugin (Kotlin): `ACTION_IMAGE_CAPTURE` into its own FileProvider, system Photo Picker, HEIC → JPEG conversion; report Open (`ACTION_VIEW`) and Share (ShareCompat) of a copy under the cache's `reports/`, the installed version; its FileProvider is the app's only one (cache `captures/` + `reports/`, a folder per report, checked in CI); Windows: file picker, report Open with the default PDF app and "Save a copy…" through a save dialog opened from Rust (D-029) | `<input capture>` opens the Photo Picker in the Android WebView (D-003); the plugin is the Android bridge (D-022, D-028) |
| i18n | Shared EN/PT string files for UI + report | One source of truth |
| CI | GitHub Actions: Android APK + Windows installer | Dev machine is macOS |

### Data model (v2, migration 2 — D-020)
```
project(id, name, address, code, ref_template, seq_scope 'project'|'fraction', next_seq, created_at, updated_at)
fraction(id, project_id, code NOCASE, next_seq, created_at)            UNIQUE(project_id, code)
plan(id, project_id, file_path, title, width_pt, height_pt, created_at)
visit(id, project_id, date, attendees, notes, report_lang, created_at)
observation(id, project_id, plan_id, fraction, seq, seq_key, x_norm, y_norm, description,
            created_visit_id, resolved_visit_id NULL, resolved_at NULL, created_at, updated_at)
                                                                        UNIQUE(project_id, seq_key, seq)
photo(id, observation_id, visit_id, file_path, taken_at, annotation_json, created_at)
```
Note: observations belong to the project (not one visit) so they can carry over; photos link to the visit they were taken in. The displayed ref is computed from `project.ref_template` + `code` + `fraction` + `seq`, never stored; `seq_key` is the fraction code when numbers run per fraction and `''` when per project (D-020). v1 (`logo_path`, `next_ref_no`, `ref_no`, `UNIQUE(project_id, ref_no)`) was migrated in Sprint 3a; `assign_ref` is still the only place a number is taken.

## 4. Milestones & sprints (1 sprint ≈ 1 week part-time)

### M0 — Foundations & risk spikes
- **Sprint 0**: Tauri v2 scaffold; Android emulator + Windows CI green. Spikes: (a) PDF.js zoom on a real A1 plan on Android; (b) camera capture; (c) Typst vs printpdf 1-page report with image. Log in `docs/decisions.md`.
- *Exit*: APK opens a plan, takes a photo, outputs a PDF.

### M1 — Plans & pins
- **Sprint 1** ✅ (2026-09-25): SQLite schema + migrations; projects CRUD; plan import; EN/PT i18n setup. Decisions D-008…D-013.
- **Sprint 2** ✅ (2026-09-26; client feedback applied 2026-10-01): plan viewer (pan/zoom); add-pin mode + drag pins; phone layout + Windows mouse/keys. Decisions D-014…D-018.
  - Step 1 — viewer spike (≤ 1 day, after D-006): `VITE_SPIKES` flag so a release APK shows the Dev screen; PDF.js stage profile (document / operator list + image decode / rasterise, legacy vs modern build, A1 vs A4); on the P30 Pro compare (1) D-002 revised (1024 px quick pass → 3072 px base + viewport tile) vs (2) import-time tile pyramid (512 px tiles, 4 levels, WebP on disk, viewer shows images only; incl. generation time, disk size, peak PSS while generating, backgrounding). Targets for the client's 2024–25 phone: first view ≤ 4 s, total PSS (app + WebView renderer) ≤ 300 MB at 8×, smooth pan; on the P30 Pro time targets are scaled ×2 (≤ 8 s), memory is not. Result → D-014; then a checkpoint before the rest of the sprint. Release APKs and `dist/` from the spike are deleted afterwards (client plan bundled).
  - Then: viewer with the chosen design (**D-014: tile pyramid**, generated in the foreground after import, resumable per level; max zoom capped at ≤ 1.5× upscaling of the top level); tap places a **draft** pin (no ref_no) with Confirm/Cancel — ref_no is taken from `next_ref_no` only on confirm (Sprint 3a: observation sheet save), so cancelled drafts leave no gaps; drag to adjust; stored as `x_norm`/`y_norm`; Android back handling; responsive layout (tablet/desktop side panel moves to Sprint 4 if the sprint runs long); asset protocol scoped to `projects/**` (resolves the D-007 item); address field save feedback.
  - Client feedback 2026-10-01 (D-018): tap-to-place replaced by the **add-pin mode** ("+" → next tap places the draft and leaves the mode; a tap outside the mode does nothing; Back leaves the mode); ref assignment isolated in one core function (`assign_ref`, placeholder on `next_ref_no` until the Sprint 3a migration, D-020); the **tablet/desktop side panel is out of Sprint 2 scope → backlog** (no tablet for now).
- *Exit*: pins persist after restart, on the phone (and Windows).

### M2 — Observations & photos
- **Sprint 3a** ✅ (2026-10-05): **ref format migration (D-020)**, without waiting for the client's exact format (every open answer is a project setting): `project.code`, per-project template (`{PROJ}` `{FRAC}` `{SEQ:n}`), sequence scope per fraction or per project, per-project fraction list, `observation.fraction` + `observation.seq` + `seq_key`; `project.next_ref_no`, `logo_path` and `UNIQUE(project_id, ref_no)` dropped; `assign_ref` swapped. Observation sheet (fraction type-to-add, description) replaces the Sprint 2 Confirm; project settings screen (code, template with live preview, scope, fractions).
- **Sprint 3b** ✅ (2026-10-08): **fixed release signing** (D-021: CI ships a signed arm64 release APK, versionCode = run number, so builds install over each other); photos on the observation sheet (camera or system Photo Picker, Windows file picker — its manual exit test moves to before v0.1, with the Sprint 6 super review; ≥ 1 photo on create and edit; edit mode for description + photos; Rust resize to ≤ 1600 px JPEG with EXIF orientation and date; HEIC converted on Android; no migration 3) and the D-007 camera follow-ups (D-022).
  - Camera plugin follow-ups deferred from the Sprint 0 code review (see decisions D-007): (a) ✅ persist the pending capture path so a photo survives the app being killed behind the camera (the open sheet is stored too and restored); (b) ✅ self-contained FileProvider (own subclass, `file_paths.xml` and `<provider>` in the plugin manifest) instead of relying on the Tauri app template; (c) ~~decide `assetProtocol` vs base64 for showing photos~~ resolved in Sprint 2: asset protocol, scope `projects/**` (D-015).
  - Backlog from Sprint 3a: editing the fraction/description of an existing observation (changing the fraction must keep `seq_key` consistent); the pin list panel could show the description.
- **Sprint 4** (runs **after Sprint 5**, D-023; plan `docs/sprint-4-plan.md`, decision D-027): photo annotation (ellipse, arrow, freehand) — fills `annotate::flatten` so annotations reach the report; observation list + jump-to-pin; **pins move only in a move mode** (select the pin, tap Move — like the add-pin mode, D-018), because on a busy plan a touch meant to pan or select drags a pin (P30 check, D-026). Two parts: **4a** ✅ code (2026-10-20, PR `sprint-4a`): full-screen SVG editor from the sheet's thumbnails (tap selects, Delete, Undo; Back/Escape cancel), format v1 validated in the core, saved with the sheet and kept in the draft, drawn on the 800 px report copy with `tiny-skia`, seed option "with annotations"; P30 check and re-measure recorded in D-027. **4b** ✅ (2026-10-21, PR `sprint-4b`): observation list (whole project, newest first, thumbnail with the shapes, fraction filter; a tap opens the plan on the pin and Back returns to the list with its filter and scroll position), **pins move only in move mode** (select → Move → drag or tap → Confirm; Back cancels), camera-path logging, the double-log fix, icon toolbar in the editor and icon pin bar so both fit one row at 360 px; the list's thumbnail memory criterion (≤ 300 MB on the P30) failed with the full-size photos and passes with a cache of 224 px derived thumbnails (peak 522.8 → 227.9 MB, same driver; D-027); the touch check of the CI build passed (2026-10-21).
  - P30 check before this sprint ✅ (2026-10-14 … 10-20, D-026): D-022 regression and the 3-photo limit pass on CI build 45; the report on the P30 in 20.5–22.7 s, captions correct (jiff on EMUI's tz data).
  - **End of 4a (D-026): re-measure the report peak on the P30** — `dev-seed` build, plan viewed first, 50 × 3 with annotations on the seeded photos (D-027: the shapes are drawn on the 800 px copy, so no decoded image is added at the peak — the measurement confirms it); report-generation limit ≈ 650 MB.
  - Backlog from the P30 check (D-026): ~~log the camera path like `CheckflatPick`~~ ✅ 4b (`adb logcat -s CheckflatPick` + `dev.log` timeline; the slow "Preparing photo…" itself was not reproduced); ~~the dev log records each photo state twice~~ ✅ 4b (not a doubled listener: two emitters said "picking" and two said "preparing", now reported once).
  - Backlog from Sprint 2: pins overlap at fit when many are close together (smaller markers or clustering at low zoom); the import dialog still says "Choose a PDF…" for the ~3 s the picker result takes on EMUI (show "Reading PDF…" as soon as the picker closes). Tile cache: one fsync pass per level in `write_manifest` instead of one per tile (D-019: ≈ +10 s on the A1 plan's one-off preparation on the P30).
- *Exit*: full field workflow offline.

### M3 — Report (first usable)
- **Sprint 5** ✅ (2026-10-12, **before Sprint 4**: no Android phone for a few days, D-023; results in D-024): plan crop + overview snapshots; report template (cover + blocks) in EN/PT.
  - Result: 50 observations × 3 photos → 105 pages, 3.4 MB on the A4 fixture (the client's A1 plan: 17.2 MB, 14.0 MB with greyscale crops — D-024 size section); Mac release harness 3.9 s, max RSS 390 MB; Android 15 emulator (release APK) 15.2 s, peak total PSS 534 MB (limit ≈ 600 MB, D-014); APK 38.4 MB (240 KB under main); the image/codec crates now build at `opt-level = 3` (5× faster photo/snapshot preparation). Memory contingency (chunked compile + lopdf merge) designed, not needed. **Pending on hardware:** ~~time/memory on the P30; `jiff` on a vendor ROM's tzdata~~ — done in D-026 (P30, A1 plan, 50 × 3 real photos: 20.5–22.7 s against a ≤ 60 s P30 target; peak total PSS 547 MB from a cold start, 604–635 MB after viewing the plan — **the report-generation limit is raised to ≈ 650 MB**, smaller report images planned for Sprint 6, re-measure at the end of Sprint 4; captions correct on EMUI); the client's phone still pending.
  - Client 2026-10-01: open observations newest first; no logo; attendees only if filled; 20–50 pages typical. Reference block: the client's `mockup.jpg` (header with project name and ID, crop with pin beside the overview with a red box, description, photos captioned "1. dd.mm.yyyy hh:mm", page x/y).
  - Scope: (1) snapshots in Rust from the tile pyramid — square crop of ~20 % of the sheet's long side around the pin (clamped inside the plan), overview from the 1024 level, both PNG; pin and box drawn as vectors by Typst; (2) Typst report: provisional cover (name, address, date; no logo) + one block per open observation, newest first, whole project (visits in Sprint 7), EN/PT strings from `i18n/*.json`, `lang: "pt"` hyphenation, language chosen at generation time (default: UI language); (3) annotation hook `annotate::flatten` (identity until Sprint 4) with a test — every photo passes through it, then is downscaled to 800 px for the PDF; (4) D-004 follow-up closed (D-024: `typst` itself pulls `typst-html`/`typst-svg`, so `typst-as-lib` stays; APK size logged); (5) temporary "Generate report" action writing `reports/<project>-<stamp>.pdf`; render time and peak memory at 50 observations × 3 photos on the Mac and the emulator (limit ≈ 600 MB total, as tile generation; chunked compile + merge if exceeded). The Sprint 0 report spike leaves the app (crate kept).
  - Open for the client (relayed offline): ~~one observation per page; photo grid for several photos; pin label; cover contents; plan title in the block header; date format; "newest first" by creation time~~ — answered 2026-10-13 (§0, Sprint 5b, D-025). **Still open:** crop size/context (the reply was only "sim"); the Aproplan sample pages (not sent); fraction in the block header; placeholder when a plan has no tile cache on this device.
- **Sprint 5b** ✅ (2026-10-13, D-025): the client's layout answers applied — continuous list (head + ID + plan row kept together, description and each captioned photo unsplit, photos one below the other), the app's marker label on the crop's pin (kept inside the crop at the edges), plan title in the header; 1–3 photos per observation enforced in the core and the sheet (`photo_limit`). Emulator, 51 × 3 on the A1 plan: **103 → 69 pages**, 14,682,417 → 14,665,723 B, 105 s in the debug build. Photo sizes unchanged (our default); one observation per page is an option only if the client finds the report too long.
  - Phone-only later: ~~time/memory on the P30; `jiff` on the real device's tzdata~~ (done, D-026); time/memory on the client's phone; partial pyramid after a HyperOS kill; ~~share/open on the phone (Sprint 6)~~ (done, D-028: P30, CI build 65).
  - Backlog from the Sprint 5 code review (PR #4): (a) ~~parallel photo/snapshot preparation~~ — **not needed: the P30 meets its ≤ 60 s target with 20.5–22.7 s (D-026)**; was: (`std::thread::scope` over the photos in `prepare`) **only if the P30 time target fails** — photos are ~55 % of the report time, but parallel decoding raises the peak memory D-024 has the least margin on; (b) the Dev → Seed screen's plan list can race across quick project switches (`SeedScreen.svelte`, `$effect` without cancellation; debug builds only).
- **Sprint 6** (plan `docs/sprint-6-plan.md`, decisions D-028, D-029), two parts: **6a** (PR `sprint-6a`, 2026-10-22): report Open/Share on Android (ACTION_VIEW, ShareCompat, a copy under the cache's `reports/`, "No app to open PDFs"), the report card (language at generation, indeterminate progress, the latest report with date, language, pages and size), readable file names, the About line with versionName + build number, one narrow FileProvider checked in CI; P30 check in D-028. **6b** ✅ (PR `sprint-6b`, 2026-10-26, D-029): Windows Open and "Save a copy…"; the smaller report images below (P30, 56 observations: 20.8 → 9.4 MB, peak ≈ 720 → ≈ 534 MB with a fast on-device sampler); photos two per row (Martín's decision, 2026-10-26; the emulator's 51-observation project 69 → 52 pages); the app switch during generation (H1–H3 hold on the emulator and the P30, no foreground service; a reloaded page follows the run; Back during a run sends the app to the background); version 0.1.0 with a build number on Windows; the core and file-locking tests in the Windows CI job. The end-of-Sprint-6 super review follows, set up by Martín. Original scope: share/save on Android and Windows; performance at 50 observations. Report size (D-024): crops are greyscale since the Sprint 5 review. **Required (not optional), done in 6b: smaller report images — 16-level greyscale crops and 640 px / q75 photos** (D-024, D-026, D-027, D-029), for memory (635 MB measured on the P30 after viewing the plan, 641.6 MB with annotated photos, true peak likely ≥ 650 MB at 1.5 s sampling; limit ≈ 650 MB) and size (16.8 MB for a realistic 50 × 3, 18.4 MB annotated, against email limits of 20–25 MB, some providers ≈ 20 MB). Final values pending the client's printed sample; the chunked compile stays the fallback.
  - **Before v0.1, together with the end-of-Sprint-6 super review:** the manual Windows exit test deferred from Sprint 3b (add a photo from a file, save, restart; D-022). The Windows CI job stays a required check meanwhile. **Moved to the preview (D-029):** no Windows machine on the team, so the friend runs the checklist in D-029 on his laptop after the merge; until then CI covers the file rules (the core tests and file-locking tests on `windows-latest`).
- *Exit*: report matches the Aproplan sample. **→ v0.1, field test with friend**

### M4 — Follow-ups & transfer (MVP complete)
- **Sprint 7**: visits; follow-up visit carries over open items; mark resolved; resolved shown in report.
  - Client 2026-10-01: separate report section with all closed observations since the project start, without photos; the cover summary lists the observations closed in this visit.
- **Sprint 8**: project export/import archive (phone → laptop); report header + summary table.
- *Exit*: two consecutive real visits on one project, report produced on the laptop. **→ v0.2 = MVP**

### M5 — Later (after v0.2 feedback)
- Automatic sync, voice notes, iOS.
- Backlog from client answers 2026-10-01: **plan revisions** (a new PDF replaces the plan in place, pins carry over, no history kept); tablet-specific layout (no tablet for now); the ≥ 600 px pin-list panel stays for Windows.

## 5. Definition of done (every sprint)
- Android + Windows builds green in CI.
- Rust unit tests for storage, report and archive; manual test on a real Android phone.
- `CHANGELOG.md` and `docs/decisions.md` updated.

## 6. Key risks
| Risk | Mitigation |
|---|---|
| Camera capture in Android WebView | Spike in Sprint 0; Kotlin plugin fallback |
| A1 plans → lag / memory on phone | Import-time tile pyramid measured on a real phone: first view < 0.2 s, 221 MB at 8× (D-014). Remaining risk: ~1.4 GB one-off peak while generating tiles; generation is foreground, resumable per level |
| Archive import conflicts (same project on two devices) | v1: import creates a copy or overwrites after confirm; real sync later |
| HyperOS (Xiaomi 15) kills background apps aggressively | Tile generation stays resumable per level (D-014): a kill costs at most one level and the plan opens once the 1024 level exists. Sprint 3b: a pending camera capture and the open observation sheet survive process death (D-022, verified by killing the process behind the camera) |
| No Android device on hand | Emulator + a cheap test phone, or friend's device |
| Windows build from macOS | GitHub Actions Windows runner |
