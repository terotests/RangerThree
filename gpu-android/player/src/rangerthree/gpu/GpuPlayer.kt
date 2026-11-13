package rangerthree.gpu

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer
import java.nio.IntBuffer

/**
 * Plays the GPU command stream RangerThree's crate writes
 * (three/src/renderers/gpu.rs, frames from renderers/gpu_frame.rs) on
 * OpenGL ES 3.0: the Android counterpart of js/gpu-webgl2.js and gpu-gl.
 * It keeps no frame logic, only the GL objects the stream names by id.
 *
 *   val player = GpuPlayer(AndroidGl())        // on the GL thread
 *   player.run(ints, floats, texts, bytes)     // each frame's commands
 *
 * One player per GpuRenderer: the ids belong to the renderer that wrote them.
 */
class GpuPlayer(val gl: Gl) {
    private sealed class Obj {
        class Buffer(val name: Int, var length: Int) : Obj()
        class Program(val name: Int, val locs: IntArray) : Obj()
        class Texture(val name: Int) : Obj()
        class Target(val fb: Int, val rbs: IntArray) : Obj()
    }

    private val objs = HashMap<Long, Obj>()
    private val vao = gl.genVertexArray()
    /** The framebuffer PASS 0 (the screen) draws into: 0 on a window surface. */
    var screenFb = 0
    var commands = 0L
        private set

    private var floatScratch: FloatBuffer = direct(1024).asFloatBuffer()
    private var intScratch: IntBuffer = direct(1024).asIntBuffer()

    private fun direct(bytes: Int): ByteBuffer = ByteBuffer.allocateDirect(bytes).order(ByteOrder.nativeOrder())

    private fun floatsAt(src: FloatArray, from: Int, n: Int): FloatBuffer {
        if (floatScratch.capacity() < n) floatScratch = direct(n * 4).asFloatBuffer()
        floatScratch.clear()
        floatScratch.put(src, from, n)
        floatScratch.flip()
        return floatScratch
    }

    private fun intsAt(src: LongArray, from: Int, n: Int): IntBuffer {
        if (intScratch.capacity() < n) intScratch = direct(n * 4).asIntBuffer()
        intScratch.clear()
        for (k in 0 until n) intScratch.put(src[from + k].toInt())
        intScratch.flip()
        return intScratch
    }

    private fun buffer(id: Long): Obj.Buffer = objs[id] as? Obj.Buffer ?: throw IllegalStateException("gpu-android: id $id is not a buffer")
    private fun texture(id: Long): Int = (objs[id] as? Obj.Texture ?: throw IllegalStateException("gpu-android: id $id is not a texture")).name
    private fun framebuffer(id: Long): Int =
        if (id == 0L) screenFb else (objs[id] as? Obj.Target ?: throw IllegalStateException("gpu-android: id $id is not a target")).fb

    private fun compile(type: Int, src: String): Int {
        val s = gl.createShader(type)
        gl.shaderSource(s, src)
        gl.compileShader(s)
        if (!gl.shaderCompiled(s)) throw IllegalStateException("gpu-android shader: " + gl.shaderInfoLog(s))
        return s
    }

    private fun delete(o: Obj) {
        when (o) {
            is Obj.Buffer -> gl.deleteBuffer(o.name)
            is Obj.Program -> gl.deleteProgram(o.name)
            is Obj.Texture -> gl.deleteTexture(o.name)
            is Obj.Target -> {
                gl.deleteFramebuffer(o.fb)
                for (rb in o.rbs) gl.deleteRenderbuffer(rb)
            }
        }
    }

    /** Runs one command stream (a GpuCommands' ints, floats, texts and bytes). */
    fun run(ints: LongArray, floats: FloatArray, texts: Array<String>, bytes: ByteArray) {
        var i = 0
        var f = 0
        val n = ints.size
        var prog: Obj.Program? = null
        fun I(k: Int): Int = ints[i + k].toInt()
        gl.bindVertexArray(vao)
        while (i < n) {
            val op = ints[i++].toInt()
            commands++
            when (op) {
                1 -> { // BUFFER id, kind, dynamic, n
                    val id = ints[i]
                    val index = I(1) == 1
                    val dyn = I(2) == 1
                    val cnt = I(3)
                    i += 4
                    val target = if (index) GL.ELEMENT_ARRAY_BUFFER else GL.ARRAY_BUFFER
                    val data: java.nio.Buffer
                    if (index) {
                        data = intsAt(ints, i, cnt)
                        i += cnt
                    } else {
                        data = floatsAt(floats, f, cnt)
                        f += cnt
                    }
                    var e = objs[id] as? Obj.Buffer
                    if (e == null) {
                        e = Obj.Buffer(gl.genBuffer(), -1)
                        objs[id] = e
                    }
                    gl.bindBuffer(target, e.name)
                    if (e.length == cnt) gl.bufferSubData(target, 0, cnt * 4, data)
                    else gl.bufferData(target, cnt * 4, data, if (dyn) GL.DYNAMIC_DRAW else GL.STATIC_DRAW)
                    e.length = cnt
                }
                2 -> { // PROGRAM id, vs, fs, names
                    val id = ints[i]
                    val vs = texts[I(1)]
                    val fs = texts[I(2)]
                    val nn = I(3)
                    i += 4
                    val p = gl.createProgram()
                    val v = compile(GL.VERTEX_SHADER, vs)
                    val fr = compile(GL.FRAGMENT_SHADER, fs)
                    gl.attachShader(p, v)
                    gl.attachShader(p, fr)
                    gl.linkProgram(p)
                    gl.deleteShader(v)
                    gl.deleteShader(fr)
                    if (!gl.programLinked(p)) throw IllegalStateException("gpu-android program: " + gl.programInfoLog(p))
                    val locs = IntArray(nn) { q -> gl.getUniformLocation(p, texts[I(q)]) }
                    i += nn
                    objs[id]?.let { delete(it) }
                    objs[id] = Obj.Program(p, locs)
                }
                3 -> { // TEXTURE id, cube, internal, format, type, w, h, min, mag, wrap s, wrap t, flags, data kind, offset, n
                    val id = ints[i]
                    val cube = I(1) == 1
                    val internal = I(2)
                    val format = I(3)
                    val type = I(4)
                    val w = I(5)
                    val h = I(6)
                    val dk = I(12)
                    val off = I(13)
                    val dn = I(14)
                    val tgt = if (cube) GL.TEXTURE_CUBE_MAP else GL.TEXTURE_2D
                    var px: java.nio.Buffer? = null
                    if (dk == 1) {
                        px = floatsAt(floats, f, dn)
                        f += dn
                    } else if (dk == 2) {
                        val b = direct(dn)
                        b.put(bytes, off, dn)
                        b.flip()
                        px = b
                    }
                    val old = objs[id] as? Obj.Texture
                    val t = old?.name ?: gl.genTexture()
                    gl.activeTexture(GL.TEXTURE0 + 31) // uploads on a unit no program samples
                    gl.bindTexture(tgt, t)
                    gl.pixelStorei(GL.UNPACK_ALIGNMENT, 1)
                    if (cube) for (face in 0 until 6) gl.texImage2D(GL.TEXTURE_CUBE_MAP_POSITIVE_X + face, 0, internal, w, h, format, type, px)
                    else gl.texImage2D(GL.TEXTURE_2D, 0, internal, w, h, format, type, px)
                    gl.texParameteri(tgt, GL.TEXTURE_MIN_FILTER, I(7))
                    gl.texParameteri(tgt, GL.TEXTURE_MAG_FILTER, I(8))
                    gl.texParameteri(tgt, GL.TEXTURE_WRAP_S, I(9))
                    gl.texParameteri(tgt, GL.TEXTURE_WRAP_T, I(10))
                    if (I(11) and 1 != 0) {
                        gl.texParameteri(tgt, GL.TEXTURE_COMPARE_MODE, GL.COMPARE_REF_TO_TEXTURE)
                        gl.texParameteri(tgt, GL.TEXTURE_COMPARE_FUNC, GL.LEQUAL)
                    }
                    if (I(11) and 2 != 0) gl.generateMipmap(tgt)
                    gl.pixelStorei(GL.UNPACK_ALIGNMENT, 4)
                    gl.bindTexture(tgt, 0)
                    gl.activeTexture(GL.TEXTURE0)
                    objs[id] = Obj.Texture(t)
                    i += 15
                }
                4 -> { // PASS target, w, h, clear | rgba, depth
                    gl.bindFramebuffer(GL.FRAMEBUFFER, framebuffer(ints[i]))
                    gl.viewport(0, 0, I(1), I(2))
                    val clear = I(3)
                    i += 4
                    gl.clearColor(floats[f], floats[f + 1], floats[f + 2], floats[f + 3])
                    gl.clearDepthf(floats[f + 4])
                    f += 5
                    gl.depthMask(true)
                    gl.colorMask(true, true, true, true)
                    gl.clear((if (clear and 1 != 0) GL.COLOR_BUFFER_BIT else 0) or (if (clear and 2 != 0) GL.DEPTH_BUFFER_BIT else 0))
                }
                5 -> { // USE id
                    val p = objs[ints[i++]] as? Obj.Program ?: throw IllegalStateException("gpu-android: USE of a non-program")
                    prog = p
                    gl.useProgram(p.name)
                }
                6 -> { // UNIFORM location, kind, count
                    val p = prog ?: throw IllegalStateException("gpu-android: UNIFORM without a program")
                    val loc = p.locs[I(0)]
                    val kind = I(1)
                    val count = I(2)
                    i += 3
                    if (kind == 7) {
                        if (loc >= 0) gl.uniform1i(loc, I(0))
                        i += count
                    } else {
                        val size = when (kind) { 2 -> 2; 3 -> 3; 4 -> 4; 5 -> 9; 6 -> 16; else -> 1 }
                        if (loc >= 0) when (kind) {
                            6 -> gl.uniformMatrix4fv(loc, count, floats, f)
                            5 -> gl.uniformMatrix3fv(loc, count, floats, f)
                            4 -> gl.uniform4fv(loc, count, floats, f)
                            3 -> gl.uniform3fv(loc, count, floats, f)
                            2 -> gl.uniform2fv(loc, count, floats, f)
                            else -> gl.uniform1fv(loc, count, floats, f)
                        }
                        f += size * count
                    }
                }
                7 -> { // STATE
                    if (I(0) != 0) {
                        gl.enable(GL.BLEND)
                        gl.blendEquationSeparate(I(1), I(2))
                        gl.blendFuncSeparate(I(3), I(4), I(5), I(6))
                        gl.blendColor(floats[f], floats[f + 1], floats[f + 2], floats[f + 3])
                    } else gl.disable(GL.BLEND)
                    if (I(7) != 0) {
                        gl.enable(GL.DEPTH_TEST)
                        gl.depthFunc(I(8))
                    } else gl.disable(GL.DEPTH_TEST)
                    gl.depthMask(I(9) == 1)
                    if (I(10) == 0) gl.disable(GL.CULL_FACE)
                    else {
                        gl.enable(GL.CULL_FACE)
                        gl.cullFace(if (I(10) == 2) GL.FRONT else GL.BACK)
                    }
                    gl.frontFace(if (I(11) != 0) GL.CCW else GL.CW)
                    val cw = I(12) == 1
                    gl.colorMask(cw, cw, cw, cw)
                    if (I(13) != 0) {
                        gl.enable(GL.POLYGON_OFFSET_FILL)
                        gl.polygonOffset(floats[f + 4], floats[f + 5])
                    } else gl.disable(GL.POLYGON_OFFSET_FILL)
                    i += 14
                    f += 6
                }
                8 -> { // ATTRIB location, buffer, size, stride, offset, divisor
                    val loc = I(0)
                    if (ints[i + 1] == 0L) {
                        gl.disableVertexAttribArray(loc)
                        gl.vertexAttrib4f(loc, floats[f], floats[f + 1], floats[f + 2], floats[f + 3])
                        f += 4
                    } else {
                        gl.bindBuffer(GL.ARRAY_BUFFER, buffer(ints[i + 1]).name)
                        gl.enableVertexAttribArray(loc)
                        gl.vertexAttribPointer(loc, I(2), GL.FLOAT, false, I(3), I(4))
                        gl.vertexAttribDivisor(loc, I(5))
                    }
                    i += 6
                }
                9 -> { // DRAW mode, first, count, instances, index buffer
                    val mode = I(0)
                    val first = I(1)
                    val count = I(2)
                    val inst = I(3)
                    val ib = ints[i + 4]
                    i += 5
                    if (ib != 0L) {
                        gl.bindBuffer(GL.ELEMENT_ARRAY_BUFFER, buffer(ib).name)
                        if (inst > 0) gl.drawElementsInstanced(mode, count, GL.UNSIGNED_INT, first * 4, inst)
                        else gl.drawElements(mode, count, GL.UNSIGNED_INT, first * 4)
                    } else if (inst > 0) gl.drawArraysInstanced(mode, first, count, inst)
                    else gl.drawArrays(mode, first, count)
                }
                10 -> { // BIND unit, texture, cube
                    gl.activeTexture(GL.TEXTURE0 + I(0))
                    gl.bindTexture(if (I(2) != 0) GL.TEXTURE_CUBE_MAP else GL.TEXTURE_2D, if (ints[i + 1] != 0L) texture(ints[i + 1]) else 0)
                    gl.activeTexture(GL.TEXTURE0)
                    i += 3
                }
                11 -> { // RELEASE id
                    val id = ints[i++]
                    objs.remove(id)?.let { delete(it) }
                }
                12 -> { // TARGET id, w, h, samples, colour kind, colour, depth kind, depth
                    val id = ints[i]
                    val w = I(1)
                    val h = I(2)
                    val samples = I(3)
                    val kinds = intArrayOf(I(4), I(6))
                    val vals = longArrayOf(ints[i + 5], ints[i + 7])
                    i += 8
                    objs.remove(id)?.let { delete(it) }
                    val fb = gl.genFramebuffer()
                    val rbs = ArrayList<Int>()
                    gl.bindFramebuffer(GL.FRAMEBUFFER, fb)
                    for (at in 0 until 2) {
                        val ak = kinds[at]
                        val av = vals[at]
                        val depthPoint = if (av.toInt() == GL.DEPTH24_STENCIL8 || av.toInt() == GL.DEPTH32F_STENCIL8) GL.DEPTH_STENCIL_ATTACHMENT else GL.DEPTH_ATTACHMENT
                        if (ak == 1 || ak >= 3) {
                            val point = if (at == 0) GL.COLOR_ATTACHMENT0 else GL.DEPTH_ATTACHMENT
                            gl.framebufferTexture2D(GL.FRAMEBUFFER, point, if (ak >= 3) GL.TEXTURE_CUBE_MAP_POSITIVE_X + ak - 3 else GL.TEXTURE_2D, texture(av), 0)
                        } else if (ak == 2) {
                            val rb = gl.genRenderbuffer()
                            gl.bindRenderbuffer(GL.RENDERBUFFER, rb)
                            if (samples > 0) gl.renderbufferStorageMultisample(GL.RENDERBUFFER, samples, av.toInt(), w, h)
                            else gl.renderbufferStorage(GL.RENDERBUFFER, av.toInt(), w, h)
                            gl.framebufferRenderbuffer(GL.FRAMEBUFFER, if (at == 0) GL.COLOR_ATTACHMENT0 else depthPoint, GL.RENDERBUFFER, rb)
                            rbs.add(rb)
                        }
                    }
                    if (kinds[0] == 0) {
                        gl.drawBuffers(intArrayOf(GL.NONE))
                        gl.readBuffer(GL.NONE)
                    }
                    if (gl.checkFramebufferStatus(GL.FRAMEBUFFER) != GL.FRAMEBUFFER_COMPLETE) throw IllegalStateException("gpu-android: target $id is incomplete")
                    gl.bindFramebuffer(GL.FRAMEBUFFER, screenFb)
                    objs[id] = Obj.Target(fb, rbs.toIntArray())
                }
                13 -> { // COPY source, destination, sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, mask, filter
                    gl.bindFramebuffer(GL.READ_FRAMEBUFFER, framebuffer(ints[i]))
                    gl.bindFramebuffer(GL.DRAW_FRAMEBUFFER, framebuffer(ints[i + 1]))
                    gl.blitFramebuffer(I(2), I(3), I(4), I(5), I(6), I(7), I(8), I(9), I(10), I(11))
                    gl.bindFramebuffer(GL.FRAMEBUFFER, screenFb)
                    i += 12
                }
                14 -> { // MIPMAPS texture
                    gl.activeTexture(GL.TEXTURE0 + 31)
                    gl.bindTexture(GL.TEXTURE_2D, texture(ints[i++]))
                    gl.generateMipmap(GL.TEXTURE_2D)
                    gl.bindTexture(GL.TEXTURE_2D, 0)
                    gl.activeTexture(GL.TEXTURE0)
                }
                else -> throw IllegalStateException("gpu-android: unknown command $op at ${i - 1}")
            }
        }
        if (f != floats.size) throw IllegalStateException("gpu-android: ${floats.size - f} floats left over")
    }

    /** The GL objects the stream made, deleted (on the GL thread). */
    fun dispose() {
        for (o in objs.values) delete(o)
        objs.clear()
    }

    /** "<GL_VERSION> | <GL_RENDERER>" */
    fun describe(): String = gl.getString(GL.VERSION) + " | " + gl.getString(GL.RENDERER)
}
