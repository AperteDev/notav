// The Kotlin bindings against the library built for this machine (`cargo build` in notav/ffi,
// then the bindings generated into target/bindings/kotlin): `scripts/bindings.sh` runs it. A
// check that they compile and work, not the Android build.
plugins {
    kotlin("jvm") version "2.4.21"
    application
}

repositories { mavenCentral() }

dependencies { implementation("net.java.dev.jna:jna:5.19.1") }

val ffi = projectDir.resolve("../..").normalize()

kotlin {
    jvmToolchain(21)
    sourceSets["main"].kotlin.srcDirs("src", ffi.resolve("target/bindings/kotlin"))
}

application { mainClass.set("SmokeKt") }

tasks.named<JavaExec>("run") {
    systemProperty("jna.library.path", ffi.resolve("target/debug").path)
    workingDir = projectDir
}
