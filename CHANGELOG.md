# Changelog

## Sprint 2 — 2026-09-26 (plan viewer, pins, responsive layout)
- Viewer spike (D-014): `VITE_SPIKES` release builds with the Dev screen; PDF.js stage profile on the P30 (the open document of the A1 plan holds ~780 MB); design comparison; **import-time tile pyramid** chosen (512 px WebP q0.80, levels 1024–8192, zoom capped at ≤ 1.5× upscaling).
- Core: pins (`create_pin` = confirm, takes the ref number in one transaction; move, delete, list), tile cache storage with a validated manifest, plan delete removes its cache, sweep keeps live plans' cache dirs; 20 new tests.
- Viewer: tile pyramid generated in the foreground on first open (progress, resumable per level, PDF.js loaded only for it), image-tile viewer with pan/pinch/wheel/keys, zoom cap, refit on rotation, pins as a constant-size layer, draft pin with Confirm/Cancel, drag to move, delete with confirmation.
- Layout: plan screen with a bottom action bar; pin list beside the plan on tablet/desktop (≥ 600 px).
- Android: system Back (cancel draft → up one level → leave at the root); content kept inside system bars, cutout and keyboard (edge-to-edge on Android 15 with older WebViews).
- Asset protocol on, scope `projects/**`; tile writes as base64 over IPC (D-015).
- Address field saves itself (after typing stops, on blur/Enter) and shows Saving… / ✓ Saved / Not saved.
- P30 (release): first view 78 ms, 297 MB total at 8× with 49 pins; one-off preparation 8 s to first view, 35 s to full detail.
- Removed: viewer spike code, `scripts/copy-spike-plan.sh` (plans are tested via in-app import).

## Sprint 1 — 2026-09-25 (schema, projects, plan import, i18n)
- New crate `crates/checkflat-core` (no Tauri dependency): rusqlite (bundled) with FK/WAL/busy_timeout, rusqlite_migration on `user_version`, migration 1 with all v1 tables, backup via `VACUUM INTO` before migrating, UUID v7 ids, RFC 3339 timestamps.
- Relative-path policy (`RelPath`), project/plan file layout, startup quarantine sweep with fresh-DB guard and containment rules.
- Projects CRUD, settings, staged plan import (read once → inspect with lopdf → rename → insert), multi-page rejection, plan delete blocked while observations exist.
- Tauri: single-connection `AppState`, async commands via `spawn_blocking`, dialog + fs plugins, `displayName` command in the Kotlin plugin for Android picker names.
- UI: projects list/detail, plan import dialog with editable title, confirm dialogs, EN/PT switch persisted in `setting`; Sprint 0 spikes moved behind a dev-only screen.
- 29 core unit/integration tests, run by the CI `checks` job.
- Post-review fixes (ultrareview, 2026-09-25): fixed-width timestamps; quarantine entries under `<id>-<stamp>` dirs purged by stamp (never mtime) with purge before quarantine; fresh-DB detection from `user_version`; strict canonical UUID ids; single-pass `{placeholder}` interpolation in TS and Rust; translated default plan title; shared modal CSS. Frontend unit tests via `node --test` (`pnpm test`) added to the CI checks job.

## Sprint 0 — 2026-09-24 (foundations & risk spikes)
- Tauri v2 + Svelte 5/TypeScript scaffold; Android project generated; Cargo workspace with `src-tauri` and `spikes/report-spike`.
- GitHub Actions: Android debug APK (three variants for engine size comparison) and Windows NSIS installer.
- Spike A: PDF.js viewer for a real A1 plan with capped base render + viewport tile on zoom; pan/pinch/wheel gestures.
- Spike B: camera capture via `<input capture>` on Android WebView.
- Spike C: one-page PDF (title, image, PT text) from Rust with embedded Typst and with printpdf; unit tests per engine. Per-ABI (arm64-v8a) release APK: 34.2 MB typst-only vs 11.8 MB printpdf-only; recommendation Typst (D-004).
- Release profile `opt-level = "z"` (−2.4 MB on the Typst APK); dev profile without dependency debug info.
- Spike B outcome: `<input capture>` opens the Photo Picker; `tauri-plugin-camera-capture` (Kotlin, `ACTION_IMAGE_CAPTURE`) works on phone and tablet emulators (D-003).
- Spike A outcome: PDF.js legacy build required on WebView 124; capped base render + viewport tile, 8× tile in 130–310 ms on the emulators (D-002).
- `docs/decisions.md` D-001…D-006. CI run blocked by GitHub billing on the account; workflow linted and Android commands validated locally (D-005).
