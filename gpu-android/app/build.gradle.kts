// The three crate on Android: Ranger's Kotlin output of three/src/lib.rs, the
// Kotlin GPU player (../player) and its android.opengl Gl (../android), in a
// GLSurfaceView. ../gen-kotlin.sh writes the crate into build/generated.
//
//   cd gpu-android/app && ./gradlew installDebug
plugins {
    id("com.android.application") version "8.5.2"
    id("org.jetbrains.kotlin.android") version "2.0.21"
}

android {
    namespace = "rangerthree.demo"
    compileSdk = 34

    defaultConfig {
        applicationId = "rangerthree.demo"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    sourceSets["main"].java.srcDirs(
        "src/main/kotlin",
        "../player/src",
        "../scenes/src",
        "../android/src",
        "build/generated/rangerthree",
    )
}

val generateCrate by tasks.registering(Exec::class) {
    description = "three/src as Kotlin (Ranger)"
    inputs.dir("../../three/src")
    outputs.file("build/generated/rangerthree/rangerthree.kt")
    commandLine("bash", "../gen-kotlin.sh", "build/generated/rangerthree")
}

tasks.matching { it.name.startsWith("compile") }.configureEach {
    dependsOn(generateCrate)
}
