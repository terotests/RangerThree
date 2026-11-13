package rangerthree.gpu

import rangerthree.core.GpuRenderer
import rangerthree.core.GpuSettings
import rangerthree.core.SoftRenderer
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * The whole Android path on a desktop JVM: the three crate as Ranger writes
 * it in Kotlin builds a scene, its GpuRenderer turns each frame into GPU
 * commands, and GpuPlayer draws them on headless OpenGL ES 3.0. The last
 * frame is checked against the crate's software renderer, as gpu-gl's
 * scenes example does for Rust.
 *
 *   bash gpu-android/test.sh
 */
fun main(args: Array<String>) {
    val w = 320
    val h = 240
    val frames = 20
    val ctx = Headless(w, h)
    val gpu = GpuRenderer.new_()
    val player = GpuPlayer(ctx.gl)
    player.screenFb = ctx.fb
    println("Kotlin crate + Kotlin player | " + player.describe())
    var failed = false
    for ((name, build) in demoScenes) {
        val b = build(w.toDouble() / h)
        val s = GpuSettings.new_(w.toLong(), h.toLong())
        var plan = 0.0
        var play = 0.0
        var px = ByteBuffer.allocateDirect(w * h * 4).order(ByteOrder.nativeOrder())
        var calls = 0L
        for (i in 0 until frames) {
            b.animate(1000.0 + i * 16.0)
            val t0 = System.nanoTime()
            gpu.upload_images(b.scene)
            val frame = gpu.render(b.scene, b.camera, s)
            val t1 = System.nanoTime()
            if (frame.missing.isNotEmpty()) throw IllegalStateException("$name needs " + frame.missing.joinToString())
            val c = frame.commands
            player.run(c.ints.toLongArray(), FloatArray(c.floats.size) { c.floats[it].toFloat() }, c.texts.toTypedArray(), c.bytes.copyOf(c.bytes.size))
            px.clear()
            ctx.gl.bindFramebuffer(GL.FRAMEBUFFER, ctx.fb)
            ctx.gl.readPixels(0, 0, w, h, GL.RGBA, GL.UNSIGNED_BYTE, px)
            val t2 = System.nanoTime()
            if (i >= frames / 2) { plan += (t1 - t0) / 1e6; play += (t2 - t1) / 1e6 }
            calls = frame.calls
        }
        val soft = SoftRenderer.new_(w.toLong(), h.toLong())
        soft.render(b.scene, b.camera)
        var sum = 0.0
        var drawn = 0
        val bg = intArrayOf(px.get(0).toInt() and 255, px.get(1).toInt() and 255, px.get(2).toInt() and 255)
        for (y in 0 until h) for (x in 0 until w) {
            val g = ((h - 1 - y) * w + x) * 4
            val si = (y * w + x) * 4
            var other = false
            for (k in 0 until 3) {
                val v = px.get(g + k).toInt() and 255
                sum += Math.abs(v - soft.pixels[si + k])
                if (v != bg[k]) other = true
            }
            if (other) drawn++
        }
        val diff = sum / (w * h * 3)
        println("%-11s %d calls | crate %.2f ms + player/GPU %.2f ms per frame | %.1f %% of pixels drawn | software renderer differs by %.2f".format(
            name, calls, plan / (frames / 2), play / (frames / 2), drawn * 100.0 / (w * h), diff))
        if (drawn == 0 || diff > 4.0) failed = true
    }
    if (failed) System.exit(1)
}
