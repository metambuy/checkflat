package com.checkflat.cameracapture

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.provider.MediaStore
import android.util.Log
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia
import androidx.core.app.ShareCompat
import androidx.core.content.FileProvider
import androidx.core.content.pm.PackageInfoCompat
import androidx.exifinterface.media.ExifInterface
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
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
 * storage permission), display names of picked files, opening and sharing the report
 * (`viewPdf`, `sharePdf`) and the installed version (`appVersion`). Both image commands resolve
 * the path of a file in the app cache, or no path when the user cancelled; HEIC/HEIF is converted
 * to JPEG here because the Rust decoder cannot read it. The capture and the report go through the
 * plugin's own [CaptureFileProvider] (authority "<applicationId>.cameracapture", declared in this
 * plugin's manifest; cache `captures/` and `reports/` only, D-022, D-028).
 */
@InvokeArg
class DisplayNameArgs {
    var uri: String? = null
}

@InvokeArg
class PdfArgs {
    var path: String? = null
    var title: String? = null
}

/**
 * A failure of the pick that the app has a message for. The `code` travels with the rejection
 * (`invoke.reject(message, code)`), through the plugin's Rust error to the page, which translates it
 * (src/lib/errors.ts, src/lib/pluginError.ts). Anything else the pick throws has no code and shows
 * as the generic error, its text going to dev.log.
 */
private class PickFailure(val code: String, message: String) : java.io.IOException(message)

@TauriPlugin
class CameraCapturePlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingFile: File? = null
    private var pickPending = false
    // When the camera was launched (elapsedRealtime), for the "after … ms" in the result line.
    private var captureStartedAt = 0L

    // The pick that has not been settled yet. `pickImage` must always resolve (a path, or nothing
    // when the user backed out); if the system never delivers the activity result, the lifecycle
    // watchdog below settles it as a cancel, so the UI is never left waiting.
    @Volatile private var pickInvoke: Invoke? = null
    private var pickObserver: LifecycleEventObserver? = null
    private val main = Handler(Looper.getMainLooper())

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
            Log.w(TAG, "capture: rejected (in progress)")
            invoke.reject("capture already in progress")
            return
        }
        purgeOldCache()
        val dir = File(activity.cacheDir, CAPTURES).apply { mkdirs() }
        val stamp = SimpleDateFormat("yyyyMMdd_HHmmss", Locale.US).format(Date())
        val file = File(dir, "IMG_$stamp.jpg")
        val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.$PROVIDER_SUFFIX", file)

        val intent = Intent(MediaStore.ACTION_IMAGE_CAPTURE)
            .putExtra(MediaStore.EXTRA_OUTPUT, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        pendingFile = file
        // Persist the target before the camera opens: if Android kills this process while the
        // camera is in front, the photo still lands in this file and `takeRecoveredCapture` finds it.
        prefs().edit().putString(PENDING_CAPTURE, file.absolutePath).commit()
        captureStartedAt = SystemClock.elapsedRealtime()
        Log.i(TAG, "capture: launching file=${file.name} api=${Build.VERSION.SDK_INT}")
        try {
            startActivityForResult(invoke, intent, "onCaptureResult")
        } catch (e: ActivityNotFoundException) {
            pendingFile = null
            prefs().edit().remove(PENDING_CAPTURE).commit()
            Log.w(TAG, "capture: no camera app")
            invoke.reject("no camera app available", NO_CAMERA_APP)
        }
    }

    private fun prefs() = activity.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /**
     * A photo taken while Android killed the app: Tauri drops the camera's result when the process
     * was recreated (its callback died with it), but the file is on disk and its path was saved by
     * `capture`. Resolves `{ path }` once (then forgets it), or nothing when there is none, the
     * file is missing or empty, or a capture is live in this process (that one is not lost).
     */
    @Command
    fun takeRecoveredCapture(invoke: Invoke) {
        if (pendingFile != null) {
            invoke.resolve(JSObject())
            return
        }
        val saved = prefs().getString(PENDING_CAPTURE, null)
        prefs().edit().remove(PENDING_CAPTURE).commit()
        val file = saved?.let { File(it) }
        val captures = File(activity.cacheDir, CAPTURES).canonicalPath + File.separator
        if (file != null && file.isFile && file.length() > 0 && file.canonicalPath.startsWith(captures)) {
            Log.i(TAG, "recovered a capture taken while the app was not running: ${file.name}")
            invoke.resolve(JSObject().apply { put("path", file.absolutePath) })
        } else {
            file?.delete()
            invoke.resolve(JSObject())
        }
    }

    @ActivityCallback
    fun onCaptureResult(invoke: Invoke, result: ActivityResult) {
        val file = pendingFile
        pendingFile = null
        prefs().edit().remove(PENDING_CAPTURE).commit()
        val after = if (captureStartedAt > 0L) SystemClock.elapsedRealtime() - captureStartedAt else -1L
        captureStartedAt = 0L
        Log.i(TAG, "capture: result code=${result.resultCode} exists=${file?.exists()} bytes=${file?.length()} after $after ms")
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
        Log.i(TAG, "pick: launching ${intent.action} (API ${Build.VERSION.SDK_INT})")
        pickPending = true
        pickInvoke = invoke
        watchPick(invoke)
        try {
            startActivityForResult(invoke, intent, "onPickResult")
        } catch (e: ActivityNotFoundException) {
            pickPending = false
            pickInvoke = null
            unwatchPick()
            invoke.reject("no photo picker available", NO_PHOTO_PICKER)
        }
    }

    /** After the picker took our activity away and gave it back, wait a moment for the result, then settle as a cancel. */
    private fun watchPick(invoke: Invoke) {
        activity.runOnUiThread {
            var left = false
            val observer = LifecycleEventObserver { _, event ->
                if (event == Lifecycle.Event.ON_PAUSE) left = true
                if (event == Lifecycle.Event.ON_RESUME && left) {
                    main.postDelayed({ pickLost(invoke) }, LOST_RESULT_GRACE_MS)
                }
            }
            pickObserver = observer
            (activity as? LifecycleOwner)?.lifecycle?.addObserver(observer)
        }
    }

    private fun unwatchPick() {
        val observer = pickObserver ?: return
        pickObserver = null
        activity.runOnUiThread { (activity as? LifecycleOwner)?.lifecycle?.removeObserver(observer) }
    }

    private fun pickLost(invoke: Invoke) {
        if (pickInvoke !== invoke) return // the result arrived (or it was settled already)
        Log.w(TAG, "pick: no activity result after the app resumed; settling as cancelled")
        pickInvoke = null
        pickPending = false
        unwatchPick()
        invoke.resolve(JSObject())
    }

    @ActivityCallback
    fun onPickResult(invoke: Invoke, result: ActivityResult) {
        if (pickInvoke !== invoke) {
            Log.w(TAG, "pick: result code=${result.resultCode} arrived after the pick was settled; ignored")
            return
        }
        pickInvoke = null
        pickPending = false
        unwatchPick()
        val uri = PickVisualMedia().parseResult(result.resultCode, result.data)
        Log.i(TAG, "pick: result code=${result.resultCode} uri=${uri != null}")
        if (uri == null) {
            invoke.resolve(JSObject()) // cancelled: no path
            return
        }
        // Copy and convert off the main thread: a large photo or a cloud-backed provider can be slow.
        Thread {
            try {
                val dir = File(activity.cacheDir, PICKS).apply { mkdirs() }
                val copy = File(dir, "PICK_${System.currentTimeMillis()}.img")
                try {
                    activity.contentResolver.openInputStream(uri)?.use { input ->
                        copy.outputStream().use { input.copyTo(it) }
                    } ?: throw java.io.IOException("no stream")
                } catch (e: Exception) {
                    // A cloud-only item while offline, a provider that went away, a revoked grant.
                    throw PickFailure(SOURCE_UNREADABLE, "cannot open $uri: ${e.message}")
                }
                val ready = prepare(copy)
                invoke.resolve(JSObject().apply { put("path", ready.absolutePath) })
            } catch (e: PickFailure) {
                Log.w(TAG, "pick: failed (${e.code})", e)
                invoke.reject(e.message ?: e.code, e.code)
            } catch (e: Exception) {
                Log.w(TAG, "pick: failed", e)
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
            throw PickFailure(IMAGE_UNSUPPORTED, "HEIF needs Android 9 or newer")
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
            throw PickFailure(UNREADABLE_IMAGE, e.message ?: e.toString())
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

    /**
     * Opens a report copy in a PDF viewer: `ACTION_VIEW` with its FileProvider content URI, type
     * `application/pdf` and a temporary read grant (FLAG_GRANT_READ_URI_PERMISSION; "Grant URI access"
     * in https://developer.android.com/training/package-visibility/use-cases, and the FileProvider
     * reference, https://developer.android.com/reference/androidx/core/content/FileProvider). The
     * viewer is not looked up first: since Android 11, package visibility filters what
     * `resolveActivity` sees, and the documented way is to start the intent and catch
     * ActivityNotFoundException (same use-cases page). That case rejects with code `no_app`.
     * `path` must be a file under `cacheDir/reports/` (the Rust side copies the report there).
     */
    @Command
    fun viewPdf(invoke: Invoke) {
        val file = reportCopy(invoke, invoke.parseArgs(PdfArgs::class.java)) ?: return
        val intent = Intent(Intent.ACTION_VIEW)
            .setDataAndType(uriFor(file), PDF)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        launch(invoke, "view", intent, file)
    }

    /**
     * Shares a report copy through the system share sheet (https://developer.android.com/training/sharing/send,
     * "Send binary content"). ShareCompat puts the stream into ClipData and adds the read grant
     * (androidx.core 1.5.0, https://developer.android.com/jetpack/androidx/releases/core), and
     * `Intent.createChooser` carries both onto the chooser intent, so the app picked there can read
     * the URI (https://developer.android.com/reference/android/content/Intent#createChooser(android.content.Intent,%20java.lang.CharSequence)).
     * The recipient sees the copy's file name (FileProvider's DISPLAY_NAME). `title` is shown by
     * choosers that use it (Android's own share sheet ignores it for ACTION_SEND).
     */
    @Command
    fun sharePdf(invoke: Invoke) {
        val args = invoke.parseArgs(PdfArgs::class.java)
        val file = reportCopy(invoke, args) ?: return
        val chooser = ShareCompat.IntentBuilder(activity)
            .setType(PDF)
            .setStream(uriFor(file))
            .setChooserTitle(args.title ?: "")
            .createChooserIntent()
        launch(invoke, "share", chooser, file)
    }

    /** The installed versionName and versionCode (the CI run number, D-021), for the About line. */
    @Command
    fun appVersion(invoke: Invoke) {
        val info = activity.packageManager.getPackageInfo(activity.packageName, 0)
        invoke.resolve(JSObject().apply {
            put("versionName", info.versionName ?: "")
            put("versionCode", PackageInfoCompat.getLongVersionCode(info))
        })
    }

    /**
     * Sends the app's task to the background as Home does, instead of finishing the activity (D-029):
     * Back on the Projects screen while a report is being generated. Finishing the activity ends the
     * process, and the run with it; in the background the run goes on and the card shows the result
     * on return.
     */
    @Command
    fun moveTaskToBack(invoke: Invoke) {
        activity.moveTaskToBack(true)
        invoke.resolve()
    }

    /** The file `args.path` names, if it is a regular file under `cacheDir/reports/`; rejects otherwise. */
    private fun reportCopy(invoke: Invoke, args: PdfArgs): File? {
        val file = try {
            args.path?.let { File(it).canonicalFile }
        } catch (e: java.io.IOException) {
            null
        }
        val root = File(activity.cacheDir, REPORTS).canonicalPath + File.separator
        if (file == null || !file.path.startsWith(root) || !file.isFile) {
            Log.w(TAG, "report: refused path ${args.path}")
            invoke.reject("not a report copy", "invalid_path")
            return null
        }
        return file
    }

    private fun uriFor(file: File): Uri = FileProvider.getUriForFile(activity, "${activity.packageName}.$PROVIDER_SUFFIX", file)

    private fun launch(invoke: Invoke, what: String, intent: Intent, file: File) {
        activity.runOnUiThread {
            try {
                activity.startActivity(intent)
                Log.i(TAG, "$what: started for ${file.name} (${file.length()} bytes)")
                invoke.resolve()
            } catch (e: ActivityNotFoundException) {
                Log.w(TAG, "$what: no app for $PDF")
                invoke.reject("no app to $what a PDF", NO_APP)
            }
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
        const val TAG = "CheckflatPick"
        const val LOST_RESULT_GRACE_MS = 1500L
        const val PREFS = "camera-capture"
        const val PENDING_CAPTURE = "pending_capture"
        const val PROVIDER_SUFFIX = "cameracapture"
        const val CAPTURES = "captures"
        const val PICKS = "picks"
        // Report copies made by the Rust side (`reports::share_copy`); `reports/` in camera_capture_paths.xml.
        const val REPORTS = "reports"
        const val PDF = "application/pdf"
        const val NO_APP = "no_app"
        // Codes of the rejections the app translates (src/lib/pluginError.ts keeps the list).
        const val NO_CAMERA_APP = "no_camera_app"
        const val NO_PHOTO_PICKER = "no_photo_picker"
        const val SOURCE_UNREADABLE = "source_unreadable"
        const val IMAGE_UNSUPPORTED = "image_unsupported"
        const val UNREADABLE_IMAGE = "unreadable_image"
        const val TARGET_LONG_SIDE = 1600
        // Keep in step with `is_heif` in crates/checkflat-core/src/photos.rs.
        val HEIF_BRANDS = setOf("heic", "heix", "hevc", "hevx", "hevm", "hevs", "heim", "heis", "mif1", "mif2", "msf1")
    }
}
