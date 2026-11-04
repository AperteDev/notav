plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.compose)
}

// SDK versions from gradle.properties, which sdk.sh installs.
val sdkVersion = providers.gradleProperty("notav.platform").get()
    .removePrefix("android-").substringBefore('.').toInt()
val buildTools = providers.gradleProperty("notav.buildTools").get()
val minimumSdk = providers.gradleProperty("notav.minSdk").get().toInt()

// The workspace's version (scripts/release.py sets it in the root Cargo.toml) and its code
// (1.2.3 is 10203), which release.py writes to gradle.properties for F-Droid's update check.
val notavVersion = rootDir.resolve("../../Cargo.toml").readText()
    .let { Regex("""\[workspace\.package][^\[]*?\nversion = "([^"]+)"""").find(it)!!.groupValues[1] }
val notavCode = providers.gradleProperty("notav.versionCode").get().toInt()

android {
    namespace = "dev.aperte.notav"
    compileSdk = sdkVersion
    buildToolsVersion = buildTools
    defaultConfig {
        applicationId = "dev.aperte.notav"
        minSdk = minimumSdk
        targetSdk = sdkVersion
        versionCode = notavCode
        versionName = notavVersion
    }
    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // The release APK is built before its commit and tag exist (the Release workflow
            // tags only what passed), so F-Droid's rebuild of the tag would differ in the hash.
            vcsInfo.include = false
        }
    }
    // Unsigned release APKs, which CI signs with apksigner: F-Droid rebuilds them from the tag and
    // publishes ours when they match byte for byte. The dependency list AGP adds is encrypted for
    // Google, which F-Droid refuses; Play gets it in the bundle.
    dependenciesInfo {
        includeInApk = false
    }
    buildFeatures {
        compose = true
    }
}

kotlin {
    jvmToolchain(21)
}

dependencies {
    implementation(project(":core"))
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons)
    implementation(libs.compose.ui.tooling.preview)
    debugImplementation(libs.compose.ui.tooling)
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.viewmodel.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.core.ktx)
    implementation(libs.coroutines.android)
    implementation(libs.browser)
    implementation(libs.work.runtime)
}
