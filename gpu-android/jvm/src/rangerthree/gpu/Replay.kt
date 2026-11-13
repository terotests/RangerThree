package rangerthree.gpu

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Plays the command streams tests/gpu-compare.mjs --dump wrote with the
 * Kotlin player on a headless OpenGL ES 3.0 context (Mesa) and compares the
 * pixels with what the same stream drew in Chromium's WebGL2, as gpu-gl's
 * replay example does for the Rust player.
 *
 *   node tests/gpu-compare.mjs --dump=tmp/gpu-dump
 *   bash gpu-android/test.sh tmp/gpu-dump
 */
private class Json(private val s: String) {
    var i = 0
    private fun ws() { while (i < s.length && s[i].isWhitespace()) i++ }
    fun value(): Any? {
        ws()
        return when (s[i]) {
            '{' -> {
                i++
                val m = HashMap<String, Any?>()
                while (true) {
                    ws()
                    if (s[i] == '}') { i++; return m }
                    val k = value() as String
                    ws(); i++ // ':'
                    m[k] = value()
                    ws()
                    if (s[i] == ',') i++
                }
                @Suppress("UNREACHABLE_CODE") m
            }
            '[' -> {
                i++
                val a = ArrayList<Any?>()
                while (true) {
                    ws()
                    if (s[i] == ']') { i++; return a }
                    a.add(value())
                    ws()
                    if (s[i] == ',') i++
                }
                @Suppress("UNREACHABLE_CODE") a
            }
            '"' -> {
                i++
                val b = StringBuilder()
                while (s[i] != '"') {
                    if (s[i] == '\\') {
                        i++
                        when (val c = s[i++]) {
                            'n' -> b.append('\n'); 't' -> b.append('\t'); 'r' -> b.append('\r')
                            'b' -> b.append('\b'); 'f' -> b.append('\u000c')
                            'u' -> { b.append(s.substring(i, i + 4).toInt(16).toChar()); i += 4 }
                            else -> b.append(c)
                        }
                    } else b.append(s[i++])
                }
                i++
                b.toString()
            }
            't', 'f', 'n' -> { while (i < s.length && s[i].isLetter()) i++; null }
            else -> {
                val st = i
                while (i < s.length && (s[i].isDigit() || s[i] in "-+.eE")) i++
                s.substring(st, i).toDouble()
            }
        }
    }
}

fun main(args: Array<String>) {
    val dir = File(args.firstOrNull { !it.startsWith("--") } ?: "tmp/gpu-dump")
    val files = (dir.listFiles { f -> f.name.endsWith(".json") } ?: emptyArray()).sortedBy { it.name }
    var ok = 0
    var total = 0
    val contexts = HashMap<Pair<Int, Int>, Headless>()
    for (file in files) {
        @Suppress("UNCHECKED_CAST")
        val j = Json(file.readText()).value() as Map<String, Any?>
        val name = j["name"] as String
        val w = (j["w"] as Double).toInt()
        val h = (j["h"] as Double).toInt()
        val ints = (j["ints"] as List<*>).map { (it as Double).toLong() }.toLongArray()
        val floats = (j["floats"] as List<*>).map { (it as Double).toFloat() }.toFloatArray()
        val texts = (j["texts"] as List<*>).map { it as String }.toTypedArray()
        val bytes = ((j["bytes"] as List<*>?) ?: emptyList<Any>()).map { (it as Double).toInt().toByte() }.toByteArray()
        val want = File(file.path.removeSuffix(".json") + ".rgba").readBytes()
        val ctx = contexts.getOrPut(w to h) { Headless(w, h) }
        val player = GpuPlayer(ctx.gl)
        player.screenFb = ctx.fb
        if (total == 0) println("OpenGL ES 3 (Kotlin player): " + player.describe())
        total++
        val t0 = System.nanoTime()
        try {
            player.run(ints, floats, texts, bytes)
        } catch (e: Exception) {
            println("FAIL  $name: ${e.message}")
            player.dispose()
            continue
        }
        val out = ByteBuffer.allocateDirect(w * h * 4).order(ByteOrder.nativeOrder())
        ctx.gl.bindFramebuffer(GL.FRAMEBUFFER, ctx.fb)
        ctx.gl.readPixels(0, 0, w, h, GL.RGBA, GL.UNSIGNED_BYTE, out)
        val ms = (System.nanoTime() - t0) / 1e6
        var sum = 0L
        var max = 0
        var far = 0
        for (k in 0 until w * h * 4) {
            val d = Math.abs((out.get(k).toInt() and 255) - (want[k].toInt() and 255))
            sum += d
            if (d > max) max = d
            if (d > 3) far++
        }
        val n = w * h * 4
        val mean = sum.toDouble() / n
        val farp = far.toDouble() / n
        val pass = mean <= 0.5 && farp <= 0.001
        if (pass) ok++
        println("%s  %s: mean %.3f max %d far %.2f %% | first frame %.1f ms".format(if (pass) "PASS" else "DIFF", name, mean, max, farp * 100, ms))
        player.dispose()
    }
    println("$ok/$total Kotlin player frames match WebGL2")
}
