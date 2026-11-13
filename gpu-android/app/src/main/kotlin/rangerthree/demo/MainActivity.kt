package rangerthree.demo

import android.app.Activity
import android.opengl.GLSurfaceView
import android.os.Bundle
import android.os.SystemClock
import android.util.Log
import android.view.MotionEvent
import rangerthree.core.GpuRenderer
import rangerthree.core.GpuSettings
import rangerthree.gpu.AndroidGl
import rangerthree.gpu.DemoScene
import rangerthree.gpu.GpuPlayer
import rangerthree.gpu.demoScenes
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10

/**
 * The three crate drawing on the phone's GPU: each frame the crate's
 * GpuRenderer (Ranger's Kotlin output of three/src) turns the scene into GPU
 * commands, and GpuPlayer plays them on GLES 3.0. A tap shows the next scene.
 * Timings go to logcat under the tag RangerThree.
 */
class MainActivity : Activity() {
    private lateinit var view: GLSurfaceView
    private val renderer = SceneRenderer()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        view = GLSurfaceView(this)
        view.setEGLContextClientVersion(3)
        view.setEGLConfigChooser(8, 8, 8, 8, 24, 8)
        view.setRenderer(renderer)
        view.setOnTouchListener { _, e ->
            if (e.action == MotionEvent.ACTION_UP) view.queueEvent { renderer.next() }
            true
        }
        setContentView(view)
    }

    override fun onPause() {
        super.onPause()
        view.onPause()
    }

    override fun onResume() {
        super.onResume()
        view.onResume()
    }
}

private class SceneRenderer : GLSurfaceView.Renderer {
    private var player: GpuPlayer? = null
    private var gpu: GpuRenderer? = null
    private var width = 1
    private var height = 1
    private var index = 0
    private var scene: DemoScene? = null
    private var frames = 0
    private var crateMs = 0.0
    private var playMs = 0.0

    override fun onSurfaceCreated(unused: GL10?, config: EGLConfig?) {
        // a new EGL context: every GL object of the old one is gone, and the
        // renderer's ids belong to the player they were sent to
        player = GpuPlayer(AndroidGl())
        gpu = GpuRenderer.new_()
        scene = null
        Log.i(TAG, "GLES " + player!!.describe())
    }

    override fun onSurfaceChanged(unused: GL10?, w: Int, h: Int) {
        width = w
        height = h
        scene = null
    }

    fun next() {
        index = (index + 1) % demoScenes.size
        scene = null
    }

    override fun onDrawFrame(unused: GL10?) {
        val p = player ?: return
        val g = gpu ?: return
        val s = scene ?: demoScenes[index].second(width.toDouble() / height).also { scene = it }
        s.animate(SystemClock.uptimeMillis().toDouble())
        val t0 = System.nanoTime()
        g.upload_images(s.scene)
        val frame = g.render(s.scene, s.camera, GpuSettings.new_(width.toLong(), height.toLong()))
        val c = frame.commands
        val t1 = System.nanoTime()
        if (frame.missing.isNotEmpty()) Log.w(TAG, "the scene needs " + frame.missing.joinToString())
        p.run(c.ints.toLongArray(), FloatArray(c.floats.size) { c.floats[it].toFloat() }, c.texts.toTypedArray(), c.bytes)
        val t2 = System.nanoTime()
        crateMs += (t1 - t0) / 1e6
        playMs += (t2 - t1) / 1e6
        if (++frames == 120) {
            Log.i(TAG, "%s %dx%d: crate %.2f ms + player %.2f ms per frame, %d draw calls".format(
                demoScenes[index].first, width, height, crateMs / frames, playMs / frames, frame.calls))
            frames = 0
            crateMs = 0.0
            playMs = 0.0
        }
    }

    companion object {
        const val TAG = "RangerThree"
    }
}
