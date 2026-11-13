package rangerthree.gpu

import java.nio.Buffer
import java.nio.ByteBuffer

/**
 * The OpenGL ES 3.0 calls the player makes, shaped like android.opengl.GLES30
 * so AndroidGl is a line per call. Object names are Ints, as GLES returns them;
 * a uniform location is -1 when the program has no such uniform.
 */
interface Gl {
    fun getString(name: Int): String
    fun genVertexArray(): Int
    fun bindVertexArray(vao: Int)

    fun genBuffer(): Int
    fun deleteBuffer(b: Int)
    fun bindBuffer(target: Int, b: Int)
    fun bufferData(target: Int, bytes: Int, data: Buffer, usage: Int)
    fun bufferSubData(target: Int, offset: Int, bytes: Int, data: Buffer)

    fun createShader(type: Int): Int
    fun shaderSource(s: Int, source: String)
    fun compileShader(s: Int)
    fun shaderCompiled(s: Int): Boolean
    fun shaderInfoLog(s: Int): String
    fun deleteShader(s: Int)
    fun createProgram(): Int
    fun attachShader(p: Int, s: Int)
    fun linkProgram(p: Int)
    fun programLinked(p: Int): Boolean
    fun programInfoLog(p: Int): String
    fun deleteProgram(p: Int)
    fun useProgram(p: Int)
    fun getUniformLocation(p: Int, name: String): Int
    fun getUniformBlockIndex(p: Int, name: String): Int
    fun uniformBlockBinding(p: Int, index: Int, binding: Int)
    fun bindBufferRange(target: Int, index: Int, b: Int, offset: Int, bytes: Int)

    fun uniform1i(loc: Int, v: Int)
    fun uniform1fv(loc: Int, count: Int, v: FloatArray, offset: Int)
    fun uniform2fv(loc: Int, count: Int, v: FloatArray, offset: Int)
    fun uniform3fv(loc: Int, count: Int, v: FloatArray, offset: Int)
    fun uniform4fv(loc: Int, count: Int, v: FloatArray, offset: Int)
    fun uniformMatrix3fv(loc: Int, count: Int, v: FloatArray, offset: Int)
    fun uniformMatrix4fv(loc: Int, count: Int, v: FloatArray, offset: Int)

    fun genTexture(): Int
    fun deleteTexture(t: Int)
    fun activeTexture(unit: Int)
    fun bindTexture(target: Int, t: Int)
    fun pixelStorei(name: Int, v: Int)
    fun texImage2D(target: Int, level: Int, internal: Int, w: Int, h: Int, format: Int, type: Int, data: Buffer?)
    fun texParameteri(target: Int, name: Int, v: Int)
    fun generateMipmap(target: Int)

    fun genFramebuffer(): Int
    fun deleteFramebuffer(fb: Int)
    fun bindFramebuffer(target: Int, fb: Int)
    fun framebufferTexture2D(target: Int, attachment: Int, texTarget: Int, t: Int, level: Int)
    fun genRenderbuffer(): Int
    fun deleteRenderbuffer(rb: Int)
    fun bindRenderbuffer(target: Int, rb: Int)
    fun renderbufferStorage(target: Int, internal: Int, w: Int, h: Int)
    fun renderbufferStorageMultisample(target: Int, samples: Int, internal: Int, w: Int, h: Int)
    fun framebufferRenderbuffer(target: Int, attachment: Int, rbTarget: Int, rb: Int)
    fun drawBuffers(bufs: IntArray)
    fun readBuffer(src: Int)
    fun checkFramebufferStatus(target: Int): Int
    fun blitFramebuffer(sx0: Int, sy0: Int, sx1: Int, sy1: Int, dx0: Int, dy0: Int, dx1: Int, dy1: Int, mask: Int, filter: Int)

    fun viewport(x: Int, y: Int, w: Int, h: Int)
    fun clearColor(r: Float, g: Float, b: Float, a: Float)
    fun clearDepthf(d: Float)
    fun clear(mask: Int)
    fun enable(cap: Int)
    fun disable(cap: Int)
    fun depthMask(on: Boolean)
    fun colorMask(r: Boolean, g: Boolean, b: Boolean, a: Boolean)
    fun depthFunc(f: Int)
    fun blendEquationSeparate(rgb: Int, alpha: Int)
    fun blendFuncSeparate(srcRgb: Int, dstRgb: Int, srcAlpha: Int, dstAlpha: Int)
    fun blendColor(r: Float, g: Float, b: Float, a: Float)
    fun cullFace(mode: Int)
    fun frontFace(mode: Int)
    fun polygonOffset(factor: Float, units: Float)

    fun enableVertexAttribArray(i: Int)
    fun disableVertexAttribArray(i: Int)
    fun vertexAttrib4f(i: Int, x: Float, y: Float, z: Float, w: Float)
    fun vertexAttribPointer(i: Int, size: Int, type: Int, normalized: Boolean, stride: Int, offset: Int)
    fun vertexAttribDivisor(i: Int, divisor: Int)
    fun drawArrays(mode: Int, first: Int, count: Int)
    fun drawArraysInstanced(mode: Int, first: Int, count: Int, instances: Int)
    fun drawElements(mode: Int, count: Int, type: Int, offset: Int)
    fun drawElementsInstanced(mode: Int, count: Int, type: Int, offset: Int, instances: Int)

    fun readPixels(x: Int, y: Int, w: Int, h: Int, format: Int, type: Int, out: ByteBuffer)
}

/** The GLES 3.0 enums the player uses. */
object GL {
    const val VERSION = 0x1F02
    const val RENDERER = 0x1F01
    const val ARRAY_BUFFER = 0x8892
    const val ELEMENT_ARRAY_BUFFER = 0x8893
    const val UNIFORM_BUFFER = 0x8A11
    const val INVALID_INDEX = -1 // 0xFFFFFFFF
    const val STATIC_DRAW = 0x88E4
    const val DYNAMIC_DRAW = 0x88E8
    const val VERTEX_SHADER = 0x8B31
    const val FRAGMENT_SHADER = 0x8B30
    const val TEXTURE_2D = 0x0DE1
    const val TEXTURE_CUBE_MAP = 0x8513
    const val TEXTURE_CUBE_MAP_POSITIVE_X = 0x8515
    const val TEXTURE0 = 0x84C0
    const val TEXTURE_MIN_FILTER = 0x2801
    const val TEXTURE_MAG_FILTER = 0x2800
    const val TEXTURE_WRAP_S = 0x2802
    const val TEXTURE_WRAP_T = 0x2803
    const val TEXTURE_COMPARE_MODE = 0x884C
    const val TEXTURE_COMPARE_FUNC = 0x884D
    const val COMPARE_REF_TO_TEXTURE = 0x884E
    const val UNPACK_ALIGNMENT = 0x0CF5
    const val FRAMEBUFFER = 0x8D40
    const val READ_FRAMEBUFFER = 0x8CA8
    const val DRAW_FRAMEBUFFER = 0x8CA9
    const val RENDERBUFFER = 0x8D41
    const val COLOR_ATTACHMENT0 = 0x8CE0
    const val DEPTH_ATTACHMENT = 0x8D00
    const val DEPTH_STENCIL_ATTACHMENT = 0x821A
    const val DEPTH24_STENCIL8 = 0x88F0
    const val DEPTH32F_STENCIL8 = 0x8CAD
    const val FRAMEBUFFER_COMPLETE = 0x8CD5
    const val NONE = 0
    const val COLOR_BUFFER_BIT = 0x4000
    const val DEPTH_BUFFER_BIT = 0x0100
    const val BLEND = 0x0BE2
    const val DEPTH_TEST = 0x0B71
    const val CULL_FACE = 0x0B44
    const val POLYGON_OFFSET_FILL = 0x8037
    const val FRONT = 0x0404
    const val BACK = 0x0405
    const val CW = 0x0900
    const val CCW = 0x0901
    const val LEQUAL = 0x0203
    const val FLOAT = 0x1406
    const val UNSIGNED_INT = 0x1405
    const val UNSIGNED_BYTE = 0x1401
    const val RGBA = 0x1908
}
