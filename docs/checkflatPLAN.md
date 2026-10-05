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
- **Ref numbering:** not one continuous number. Per-project configurable ref format built from project code + fraction + sequence. **Exact format still pending client confirmation.** Planned design (Sprint 3 migration, D-018): `project.code`; `observation.fraction` + `observation.seq`; per-project template with tokens `{PROJ}` `{FRAC}` `{SEQ:n}`; sequence scope per fraction or per project; the displayed ref is computed from the template; fractions come from a per-project list (typing a new one adds it); common areas use a code such as "PC". This replaces `project.next_ref_no` and `UNIQUE(project_id, ref_no)` — **pending migration, schema unchanged for now**.
- **Report:** open observations newest first; a separate section with **all** closed observations since the project start, **without photos**; the cover summary lists the observations closed in this visit. Attendees optional (shown only if filled). **No logo.** Typical length 20–50 pages.
- **Plan revisions:** a new revision replaces the plan in place, pins carry over, no history kept. Backlog, after v0.2.
- **Devices:** Xiaomi 15 (Android 15, HyperOS) + Windows laptop. **No tablet for now.** Acceptance stays on the P30 Pro with scaled time targets (D-014). HyperOS kills background apps aggressively → tile generation must stay resumable.

## 1. Concept
- **Project** (a building/site) → contains **Plans** (one PDF each) and **Visits** (dated sessions).
- During a **Visit**, the user opens a plan, taps a spot → creates an **Observation**: auto ref number, pin, photo(s) with annotation, description.
- **Follow-up visit**: starts with all still-open observations from previous visits; user marks each resolved or keeps it open, and adds new ones.
- End of visit → **Report** PDF: cover page + one block per observation (ref, zoomed plan crop with pin, overview plan with red box, photo(s) with timestamp, description, open/resolved).

## 2. Requirements

### Must (MVP)
- M1 Create/rename/delete projects.
- M2 Import PDF plan (one page per file, A3/A1); plan title.
- M3 Smooth pan/zoom on A1 plans on Android phone and tablet.
- M4 Add-pin mode ("+", then tap) places a pin; drag to adjust; stored as normalised x/y (0–1).
- M5 Observation: ref (per-project format: project code + fraction + sequence, §0 2026-10-01; exact format pending), description, ≥1 photo (camera or gallery), timestamp.
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
| Plan snapshots | Rust stitches crops from the tiles (`image` crate), draws pin/box → PNG | No WebView or PDF.js at report time; same on Windows (D-014) |
| Photo annotation | Canvas overlay; shapes stored as JSON; flattened at report time | Non-destructive edits |
| Storage | SQLite (`rusqlite`) + files in app data dir | Offline, portable |
| Images | Rust `image` crate: resize to ~1600 px JPEG, keep EXIF date | Small reports |
| Report | Rust: embedded **Typst** or `printpdf` (decide in spike) | Templated layout, EN/PT |
| Transfer | Project archive: `.zip` with SQLite export + files | Simple phone → laptop move |
| Camera | `<input capture>` first; fallback Tauri mobile plugin (Kotlin) | Android WebView camera capture is unreliable in hybrid apps |
| i18n | Shared EN/PT string files for UI + report | One source of truth |
| CI | GitHub Actions: Android APK + Windows installer | Dev machine is macOS |

### Data model (v1)
```
project(id, name, address, logo_path, next_ref_no, created_at)
plan(id, project_id, file_path, title, width_pt, height_pt)
visit(id, project_id, date, attendees, notes, report_lang)
observation(id, project_id, plan_id, ref_no, x_norm, y_norm, description,
            created_visit_id, resolved_visit_id NULL, resolved_at NULL, created_at)
photo(id, observation_id, visit_id, file_path, taken_at, annotation_json)
```
Note: observations belong to the project (not one visit) so they can carry over; photos link to the visit they were taken in.

**Pending migration (Sprint 3, D-018):** `project.next_ref_no`, `observation.ref_no` and `UNIQUE(project_id, ref_no)` conflict with the client's ref format (project code + fraction + sequence). Planned: `project.code`, a per-project ref template and fraction list, `observation.fraction` + `observation.seq`, display ref computed from the template. Not changed until the client confirms the exact format; until then Confirm uses `next_ref_no` as a placeholder through one core function (`assign_ref`). `project.logo_path` is unused (no logo in the report).

## 4. Milestones & sprints (1 sprint ≈ 1 week part-time)

### M0 — Foundations & risk spikes
- **Sprint 0**: Tauri v2 scaffold; Android emulator + Windows CI green. Spikes: (a) PDF.js zoom on a real A1 plan on Android; (b) camera capture; (c) Typst vs printpdf 1-page report with image. Log in `docs/decisions.md`.
- *Exit*: APK opens a plan, takes a photo, outputs a PDF.

### M1 — Plans & pins
- **Sprint 1** ✅ (2026-09-25): SQLite schema + migrations; projects CRUD; plan import; EN/PT i18n setup. Decisions D-008…D-013.
- **Sprint 2** ✅ (2026-09-26; client feedback applied 2026-10-01): plan viewer (pan/zoom); add-pin mode + drag pins; phone layout + Windows mouse/keys. Decisions D-014…D-018.
  - Step 1 — viewer spike (≤ 1 day, after D-006): `VITE_SPIKES` flag so a release APK shows the Dev screen; PDF.js stage profile (document / operator list + image decode / rasterise, legacy vs modern build, A1 vs A4); on the P30 Pro compare (1) D-002 revised (1024 px quick pass → 3072 px base + viewport tile) vs (2) import-time tile pyramid (512 px tiles, 4 levels, WebP on disk, viewer shows images only; incl. generation time, disk size, peak PSS while generating, backgrounding). Targets for the client's 2024–25 phone: first view ≤ 4 s, total PSS (app + WebView renderer) ≤ 300 MB at 8×, smooth pan; on the P30 Pro time targets are scaled ×2 (≤ 8 s), memory is not. Result → D-014; then a checkpoint before the rest of the sprint. Release APKs and `dist/` from the spike are deleted afterwards (client plan bundled).
  - Then: viewer with the chosen design (**D-014: tile pyramid**, generated in the foreground after import, resumable per level; max zoom capped at ≤ 1.5× upscaling of the top level); tap places a **draft** pin (no ref_no) with Confirm/Cancel — ref_no is taken from `next_ref_no` only on confirm (Sprint 3: observation sheet save), so cancelled drafts leave no gaps; drag to adjust; stored as `x_norm`/`y_norm`; Android back handling; responsive layout (tablet/desktop side panel moves to Sprint 4 if the sprint runs long); asset protocol scoped to `projects/**` (resolves the D-007 item); address field save feedback.
  - Client feedback 2026-10-01 (D-018): tap-to-place replaced by the **add-pin mode** ("+" → next tap places the draft and leaves the mode; a tap outside the mode does nothing; Back leaves the mode); ref assignment isolated in one core function (`assign_ref`, placeholder on `next_ref_no` until the Sprint 3 migration); the **tablet/desktop side panel is out of Sprint 2 scope → backlog** (no tablet for now).
- *Exit*: pins persist after restart, on the phone (and Windows).

### M2 — Observations & photos
- **Sprint 3**: observation sheet (ref, description, photos); image compression.
  - **Ref format migration (D-018)**, once the client confirms the exact format: `project.code`, per-project template (`{PROJ}` `{FRAC}` `{SEQ:n}`), sequence scope per fraction or per project, per-project fraction list, `observation.fraction` + `observation.seq`; drop `project.next_ref_no` and `UNIQUE(project_id, ref_no)`; swap `assign_ref`.
  - Camera plugin follow-ups deferred from the Sprint 0 code review (see decisions D-007): (a) persist the pending capture path so a photo survives the app being killed behind the camera; (b) self-contained FileProvider (own subclass, `file_paths.xml` and `<provider>` in the plugin manifest) instead of relying on the Tauri app template; (c) ~~decide `assetProtocol` vs base64 for showing photos~~ resolved in Sprint 2: asset protocol, scope `projects/**` (D-015).
- **Sprint 4**: photo annotation (ellipse, arrow, freehand); observation list + jump-to-pin.
  - Backlog from Sprint 2: pins overlap at fit when many are close together (smaller markers or clustering at low zoom); the import dialog still says "Choose a PDF…" for the ~3 s the picker result takes on EMUI (show "Reading PDF…" as soon as the picker closes). Tile cache: one fsync pass per level in `write_manifest` instead of one per tile (D-019: ≈ +10 s on the A1 plan's one-off preparation on the P30).
- *Exit*: full field workflow offline.

### M3 — Report (first usable)
- **Sprint 5**: plan crop + overview snapshots; report template (cover + blocks) in EN/PT.
  - Client 2026-10-01: open observations newest first; no logo; attendees only if filled; 20–50 pages typical.
- **Sprint 6**: share/save on Android and Windows; performance at 50 observations.
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
| HyperOS (Xiaomi 15) kills background apps aggressively | Tile generation stays resumable per level (D-014): a kill costs at most one level and the plan opens once the 1024 level exists. Sprint 3: pending camera capture must survive process death (D-007 a) |
| No Android device on hand | Emulator + a cheap test phone, or friend's device |
| Windows build from macOS | GitHub Actions Windows runner |
