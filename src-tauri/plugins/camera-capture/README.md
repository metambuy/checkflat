# tauri-plugin-camera-capture
The app's Android bridge (started as Sprint 0 Spike B; D-003, D-013, D-022, D-028). Despite the name it does more than the camera:

- `capture()` fires `MediaStore.ACTION_IMAGE_CAPTURE` with a `FileProvider` URI in the app cache dir and resolves with the file path. No `CAMERA` permission is declared on purpose: `ACTION_IMAGE_CAPTURE` does not need it, and declaring it without a runtime grant makes the intent throw `SecurityException`.
- `pickImage()` opens the system Photo Picker, `takeRecoveredCapture()` returns a photo taken while the app was killed, `displayName(uri)` names a picked file. These four are callable from the WebView (`permissions/default.toml`).
- `viewPdf(path)` / `sharePdf(path, title)` open or share a report copy under the cache's `reports/` (`ACTION_VIEW` / `ACTION_SEND` through ShareCompat, read grant on the content URI; `no_app` rejection code when nothing can handle it), and `appVersion()` returns the installed versionName and versionCode. These are called from Rust only (`CameraCapture::view_pdf` etc.), never from the WebView: the app resolves the report's path itself.

Its FileProvider (`CaptureFileProvider`, authority `<applicationId>.cameracapture`) exposes the cache's `captures/` and `reports/` only; CI checks the release APK declares nothing broader (`scripts/check-fileprovider.sh`).
