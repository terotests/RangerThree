package rangerthree.gpu

import rangerthree.core.*

/**
 * Scenes built with the three crate's Kotlin output (package
 * rangerthree.core), shared by the JVM test (jvm/Scenes.kt) and the Android
 * app (app/). `animate` takes a time in milliseconds.
 */
private fun sceneWith(bg: Long): Object3D {
    val s = Object3D.scene().shared()
    s.scene_data_mut()!!.background = Background_Color(Color.from_hex(bg))
    return s
}

private fun camera(x: Double, y: Double, z: Double, aspect: Double): Object3D {
    val c = Object3D.perspective_camera(55.0, aspect, 0.1, 100.0).shared()
    c.position._set(x, y, z)
    CoreObject3d.look_at(c, Vector3.new_(0.0, 0.0, 0.0))
    return c
}

class DemoScene(val scene: Object3D, val camera: Object3D, val animate: (Double) -> Unit)

fun torusScene(aspect: Double): DemoScene {
    val scene = sceneWith(0x1b1e26)
    val m = Material.new_(MaterialKind.MeshStandard)
    m.color = Color.from_hex(0xff8a3d)
    val knot = Object3D.mesh(GeometriesPrimitives.torus_knot_geometry(1.0, 0.32, 96.0, 10.0, 2.0, 3.0).shared(), m.shared()).shared()
    CoreObject3d.add(scene, knot)
    CoreObject3d.add(scene, Object3D.ambient_light(Color.from_hex(0xffffff), 0.25).shared())
    val sun = Object3D.directional_light(Color.from_hex(0xffffff), 2.5).shared()
    sun.position._set(3.0, 4.0, 2.0)
    CoreObject3d.add(scene, sun)
    return DemoScene(scene, camera(0.0, 1.5, 5.0, aspect)) { t -> knot.set_rotation_from_euler(Euler.new_(t / 2000.0, t / 1300.0, 0.0, 0L)) }
}

fun primitivesScene(aspect: Double): DemoScene {
    val scene = sceneWith(0x202428)
    val colors = longArrayOf(0x4caf50, 0x2196f3, 0xffc107, 0xe91e63)
    val geos = listOf(
        GeometriesPrimitives.box_geometry(1.2, 1.2, 1.2, 1.0, 1.0, 1.0),
        GeometriesPrimitives.sphere_geometry(0.8, 32.0, 16.0, 0.0, Math.PI * 2, 0.0, Math.PI),
        GeometriesPrimitives.torus_geometry(0.6, 0.25, 16.0, 48.0, Math.PI * 2, 0.0, Math.PI * 2),
        GeometriesPrimitives.cone_geometry(0.7, 1.4, 32.0, 1.0, false, 0.0, Math.PI * 2),
    )
    val meshes = ArrayList<Object3D>()
    for (k in geos.indices) {
        val m = Material.new_(if (k % 2 == 0) MaterialKind.MeshStandard else MaterialKind.MeshPhong)
        m.color = Color.from_hex(colors[k])
        val mesh = Object3D.mesh(geos[k].shared(), m.shared()).shared()
        mesh.position._set(-2.4 + k * 1.6, 0.0, 0.0)
        CoreObject3d.add(scene, mesh)
        meshes.add(mesh)
    }
    CoreObject3d.add(scene, Object3D.hemisphere_light(Color.from_hex(0xbfd8ff), Color.from_hex(0x202020), 1.0).shared())
    val sun = Object3D.directional_light(Color.from_hex(0xffffff), 2.0).shared()
    sun.position._set(2.0, 5.0, 4.0)
    CoreObject3d.add(scene, sun)
    return DemoScene(scene, camera(0.0, 1.5, 6.0, aspect)) { t ->
        for (k in meshes.indices) meshes[k].set_rotation_from_euler(Euler.new_(t / 1500.0 + k, t / 1100.0, 0.0, 0L))
    }
}

val demoScenes: List<Pair<String, (Double) -> DemoScene>> = listOf("torus" to ::torusScene, "primitives" to ::primitivesScene)
