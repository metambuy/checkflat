# Changelog

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
