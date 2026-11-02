plugins {
    id("com.android.application")
}

android {
    namespace = "org.algoclerk.game"
    compileSdk = 35

    defaultConfig {
        applicationId = "org.algoclerk.game"
        minSdk = 26
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

    // The native library (libalgoclerk_app.so) is built by cargo-ndk into src/main/jniLibs.
    sourceSets["main"].jniLibs.srcDir("src/main/jniLibs")
}
