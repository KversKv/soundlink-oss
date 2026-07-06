plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "com.soundlink.soundlink"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "com.soundlink.soundlink"
        // AudioPlaybackCapture 需要 API 29+（spec §8.3）。
        minSdk = 29
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName

        // libopus JNI 桥接（见 src/main/cpp/CMakeLists.txt）。
        // 需将 libopus 源码放入 src/main/cpp/opus/ 并在 CMakeLists 中取消 add_subdirectory 注释。
        externalNativeBuild {
            cmake {
                arguments("-DANDROID_STL=c++_static")
            }
        }
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }

    buildTypes {
        release {
            // TODO: Add your own signing config for the release build.
            // Signing with the debug keys for now, so `flutter run --release` works.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

dependencies {
    // ChaCha20-Poly1305 AEAD（允许自定义 12B nonce，与桌面端字节级互通）。
    implementation("org.bouncycastle:bcprov-jdk18on:1.78.1")
    // ActivityResultLauncher（MediaProjection 授权）。
    implementation("androidx.activity:activity-ktx:1.9.3")
}

flutter {
    source = "../.."
}
