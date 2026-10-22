package com.checkflat.cameracapture

import androidx.core.content.FileProvider

/**
 * The app's only FileProvider (D-022, D-028). A subclass of `androidx.core.content.FileProvider` so
 * its manifest entry cannot collide with a stock provider another library or a regenerated app
 * template might declare; the template's own broad one was removed in Sprint 6a, and CI checks
 * the release APK for it (scripts/check-fileprovider.sh). Its paths are in
 * `res/xml/camera_capture_paths.xml`: the cache's `captures/` and `reports/` only.
 */
class CaptureFileProvider : FileProvider()
