package com.checkflat.cameracapture

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia
import androidx.core.content.FileProvider
import androidx.exifinterface.media.ExifInterface
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * The app's Android bridge: system camera (`capture`), system Photo Picker (`pickImage`, no
 * storage permission) and display names of picked files. Both image commands resolve the path of
 * a file in the app cache, or no path when the user cancelled; HEIC/HEIF is converted to JPEG here
 * because the Rust decoder cannot read it. The capture uses the app's FileProvider (authority
 * "<applicationId>.fileprovider", declared by the Tauri Android template with cache-path ".").
 */
@InvokeArg
class DisplayNameArgs {
    var uri: String? = null
}

@TauriPlugin
class CameraCapturePlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingFile: File? = null
    private var pickPending = false

    /**
     * Display name of a picked file (content:// via ContentResolver, file:// via the path).
     * Runs off the main thread: a remote document provider can block. Resolves { name: String? }.
     */
    @Command
    fun displayName(invoke: Invoke) {
        val uriStr = invoke.parseArgs(DisplayNameArgs::class.java).uri
        if (uriStr.isNullOrBlank()) {
            invoke.reject("uri is required")
            return
        }
        Thread {
            val name = try {
                resolveDisplayName(Uri.parse(uriStr))
            } catch (e: Exception) {
                null
            }
            val ret = JSObject()
            if (name != null) ret.put("name", name)
            invoke.resolve(ret)
        }.start()
    }

    private fun resolveDisplayName(uri: Uri): String? {
        val raw = when (uri.scheme) {
            "content" -> activity.contentResolver
                .query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { c ->
                    val idx = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (idx >= 0 && c.moveToFirst()) c.getString(idx) else null
                }
            "file" -> uri.path?.let { File(it).name }
            else -> null
        } ?: return null
        val cleaned = raw.replace(Regex("[\\p{Cntrl}/\\\\]"), "").trim().take(200)
        return cleaned.ifEmpty { null }
    }

    @Command
    fun capture(invoke: Invoke) {
        // Tauri's PluginManager keeps a single activity-result callback; a second capture while the
        // camera is open would orphan the first Invoke (its promise would never settle).
        if (pendingFile != null || pickPending) {
            invoke.reject("capture already in progress")
            return
        }
        purgeOldCache()
        val dir = File(activity.cacheDir, CAPTURES).apply { mkdirs() }
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
            invoke.resolve(JSObject()) // cancelled: no path
            return
        }
        invoke.resolve(JSObject().apply { put("path", file.absolutePath) })
    }

    /** Photo Picker (Android 11+; backported by Play services; ACTION_OPEN_DOCUMENT fallback). */
    @Command
    fun pickImage(invoke: Invoke) {
        if (pendingFile != null || pickPending) {
            invoke.reject("capture already in progress")
            return
        }
        purgeOldCache()
        val request = PickVisualMediaRequest.Builder().setMediaType(PickVisualMedia.ImageOnly).build()
        val intent = PickVisualMedia().createIntent(activity, request)
        pickPending = true
        try {
            startActivityForResult(invoke, intent, "onPickResult")
        } catch (e: ActivityNotFoundException) {
            pickPending = false
            invoke.reject("no photo picker available")
        }
    }

    @ActivityCallback
    fun onPickResult(invoke: Invoke, result: ActivityResult) {
        pickPending = false
        val uri = PickVisualMedia().parseResult(result.resultCode, result.data)
        if (uri == null) {
            invoke.resolve(JSObject()) // cancelled: no path
            return
        }
        // Copy and convert off the main thread: a large photo or a cloud-backed provider can be slow.
        Thread {
            try {
                val dir = File(activity.cacheDir, PICKS).apply { mkdirs() }
                val copy = File(dir, "PICK_${System.currentTimeMillis()}.img")
                activity.contentResolver.openInputStream(uri)?.use { input ->
                    copy.outputStream().use { input.copyTo(it) }
                } ?: throw java.io.IOException("cannot open $uri")
                val ready = prepare(copy)
                invoke.resolve(JSObject().apply { put("path", ready.absolutePath) })
            } catch (e: Exception) {
                invoke.reject(e.message ?: e.toString())
            }
        }.start()
    }

    private fun isHeif(f: File): Boolean {
        val head = ByteArray(12)
        val n = f.inputStream().use { it.read(head) }
        if (n < 12 || String(head, 4, 4, Charsets.US_ASCII) != "ftyp") return false
        return String(head, 8, 4, Charsets.US_ASCII) in HEIF_BRANDS
    }

    /**
     * HEIC/HEIF → JPEG (decoder applies the orientation; sampled so the long side stays ≥ 1600 px;
     * the capture time is copied into the JPEG's EXIF, where the Rust side reads it). Any other
     * format is returned untouched: Rust reads JPEG, PNG and WebP and their EXIF itself.
     */
    private fun prepare(file: File): File {
        if (!isHeif(file)) return file
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) {
            file.delete()
            throw java.io.IOException("image_unsupported: HEIF needs Android 9 or newer")
        }
        val out = File(file.parentFile, file.nameWithoutExtension + ".jpg")
        try {
            val bitmap = ImageDecoder.decodeBitmap(ImageDecoder.createSource(file)) { decoder, info, _ ->
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                val longSide = maxOf(info.size.width, info.size.height)
                var sample = 1
                while (longSide / (sample * 2) >= TARGET_LONG_SIDE) sample *= 2
                if (sample > 1) decoder.setTargetSampleSize(sample)
            }
            out.outputStream().use { bitmap.compress(Bitmap.CompressFormat.JPEG, 92, it) }
            bitmap.recycle()
            copyCaptureTime(file, out)
        } catch (e: Exception) {
            out.delete()
            throw java.io.IOException("unreadable_image: ${e.message}")
        } finally {
            file.delete()
        }
        return out
    }

    private fun copyCaptureTime(from: File, to: File) {
        try {
            val src = ExifInterface(from)
            val time = src.getAttribute(ExifInterface.TAG_DATETIME_ORIGINAL) ?: src.getAttribute(ExifInterface.TAG_DATETIME)
            if (time == null) return
            val dst = ExifInterface(to)
            dst.setAttribute(ExifInterface.TAG_DATETIME_ORIGINAL, time)
            src.getAttribute(ExifInterface.TAG_OFFSET_TIME_ORIGINAL)?.let { dst.setAttribute(ExifInterface.TAG_OFFSET_TIME_ORIGINAL, it) }
            dst.saveAttributes()
        } catch (e: Exception) {
            // No capture time is not an error: the Rust side falls back to the import time.
        }
    }

    /** Drops cache copies older than a day (the Rust side has long since staged them). */
    private fun purgeOldCache() {
        val cutoff = System.currentTimeMillis() - 24L * 3600 * 1000
        for (name in arrayOf(CAPTURES, PICKS)) {
            File(activity.cacheDir, name).listFiles()?.forEach { f ->
                if (f.isFile && f.lastModified() < cutoff && f != pendingFile) f.delete()
            }
        }
    }

    private companion object {
        const val CAPTURES = "captures"
        const val PICKS = "picks"
        const val TARGET_LONG_SIDE = 1600
        val HEIF_BRANDS = setOf("heic", "heix", "hevc", "hevx", "heim", "heis", "mif1", "msf1")
    }
}
