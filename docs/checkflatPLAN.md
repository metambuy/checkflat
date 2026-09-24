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
- M4 Tap to place pin; drag to adjust; stored as normalised x/y (0–1).
- M5 Observation: ref number (continuous per project, editable), description, ≥1 photo (camera or gallery), timestamp.
- M6 Photo annotation: ellipse, arrow, freehand; red; saved non-destructively (original kept).
- M7 Observation list; tap to jump to pin.
- M8 Visits + follow-ups: carry over open observations; mark resolved (with date).
- M9 PDF report per visit in EN or PT; share (Android) / save (Windows).
- M10 Export project as one archive file (.zip) and import it on another device (phone → laptop).
- M11 Responsive UI: phone, tablet, Windows (touch + mouse).
- M12 Works fully offline.

### Should
- S1 Report header: logo, project name, address, visit date, attendees.
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
| PDF viewing | **PDF.js** in WebView, canvas, tiled at high zoom | No native PDFium bundling |
| Plan snapshots | PDF.js → canvas → draw pin/box → PNG → Rust | Reuses viewer |
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

## 4. Milestones & sprints (1 sprint ≈ 1 week part-time)

### M0 — Foundations & risk spikes
- **Sprint 0**: Tauri v2 scaffold; Android emulator + Windows CI green. Spikes: (a) PDF.js zoom on a real A1 plan on Android; (b) camera capture; (c) Typst vs printpdf 1-page report with image. Log in `docs/decisions.md`.
- *Exit*: APK opens a plan, takes a photo, outputs a PDF.

### M1 — Plans & pins
- **Sprint 1**: SQLite schema + migrations; projects CRUD; plan import; EN/PT i18n setup.
- **Sprint 2**: plan viewer (pan/zoom); tap/drag pins; responsive layout phone/tablet/desktop.
- *Exit*: pins persist after restart, on phone and tablet.

### M2 — Observations & photos
- **Sprint 3**: observation sheet (ref, description, photos); image compression.
- **Sprint 4**: photo annotation (ellipse, arrow, freehand); observation list + jump-to-pin.
- *Exit*: full field workflow offline.

### M3 — Report (first usable)
- **Sprint 5**: plan crop + overview snapshots; report template (cover + blocks) in EN/PT.
- **Sprint 6**: share/save on Android and Windows; performance at 50 observations.
- *Exit*: report matches the Aproplan sample. **→ v0.1, field test with friend**

### M4 — Follow-ups & transfer (MVP complete)
- **Sprint 7**: visits; follow-up visit carries over open items; mark resolved; resolved shown in report.
- **Sprint 8**: project export/import archive (phone → laptop); report header + summary table.
- *Exit*: two consecutive real visits on one project, report produced on the laptop. **→ v0.2 = MVP**

### M5 — Later (after v0.2 feedback)
- Automatic sync, voice notes, iOS.

## 5. Definition of done (every sprint)
- Android + Windows builds green in CI.
- Rust unit tests for storage, report and archive; manual test on a real Android phone.
- `CHANGELOG.md` and `docs/decisions.md` updated.

## 6. Key risks
| Risk | Mitigation |
|---|---|
| Camera capture in Android WebView | Spike in Sprint 0; Kotlin plugin fallback |
| A1 plans → lag on phone | Capped render resolution, tiling; test with real plan |
| Archive import conflicts (same project on two devices) | v1: import creates a copy or overwrites after confirm; real sync later |
| No Android device on hand | Emulator + a cheap test phone, or friend's device |
| Windows build from macOS | GitHub Actions Windows runner |
