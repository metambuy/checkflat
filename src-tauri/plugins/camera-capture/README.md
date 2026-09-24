# tauri-plugin-camera-capture
Minimal Android-only Tauri mobile plugin (Sprint 0 Spike B). `capture()` fires `MediaStore.ACTION_IMAGE_CAPTURE` with a
`FileProvider` URI in the app cache dir and resolves with the file path. No `CAMERA` permission is declared on purpose:
`ACTION_IMAGE_CAPTURE` does not need it, and declaring it without a runtime grant makes the intent throw `SecurityException`.
