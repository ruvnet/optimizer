/**
 * RuVector Memory Optimizer - Android Build Configuration
 *
 * This is the root build configuration for the Android integration.
 * It sets up the necessary plugins and configurations for building
 * the Android app with Rust native library support.
 */

plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.kotlin.android) apply false
    id("org.mozilla.rust-android-gradle.rust-android") version "0.9.4" apply false
}

buildscript {
    repositories {
        google()
        mavenCentral()
        maven { url = uri("https://plugins.gradle.org/m2/") }
    }
    dependencies {
        classpath("com.android.tools.build:gradle:8.2.0")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.21")
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

tasks.register("clean", Delete::class) {
    delete(rootProject.buildDir)
}
