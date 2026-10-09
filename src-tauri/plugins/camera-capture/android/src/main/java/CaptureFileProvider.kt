package com.checkflat.cameracapture

import androidx.core.content.FileProvider

/**
 * The plugin's own FileProvider. A subclass (not `androidx.core.content.FileProvider` itself) so the
 * manifest merger does not collide with the app template's provider, which is then no longer
 * needed by the plugin. Its paths are in `res/xml/camera_capture_paths.xml`.
 */
class CaptureFileProvider : FileProvider()
