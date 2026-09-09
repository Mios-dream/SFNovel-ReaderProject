package com.sansan.sf_novel_flow

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Movie
import ai.onnxruntime.OnnxTensor
import ai.onnxruntime.OrtEnvironment
import ai.onnxruntime.OrtSession
import java.io.File
import java.io.FileOutputStream
import java.nio.FloatBuffer
import java.util.Locale
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import pl.droidsonroids.gif.GifDrawable

@InvokeArg
class RecognizeChapterArgs {
    lateinit var sourcePath: String
    lateinit var segmentsDir: String
}

/** Android image OCR plugin. Authentication and cookie state are intentionally out of scope. */
@TauriPlugin
class SfacgOcrPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun recognizeChapter(invoke: Invoke) {
        val args = try {
            invoke.parseArgs(RecognizeChapterArgs::class.java)
        } catch (error: Exception) {
            invoke.reject("OCR 参数无效：${error.message ?: "未知错误"}")
            return
        }
        Thread {
            try {
                val source = File(args.sourcePath)
                require(source.isFile) { "OCR 输入图片不存在" }
                require(source.length() <= MAX_OCR_SOURCE_BYTES) { "OCR 图片过大，已拒绝处理" }
                val segments = File(args.segmentsDir).also { it.mkdirs() }
                val text = OnnxChapterRecognizer(activity).use { recognizer ->
                    recognizeFrames(source, segments, recognizer)
                }
                if (text.isBlank()) throw IllegalStateException("未识别到可用文字")
                invoke.resolve(JSObject().put("text", text))
            } catch (error: Exception) {
                invoke.reject(error.message ?: "Android OCR 失败")
            }
        }.start()
    }

    private fun recognizeFrames(source: File, segments: File, recognizer: OnnxChapterRecognizer): String {
        val frameTexts = ArrayList<String>()
        if (source.extension.lowercase(Locale.ROOT) != "gif") {
            BitmapFactory.decodeFile(source.absolutePath)?.let { bitmap ->
                frameTexts += recognizeBitmap(bitmap, segments, 1, recognizer)
                bitmap.recycle()
            }
        } else {
            val movie = Movie.decodeFile(source.absolutePath)
            if (movie != null && movie.duration() <= 0) {
                BitmapFactory.decodeFile(source.absolutePath)?.let { bitmap ->
                    frameTexts += recognizeBitmap(bitmap, segments, 1, recognizer)
                    bitmap.recycle()
                }
            } else {
                val gif = GifDrawable(source)
                try {
                    require(gif.numberOfFrames > 0) { "GIF 中不包含可识别帧" }
                    for (index in 0 until gif.numberOfFrames) {
                        gif.seekToFrame(index)
                        val frame = Bitmap.createBitmap(gif.intrinsicWidth, gif.intrinsicHeight, Bitmap.Config.ARGB_8888)
                        val canvas = android.graphics.Canvas(frame)
                        canvas.drawColor(android.graphics.Color.WHITE)
                        gif.setBounds(0, 0, frame.width, frame.height)
                        gif.draw(canvas)
                        try { frameTexts += recognizeBitmap(frame, segments, index + 1, recognizer) }
                        finally { frame.recycle() }
                    }
                } finally { gif.recycle() }
            }
        }
        return frameTexts.filter { it.isNotBlank() }.joinToString("\n\n").also {
            saveText(it, File(segments, "ocr-result.txt"))
        }
    }

    private fun recognizeBitmap(bitmap: Bitmap, segments: File, frame: Int, recognizer: OnnxChapterRecognizer): String {
        val flattened = flattenOnWhite(bitmap)
        try {
            val sourceColors = readColors(flattened)
            val denoisedColors = lightlyBlur(sourceColors, flattened.width, flattened.height)
            saveRgb(denoisedColors, flattened.width, flattened.height, File(segments, frameFileName(frame, "denoised.png")))
            val watermark = expandMask(watermarkMask(denoisedColors), flattened.width, flattened.height)
            saveMask(watermark, flattened.width, flattened.height, File(segments, frameFileName(frame, "watermark-mask.png")))
            val cleanedColors = removeWatermark(sourceColors, watermark)
            saveRgb(cleanedColors, flattened.width, flattened.height, File(segments, frameFileName(frame, "watermark-removed.png")))
            val gray = grayPixels(cleanedColors)
            if (!hasRecognizableContent(gray)) return ""
            val bounds = fixedHeightBounds(flattened.height)
            val covered = coverPinyin(gray, flattened.width, bounds)
            saveGray(GrayImage(flattened.width, flattened.height, covered), File(segments, frameFileName(frame, "pinyin-covered.png")))
            val texts = ArrayList<String>()
            var contentLine = 0
            for ((start, end) in bounds) {
                val linePixels = covered.copyOfRange(start * flattened.width, end * flattened.width)
                if (!hasRecognizableContent(linePixels)) continue
                contentLine++
                val line = GrayImage(flattened.width, end - start, linePixels)
                val prefix = "frame-${frame.toString().padStart(4, '0')}-line-${contentLine.toString().padStart(4, '0')}"
                saveGray(line, File(segments, "$prefix.png"))
                val value = recognizer.recognize(line)
                saveText(value, File(segments, "$prefix-ocr.txt"))
                if (value.isNotEmpty()) texts += value
            }
            return texts.joinToString("\n")
        } finally { flattened.recycle() }
    }

    private data class GrayImage(val width: Int, val height: Int, val pixels: IntArray) {
        fun toBitmap(): Bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888).also { output ->
            val colors = IntArray(pixels.size) { index -> android.graphics.Color.rgb(pixels[index], pixels[index], pixels[index]) }
            output.setPixels(colors, 0, width, 0, 0, width, height)
        }
    }

    private fun readColors(bitmap: Bitmap): IntArray = IntArray(bitmap.width * bitmap.height).also {
        bitmap.getPixels(it, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
    }
    private fun grayPixels(colors: IntArray): IntArray = IntArray(colors.size) { index ->
        val color = colors[index]
        (android.graphics.Color.red(color) * 299 + android.graphics.Color.green(color) * 587 + android.graphics.Color.blue(color) * 114) / 1000
    }
    private fun flattenOnWhite(bitmap: Bitmap): Bitmap = Bitmap.createBitmap(bitmap.width, bitmap.height, Bitmap.Config.ARGB_8888).also { output ->
        val canvas = android.graphics.Canvas(output)
        canvas.drawColor(android.graphics.Color.WHITE)
        canvas.drawBitmap(bitmap, 0f, 0f, null)
    }
    private fun lightlyBlur(colors: IntArray, width: Int, height: Int): IntArray {
        val output = IntArray(colors.size)
        val weights = intArrayOf(1, 2, 1)
        for (y in 0 until height) for (x in 0 until width) {
            var red = 0; var green = 0; var blue = 0
            for (oy in -1..1) for (ox in -1..1) {
                val color = colors[(y + oy).coerceIn(0, height - 1) * width + (x + ox).coerceIn(0, width - 1)]
                val weight = weights[oy + 1] * weights[ox + 1]
                red += android.graphics.Color.red(color) * weight
                green += android.graphics.Color.green(color) * weight
                blue += android.graphics.Color.blue(color) * weight
            }
            output[y * width + x] = android.graphics.Color.rgb(red / 16, green / 16, blue / 16)
        }
        return output
    }
    private fun watermarkMask(colors: IntArray): BooleanArray = BooleanArray(colors.size) { index ->
        val color = colors[index]
        val red = android.graphics.Color.red(color); val green = android.graphics.Color.green(color); val blue = android.graphics.Color.blue(color)
        val distance = (red - WATERMARK_RED) * (red - WATERMARK_RED) + (green - WATERMARK_GREEN) * (green - WATERMARK_GREEN) + (blue - WATERMARK_BLUE) * (blue - WATERMARK_BLUE)
        distance <= WATERMARK_COLOR_TOLERANCE * WATERMARK_COLOR_TOLERANCE && red + green + blue >= WATERMARK_MIN_BRIGHTNESS * 3 && red >= green + 8 && green >= blue + 2
    }
    private fun expandMask(mask: BooleanArray, width: Int, height: Int): BooleanArray = BooleanArray(mask.size) { index ->
        val y = index / width; val x = index % width
        (-1..1).any { oy -> (-1..1).any { ox -> mask[(y + oy).coerceIn(0, height - 1) * width + (x + ox).coerceIn(0, width - 1)] } }
    }
    private fun removeWatermark(colors: IntArray, mask: BooleanArray): IntArray = colors.copyOf().also { output ->
        for (index in output.indices) {
            if (!mask[index]) continue
            val color = output[index]
            val red = android.graphics.Color.red(color); val green = android.graphics.Color.green(color); val blue = android.graphics.Color.blue(color)
            output[index] = if ((red + green + blue) / 3 >= WATERMARK_LIGHT_BRIGHTNESS) android.graphics.Color.WHITE else {
                val gray = (red * 299 + green * 587 + blue * 114 + 500) / 1000
                android.graphics.Color.rgb(gray, gray, gray)
            }
        }
    }
    private fun hasRecognizableContent(pixels: IntArray): Boolean = pixels.any { it < WHITE_CONTENT_THRESHOLD }
    private fun fixedHeightBounds(height: Int): List<Pair<Int, Int>> {
        require(FIXED_TOP_PAD < height) { "固定高度分行的顶部留白覆盖整张图片" }
        val bounds = ArrayList<Pair<Int, Int>>(); var start = FIXED_TOP_PAD
        while (start < height) { val end = minOf(start + FIXED_BLOCK_HEIGHT, height); bounds += start to end; start = end }
        return bounds
    }
    private fun coverPinyin(pixels: IntArray, width: Int, bounds: List<Pair<Int, Int>>): IntArray = pixels.copyOf().also { output ->
        for ((start, end) in bounds) {
            val coverEnd = start + kotlin.math.round((end - start) * PINYIN_TOP_COVER_RATIO).toInt()
            for (y in start until coverEnd) java.util.Arrays.fill(output, y * width, (y + 1) * width, 255)
        }
    }
    private fun saveGray(image: GrayImage, file: File) {
        runCatching {
            file.parentFile?.mkdirs()
            image.toBitmap().useBitmap { bitmap ->
                FileOutputStream(file).use { output -> bitmap.compress(Bitmap.CompressFormat.PNG, 100, output) }
            }
        }
    }
    private fun saveRgb(colors: IntArray, width: Int, height: Int, file: File) { saveBitmap(Bitmap.createBitmap(colors, width, height, Bitmap.Config.ARGB_8888), file) }
    private fun saveMask(mask: BooleanArray, width: Int, height: Int, file: File) { saveBitmap(Bitmap.createBitmap(IntArray(mask.size) { if (mask[it]) android.graphics.Color.WHITE else android.graphics.Color.BLACK }, width, height, Bitmap.Config.ARGB_8888), file) }
    private fun saveBitmap(bitmap: Bitmap, file: File) { runCatching { file.parentFile?.mkdirs(); FileOutputStream(file).use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) } }; bitmap.recycle() }
    private fun saveText(text: String, file: File) { runCatching { file.parentFile?.mkdirs(); file.writeText(text, Charsets.UTF_8) } }
    private fun frameFileName(frame: Int, suffix: String): String = "frame-${frame.toString().padStart(4, '0')}-$suffix"

    private class OnnxChapterRecognizer(private val activity: Activity) : AutoCloseable {
        private val environment = OrtEnvironment.getEnvironment()
        private val sessionOptions = OrtSession.SessionOptions().apply { setIntraOpNumThreads(1); setInterOpNumThreads(1) }
        private val modelFile = materializeModel()
        private val session = environment.createSession(modelFile.absolutePath, sessionOptions)
        private val inputName = session.inputNames.single()
        private val characters = session.metadata.customMetadata["character"]?.lines()?.takeIf { it.isNotEmpty() } ?: throw IllegalStateException("OCR 识别模型未包含字符表")
        fun recognize(line: GrayImage): String {
            val maxWidth = maxOf(RECOGNITION_DEFAULT_WIDTH, (RECOGNITION_HEIGHT * line.width.toFloat() / line.height).toInt())
            val source = line.toBitmap(); val resized = Bitmap.createScaledBitmap(source, maxWidth, RECOGNITION_HEIGHT, true); source.recycle()
            val colors = IntArray(maxWidth * RECOGNITION_HEIGHT)
            try {
                resized.getPixels(colors, 0, maxWidth, 0, 0, maxWidth, RECOGNITION_HEIGHT)
                val input = FloatArray(3 * RECOGNITION_HEIGHT * maxWidth)
                for (y in 0 until RECOGNITION_HEIGHT) for (x in 0 until maxWidth) {
                    val normalized = android.graphics.Color.red(colors[y * maxWidth + x]) / 127.5f - 1f; val offset = y * maxWidth + x
                    input[offset] = normalized; input[RECOGNITION_HEIGHT * maxWidth + offset] = normalized; input[2 * RECOGNITION_HEIGHT * maxWidth + offset] = normalized
                }
                OnnxTensor.createTensor(environment, FloatBuffer.wrap(input), longArrayOf(1, 3, RECOGNITION_HEIGHT.toLong(), maxWidth.toLong())).use { tensor ->
                    session.run(mapOf(inputName to tensor)).use { result ->
                        return decodeCtc((result[0].value as Array<Array<FloatArray>>).single())
                    }
                }
            } finally { resized.recycle() }
        }
        private fun decodeCtc(predictions: Array<FloatArray>): String {
            val text = StringBuilder(); var previous = -1
            for (scores in predictions) {
                var token = 0; var best = Float.NEGATIVE_INFINITY
                for (index in scores.indices) if (scores[index] > best) { best = scores[index]; token = index }
                if (token != 0 && token != previous) when { token <= characters.size -> text.append(characters[token - 1]); token == characters.size + 1 -> text.append(' '); else -> throw IllegalStateException("OCR 模型输出了未知字符索引：$token") }
                previous = token
            }
            return text.toString()
        }
        private fun materializeModel(): File {
            val destination = File(activity.filesDir, "ocr-models/$RECOGNITION_MODEL_FILE")
            if (destination.length() == RECOGNITION_MODEL_BYTES) return destination
            destination.parentFile?.mkdirs()
            activity.assets.open("ocr-models/$RECOGNITION_MODEL_FILE").use { input -> FileOutputStream(destination).use { output -> input.copyTo(output) } }
            require(destination.length() == RECOGNITION_MODEL_BYTES) { "OCR 识别模型复制不完整" }
            return destination
        }
        override fun close() { session.close(); sessionOptions.close() }
    }

    private companion object {
        const val FIXED_BLOCK_HEIGHT = 38
        const val FIXED_TOP_PAD = 5
        const val PINYIN_TOP_COVER_RATIO = 0.39f
        const val WHITE_CONTENT_THRESHOLD = 250
        const val WATERMARK_RED = 255
        const val WATERMARK_GREEN = 213
        const val WATERMARK_BLUE = 204
        const val WATERMARK_COLOR_TOLERANCE = 25
        const val WATERMARK_MIN_BRIGHTNESS = 150
        const val WATERMARK_LIGHT_BRIGHTNESS = 185
        const val MAX_OCR_SOURCE_BYTES = 32L * 1024L * 1024L
        const val RECOGNITION_HEIGHT = 48
        const val RECOGNITION_DEFAULT_WIDTH = 320
        const val RECOGNITION_MODEL_FILE = "PP-OCRv6_rec_small.onnx"
        const val RECOGNITION_MODEL_BYTES = 21_234_383L
    }
}

private inline fun Bitmap.useBitmap(block: (Bitmap) -> Unit) { try { block(this) } finally { recycle() } }
