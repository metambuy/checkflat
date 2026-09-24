package com.checkflat.cameracapture

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.provider.MediaStore
import androidx.activity.result.ActivityResult
import androidx.core.content.FileProvider
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Launches the system camera app and returns the captured JPEG's path.
 * Uses the app's FileProvider (authority "<applicationId>.fileprovider", declared by the Tauri
 * Android template with cache-path "."), so no extra manifest entries are needed.
 */
@TauriPlugin
class CameraCapturePlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingFile: File? = null

    @Command
    fun capture(invoke: Invoke) {
        val dir = File(activity.cacheDir, "captures").apply { mkdirs() }
        val stamp = SimpleDateFormat("yyyyMMdd_HHmmss", Locale.US).format(Date())
        val file = File(dir, "IMG_$stamp.jpg")
        val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)

        val intent = Intent(MediaStore.ACTION_IMAGE_CAPTURE)
            .putExtra(MediaStore.EXTRA_OUTPUT, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        pendingFile = file
        try {
            startActivityForResult(invoke, intent, "onCaptureResult")
        } catch (e: ActivityNotFoundException) {
            pendingFile = null
            invoke.reject("no camera app available")
        }
    }

    @ActivityCallback
    fun onCaptureResult(invoke: Invoke, result: ActivityResult) {
        val file = pendingFile
        pendingFile = null
        if (result.resultCode != Activity.RESULT_OK || file == null || !file.exists() || file.length() == 0L) {
            file?.delete()
            invoke.reject("cancelled")
            return
        }
        invoke.resolve(JSObject().apply { put("path", file.absolutePath) })
    }
}
