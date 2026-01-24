/**
 * RuVector Memory Optimizer - Android App Build Configuration
 *
 * This configures the Android app module with Rust native library integration.
 * The native library is built using cargo-ndk and the rust-android-gradle plugin.
 */

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.mozilla.rust-android-gradle.rust-android")
}

android {
    namespace = "com.ruvector.memopt"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.ruvector.memopt"
        minSdk = 24  // Android 7.0+
        targetSdk = 34
        versionCode = 3
        versionName = "0.3.1"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ndk {
            // Support common Android architectures
            abiFilters += listOf("armeabi-v7a", "arm64-v8a", "x86", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
        debug {
            isMinifyEnabled = false
            isDebuggable = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        viewBinding = true
        buildConfig = true
    }

    // Native library configuration
    externalNativeBuild {
        // We use rust-android-gradle instead of CMake
    }

    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }
}

// Rust native library configuration
cargo {
    module = "../.."  // Path to Cargo.toml (project root)
    libname = "ruvector_memopt"
    targets = listOf("arm", "arm64", "x86", "x86_64")
    profile = "release"

    // Features to enable for Android build
    features {
        defaultAnd("android")
    }
}

// Ensure Rust library is built before assembling the app
tasks.whenTaskAdded {
    if (name.startsWith("merge") && name.endsWith("JniLibFolders")) {
        dependsOn("cargoBuild")
    }
}

dependencies {
    // Core Android libraries
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("androidx.appcompat:appcompat:1.6.1")
    implementation("com.google.android.material:material:1.11.0")
    implementation("androidx.constraintlayout:constraintlayout:2.1.4")

    // Lifecycle & ViewModel
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.7.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.7.0")
    implementation("androidx.lifecycle:lifecycle-service:2.7.0")

    // WorkManager for background monitoring
    implementation("androidx.work:work-runtime-ktx:2.9.0")

    // DataStore for preferences
    implementation("androidx.datastore:datastore-preferences:1.0.0")

    // Coroutines
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")

    // JSON parsing (for native library responses)
    implementation("org.json:json:20231013")

    // Charts for memory visualization (optional)
    implementation("com.github.PhilJay:MPAndroidChart:v3.1.0")

    // Testing
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.1")
}
