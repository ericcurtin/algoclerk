// SPDX-License-Identifier: Apache-2.0
plugins {
    id("com.android.application")
}

android {
    namespace = "org.algoclerk.game"
    compileSdk = 35

    defaultConfig {
        applicationId = "org.algoclerk.game"
        minSdk = 28
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    signingConfigs {
        // Release signing is optional: set these environment variables (see android/README.md).
        // Without them the release APK/AAB are produced unsigned.
        val keystore = System.getenv("ALGOCLERK_KEYSTORE")
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("ALGOCLERK_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ALGOCLERK_KEY_ALIAS")
                keyPassword = System.getenv("ALGOCLERK_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfigs.findByName("release")?.let { signingConfig = it }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // libalgoclerk_app.so per ABI, built by cargo-ndk into src/main/jniLibs.
    sourceSets["main"].jniLibs.srcDir("src/main/jniLibs")
    buildFeatures { prefab = true }
}

dependencies {
    // GameActivity is the activity flavour enabled by the `android-game-activity` cargo feature.
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.core:core:1.13.1")
    implementation("androidx.games:games-activity:2.0.2")
}
