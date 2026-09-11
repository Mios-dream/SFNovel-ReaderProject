package com.sansan.sf_novel_flow

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Movie
import android.util.Log
import ai.onnxruntime.OnnxTensor
import ai.onnxruntime.OrtEnvironment
import ai.onnxruntime.OrtSession
import ai.onnxruntime.TensorInfo
import java.io.File
import java.io.FileOutputStream
import java.io.BufferedInputStream
import java.io.InputStream
import java.io.OutputStreamWriter
import java.io.PrintWriter
import java.nio.FloatBuffer
import java.nio.charset.StandardCharsets
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
    var segmentsDir: String? = null
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
            val progress = OcrProgress()
            var segments: File? = null
            try {
                progress.stage = "检查输入文件"
                val source = File(args.sourcePath)
                require(source.isFile) { "OCR 输入图片不存在：${source.absolutePath}" }
                val segmentDirectory = args.segmentsDir
                    ?.takeIf { it.isNotBlank() }
                    ?.let(::prepareSegmentsDirectory)
                segments = segmentDirectory
                progress.stage = "加载 OCR 模型"
                val text = OnnxChapterRecognizer(activity).use { recognizer ->
                    recognizeFrames(source, segmentDirectory, recognizer, progress)
                }
                if (text.isBlank()) throw IllegalStateException("未识别到可用文字")
                invoke.resolve(JSObject().put("text", text))
            } catch (error: Throwable) {
                val report = writeFailureReport(segments, progress.stage, error)
                Log.e(LOG_TAG, "OCR failed at ${progress.stage}", error)
                invoke.reject(formatFailure(progress.stage, error, report))
            }
        }.start()
    }

    private fun prepareSegmentsDirectory(path: String): File = File(path).also { directory ->
        require(directory.isDirectory || directory.mkdirs()) { "无法创建 OCR 诊断目录：${directory.absolutePath}" }
        require(directory.isDirectory) { "OCR 诊断目录不是文件夹：${directory.absolutePath}" }
    }

    private fun recognizeFrames(source: File, segments: File?, recognizer: OnnxChapterRecognizer, progress: OcrProgress): String {
        val frameTexts = ArrayList<String>()
        if (source.extension.lowercase(Locale.ROOT) != "gif") {
            progress.stage = "解码输入图片"
            val bitmap = decodeMutableBitmap(source)
            try { frameTexts += recognizeBitmap(bitmap, segments, 1, recognizer, progress) }
            finally { bitmap.recycle() }
        } else {
            val movie = Movie.decodeFile(source.absolutePath)
            if (movie != null && movie.duration() <= 0) {
                progress.stage = "解码 GIF 图片"
                val bitmap = decodeMutableBitmap(source)
                try { frameTexts += recognizeBitmap(bitmap, segments, 1, recognizer, progress) }
                finally { bitmap.recycle() }
            } else {
                val gif = GifDrawable(source)
                try {
                    require(gif.numberOfFrames > 0) { "GIF 中不包含可识别帧" }
                    for (index in 0 until gif.numberOfFrames) {
                        progress.stage = "解码 GIF 第 ${index + 1}/${gif.numberOfFrames} 帧"
                        gif.seekToFrame(index)
                        val frame = Bitmap.createBitmap(gif.intrinsicWidth, gif.intrinsicHeight, Bitmap.Config.ARGB_8888)
                        val canvas = android.graphics.Canvas(frame)
                        canvas.drawColor(android.graphics.Color.WHITE)
                        gif.setBounds(0, 0, frame.width, frame.height)
                        gif.draw(canvas)
                        try { frameTexts += recognizeBitmap(frame, segments, index + 1, recognizer, progress) }
                        finally { frame.recycle() }
                    }
                } finally { gif.recycle() }
            }
        }
        return frameTexts.filter { it.isNotBlank() }.joinToString("\n\n").also {
            if (segments != null) {
                progress.stage = "保存 OCR 结果"
                saveText(it, File(segments, "ocr-result.txt"))
            }
        }
    }

    private fun decodeMutableBitmap(source: File): Bitmap {
        val options = BitmapFactory.Options().apply {
            inPreferredConfig = Bitmap.Config.ARGB_8888
            inMutable = true
        }
        val decoded = BitmapFactory.decodeFile(source.absolutePath, options)
            ?: throw IllegalArgumentException("无法解码 OCR 输入图片：${source.absolutePath}")
        if (decoded.isMutable && decoded.config == Bitmap.Config.ARGB_8888) return decoded
        return try {
            decoded.copy(Bitmap.Config.ARGB_8888, true)
                ?: throw IllegalStateException("无法创建可写 OCR 图片：${source.absolutePath}")
        } finally {
            decoded.recycle()
        }
    }

    private fun recognizeBitmap(bitmap: Bitmap, segments: File?, frame: Int, recognizer: OnnxChapterRecognizer, progress: OcrProgress): String {
        // Keep only this cleaned page bitmap. Watermark buffers are bounded to a small row chunk.
        progress.stage = "第 $frame 帧去水印"
        cleanWatermarkInPlace(bitmap)
        if (segments != null) {
            progress.stage = "保存第 $frame 帧去水印图"
            saveBitmapCopy(bitmap, File(segments, frameFileName(frame, "watermark-removed.png")))
        }
        return recognizeCleanedBitmap(bitmap, segments, frame, recognizer, progress)
    }

    private data class GrayImage(val width: Int, val height: Int, val pixels: IntArray) {
        fun toBitmap(): Bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888).also { output ->
            val colors = IntArray(pixels.size) { index -> android.graphics.Color.rgb(pixels[index], pixels[index], pixels[index]) }
            output.setPixels(colors, 0, width, 0, 0, width, height)
        }
    }

    private fun cleanWatermarkInPlace(bitmap: Bitmap) {
        val width = bitmap.width
        var previousRawBottomRow: IntArray? = null
        for (top in 0 until bitmap.height step WATERMARK_PROCESS_ROWS) {
            val bottom = minOf(top + WATERMARK_PROCESS_ROWS, bitmap.height)
            // Include a one-pixel halo so blur and mask expansion match neighbouring chunks.
            val sampleTop = maxOf(0, top - WATERMARK_MASK_EXPANSION)
            val sampleBottom = minOf(bitmap.height, bottom + WATERMARK_MASK_EXPANSION)
            val sampleHeight = sampleBottom - sampleTop
            val colors = IntArray(width * sampleHeight)
            bitmap.getPixels(colors, 0, width, 0, sampleTop, width, sampleHeight)
            flattenOnWhiteInPlace(colors)
            // The halo above this chunk must be the source row, not the row cleaned by the prior chunk.
            previousRawBottomRow?.copyInto(colors, 0, 0, width)
            previousRawBottomRow = colors.copyOfRange(
                (bottom - sampleTop - 1) * width,
                (bottom - sampleTop) * width,
            )
            val denoised = lightlyBlur(colors, width, sampleHeight)
            val mask = expandMask(watermarkMask(denoised), width, sampleHeight)
            removeWatermarkInPlace(colors, mask, width, top - sampleTop, bottom - sampleTop)
            val coreOffset = (top - sampleTop) * width
            bitmap.setPixels(colors, coreOffset, width, 0, top, width, bottom - top)
        }
    }

    private fun flattenOnWhiteInPlace(colors: IntArray) {
        for (index in colors.indices) {
            val color = colors[index]
            val alpha = android.graphics.Color.alpha(color)
            if (alpha == 255) continue
            val inverseAlpha = 255 - alpha
            val red = (android.graphics.Color.red(color) * alpha + 255 * inverseAlpha + 127) / 255
            val green = (android.graphics.Color.green(color) * alpha + 255 * inverseAlpha + 127) / 255
            val blue = (android.graphics.Color.blue(color) * alpha + 255 * inverseAlpha + 127) / 255
            colors[index] = android.graphics.Color.rgb(red, green, blue)
        }
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
    private fun removeWatermarkInPlace(colors: IntArray, mask: BooleanArray, width: Int, startRow: Int, endRow: Int) {
        for (y in startRow until endRow) for (x in 0 until width) {
            val index = y * width + x
            if (!mask[index]) continue
            val color = colors[index]
            val red = android.graphics.Color.red(color); val green = android.graphics.Color.green(color); val blue = android.graphics.Color.blue(color)
            colors[index] = if ((red + green + blue) / 3 >= WATERMARK_LIGHT_BRIGHTNESS) android.graphics.Color.WHITE else {
                val gray = (red * 299 + green * 587 + blue * 114 + 500) / 1000
                android.graphics.Color.rgb(gray, gray, gray)
            }
        }
    }

    private fun recognizeCleanedBitmap(bitmap: Bitmap, segments: File?, frame: Int, recognizer: OnnxChapterRecognizer, progress: OcrProgress): String {
        val texts = ArrayList<String>()
        var contentLine = 0
        for ((start, end) in fixedHeightBounds(bitmap.height)) {
            val lineNumber = contentLine + 1
            var lineHasContent = false
            var partNumber = 0
            for (left in 0 until bitmap.width step OCR_PART_WIDTH) {
                val partWidth = minOf(OCR_PART_WIDTH, bitmap.width - left)
                val pixels = IntArray(partWidth * (end - start))
                bitmap.getPixels(pixels, 0, partWidth, left, start, partWidth, end - start)
                toGrayInPlace(pixels)
                val line = GrayImage(partWidth, end - start, pixels)
                coverPinyinInPlace(line.pixels, line.width, line.height)
                if (!hasRecognizableContent(line.pixels)) continue
                lineHasContent = true
                partNumber++
                val prefix = "frame-${frame.toString().padStart(4, '0')}-line-${lineNumber.toString().padStart(4, '0')}-part-${partNumber.toString().padStart(4, '0')}"
                if (segments != null) {
                    progress.stage = "保存第 $frame 帧第 $lineNumber 行第 $partNumber 段"
                    saveGray(line, File(segments, "$prefix.png"))
                }
                progress.stage = "识别第 $frame 帧第 $lineNumber 行第 $partNumber 段"
                val value = recognizer.recognize(line)
                if (segments != null) {
                    progress.stage = "保存第 $frame 帧第 $lineNumber 行第 $partNumber 段识别结果"
                    saveText(value, File(segments, "$prefix-ocr.txt"))
                }
                if (value.isNotEmpty()) texts += value
            }
            if (lineHasContent) contentLine++
        }
        return texts.joinToString("\n")
    }

    private fun toGrayInPlace(colors: IntArray) {
        for (index in colors.indices) {
            val color = colors[index]
            colors[index] = (android.graphics.Color.red(color) * 299 + android.graphics.Color.green(color) * 587 + android.graphics.Color.blue(color) * 114) / 1000
        }
    }
    private fun hasRecognizableContent(pixels: IntArray): Boolean = pixels.any { it < WHITE_CONTENT_THRESHOLD }
    private fun fixedHeightBounds(height: Int): List<Pair<Int, Int>> {
        require(FIXED_TOP_PAD < height) { "固定高度分行的顶部留白覆盖整张图片" }
        val bounds = ArrayList<Pair<Int, Int>>(); var start = FIXED_TOP_PAD
        while (start < height) { val end = minOf(start + FIXED_BLOCK_HEIGHT, height); bounds += start to end; start = end }
        return bounds
    }
    private fun coverPinyinInPlace(pixels: IntArray, width: Int, height: Int) {
        val coverEnd = kotlin.math.round(height * PINYIN_TOP_COVER_RATIO).toInt()
        for (y in 0 until coverEnd) java.util.Arrays.fill(pixels, y * width, (y + 1) * width, 255)
    }
    private fun saveGray(image: GrayImage, file: File) {
        file.parentFile?.let { parent -> require(parent.isDirectory || parent.mkdirs()) { "无法创建诊断目录：${parent.absolutePath}" } }
        image.toBitmap().useBitmap { bitmap ->
            FileOutputStream(file).use { output ->
                require(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) { "无法保存诊断图片：${file.absolutePath}" }
            }
        }
    }
    private fun saveBitmapCopy(bitmap: Bitmap, file: File) {
        file.parentFile?.let { parent -> require(parent.isDirectory || parent.mkdirs()) { "无法创建诊断目录：${parent.absolutePath}" } }
        FileOutputStream(file).use { output ->
            require(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) { "无法保存诊断图片：${file.absolutePath}" }
        }
    }
    private fun saveText(text: String, file: File) {
        file.parentFile?.let { parent -> require(parent.isDirectory || parent.mkdirs()) { "无法创建诊断目录：${parent.absolutePath}" } }
        file.writeText(text, Charsets.UTF_8)
    }
    private fun frameFileName(frame: Int, suffix: String): String = "frame-${frame.toString().padStart(4, '0')}-$suffix"

    private fun writeFailureReport(segments: File?, stage: String, error: Throwable): String? {
        val directory = segments ?: return null
        return runCatching {
            val report = File(directory, "ocr-failure.txt")
            FileOutputStream(report).use { output ->
                PrintWriter(OutputStreamWriter(output, StandardCharsets.UTF_8)).use { writer ->
                    writer.println("stage: $stage")
                    writer.println("error: ${error.javaClass.name}")
                    writer.println("message: ${error.message ?: "(no message)"}")
                    writer.println()
                    error.printStackTrace(writer)
                }
            }
            report.absolutePath
        }.getOrNull()
    }

    private fun formatFailure(stage: String, error: Throwable, report: String?): String {
        if (error is OutOfMemoryError) {
            return "Android OCR 内存不足（阶段：$stage）。${report?.let { "诊断报告：$it" } ?: "未能写入诊断报告"}"
        }
        val causes = generateSequence(error) { it.cause }
            .take(4)
            .joinToString(" <- ") { cause -> "${cause.javaClass.name}: ${cause.message ?: "(无消息)"}" }
        return "Android OCR 失败（阶段：$stage；异常：$causes）。${report?.let { "诊断报告：$it" } ?: "未能写入诊断报告"}"
    }

    private class OcrProgress(var stage: String = "初始化")

    private class OnnxChapterRecognizer(private val activity: Activity) : AutoCloseable {
        private val environment = OrtEnvironment.getEnvironment()
        private val sessionOptions = OrtSession.SessionOptions().apply { setIntraOpNumThreads(1); setInterOpNumThreads(1) }
        private val modelFile = materializeModel()
        private val characters = loadCharacterTable(modelFile)
        private val session = environment.createSession(modelFile.absolutePath, sessionOptions)
        private val inputName = session.inputNames.single()

        /**
         * Android ONNX Runtime 1.22 can abort in JNI while constructing model metadata.
         * Read the ONNX ModelProto metadata entry directly instead of calling session.metadata.
         */
        private fun loadCharacterTable(file: File): List<String> {
            val characterText = BufferedInputStream(file.inputStream()).use { input ->
                while (true) {
                    val tag = readProtoTag(input) ?: break
                    val field = (tag ushr 3).toInt()
                    val wireType = (tag and 0x07).toInt()
                    if (field == ONNX_METADATA_PROPERTIES_FIELD && wireType == PROTO_LENGTH_DELIMITED) {
                        parseMetadataEntry(readProtoBytes(input))?.let { return@use it }
                    } else {
                        skipProtoField(input, wireType)
                    }
                }
                null
            }
            return characterText
                ?.lineSequence()
                ?.filter { it.isNotEmpty() }
                ?.toList()
                ?.takeIf { it.isNotEmpty() }
                ?: throw IllegalStateException("OCR 识别模型未包含字符表")
        }

        private fun parseMetadataEntry(entry: ByteArray): String? {
            var key: String? = null
            var value: String? = null
            entry.inputStream().use { input ->
                while (true) {
                    val tag = readProtoTag(input) ?: break
                    val field = (tag ushr 3).toInt()
                    val wireType = (tag and 0x07).toInt()
                    if (wireType != PROTO_LENGTH_DELIMITED) {
                        skipProtoField(input, wireType)
                    } else when (field) {
                        1 -> key = String(readProtoBytes(input), StandardCharsets.UTF_8)
                        2 -> value = String(readProtoBytes(input), StandardCharsets.UTF_8)
                        else -> skipProtoField(input, wireType)
                    }
                }
            }
            return value?.takeIf { key == "character" }
        }

        private fun readProtoTag(input: InputStream): Long? {
            val firstByte = input.read()
            return if (firstByte == -1) null else readProtoVarint(input, firstByte)
        }

        private fun readProtoVarint(input: InputStream, firstByte: Int? = null): Long {
            var value = 0L
            var shift = 0
            var nextByte = firstByte ?: input.read().also { require(it != -1) { "OCR 模型文件意外结束" } }
            while (true) {
                value = value or ((nextByte and 0x7f).toLong() shl shift)
                if (nextByte and 0x80 == 0) return value
                shift += 7
                require(shift < 64) { "OCR 模型包含无效的 protobuf 变长整数" }
                nextByte = input.read()
                require(nextByte != -1) { "OCR 模型文件意外结束" }
            }
        }

        private fun readProtoBytes(input: InputStream): ByteArray {
            val length = readProtoVarint(input)
            require(length in 0..MAX_PROTO_FIELD_BYTES.toLong()) { "OCR 模型 protobuf 字段过大：$length" }
            return ByteArray(length.toInt()).also { bytes ->
                var offset = 0
                while (offset < bytes.size) {
                    val count = input.read(bytes, offset, bytes.size - offset)
                    require(count > 0) { "OCR 模型文件意外结束" }
                    offset += count
                }
            }
        }

        private fun skipProtoField(input: InputStream, wireType: Int) {
            when (wireType) {
                0 -> readProtoVarint(input)
                1 -> skipProtoBytes(input, 8)
                2 -> skipProtoBytes(input, readProtoVarint(input))
                5 -> skipProtoBytes(input, 4)
                else -> throw IllegalArgumentException("OCR 模型包含不支持的 protobuf 字段类型：$wireType")
            }
        }

        private fun skipProtoBytes(input: InputStream, byteCount: Long) {
            require(byteCount >= 0) { "OCR 模型 protobuf 字段长度无效：$byteCount" }
            var remaining = byteCount
            while (remaining > 0) {
                val skipped = input.skip(remaining)
                if (skipped > 0) {
                    remaining -= skipped
                } else {
                    require(input.read() != -1) { "OCR 模型文件意外结束" }
                    remaining--
                }
            }
        }
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
                        return decodeCtc(result[0] as OnnxTensor)
                    }
                }
            } finally { resized.recycle() }
        }
        private fun decodeCtc(output: OnnxTensor): String {
            val shape = (output.info as TensorInfo).shape
            require(shape.size == 3 && shape[0] == 1L) { "OCR 模型输出形状无效：${shape.contentToString()}" }
            val timeSteps = shape[1].toInt()
            val classCount = shape[2].toInt()
            val scores = output.floatBuffer
            val text = StringBuilder(); var previous = -1
            for (step in 0 until timeSteps) {
                var token = 0; var best = Float.NEGATIVE_INFINITY
                for (index in 0 until classCount) {
                    val score = scores.get()
                    if (score > best) { best = score; token = index }
                }
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
        const val LOG_TAG = "SfacgOcr"
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
        // A bounded working block, not a source-image or line-count limit.
        const val WATERMARK_PROCESS_ROWS = 128
        const val WATERMARK_MASK_EXPANSION = 1
        // Every fixed-height line is traversed from left to right in 728-pixel parts.
        const val OCR_PART_WIDTH = 728
        const val RECOGNITION_HEIGHT = 48
        const val RECOGNITION_DEFAULT_WIDTH = 320
        const val RECOGNITION_MODEL_FILE = "PP-OCRv6_rec_small.onnx"
        const val RECOGNITION_MODEL_BYTES = 21_234_383L
        const val ONNX_METADATA_PROPERTIES_FIELD = 14
        const val PROTO_LENGTH_DELIMITED = 2
        // The model's metadata entries are small; this protects the parser from corrupt model files.
        const val MAX_PROTO_FIELD_BYTES = 1 * 1024 * 1024
    }
}

private inline fun Bitmap.useBitmap(block: (Bitmap) -> Unit) { try { block(this) } finally { recycle() } }
