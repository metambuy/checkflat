plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.checkflat.cameracapture"
    compileSdk = 36

    defaultConfig {
        minSdk = 24
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
}

dependencies {
    // core: FileProvider. activity: ActivityResult in the @ActivityCallback signature.
    implementation("androidx.core:core-ktx:1.9.0")
    implementation("androidx.activity:activity:1.10.1")
    implementation(project(":tauri-android"))
}
