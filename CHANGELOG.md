# Changelog

## Sprint 0 — 2026-09-24 (foundations & risk spikes)
- Tauri v2 + Svelte 5/TypeScript scaffold; Android project generated; Cargo workspace with `src-tauri` and `spikes/report-spike`.
- GitHub Actions: Android debug APK (three variants for engine size comparison) and Windows NSIS installer.
- Spike A: PDF.js viewer for a real A1 plan with capped base render + viewport tile on zoom; pan/pinch/wheel gestures.
- Spike B: camera capture via `<input capture>` on Android WebView.
- Spike C: one-page PDF (title, image, PT text) from Rust with embedded Typst and with printpdf; unit tests per engine.
- `docs/decisions.md` D-001…D-006.
