// Settings for the Jarvy IntelliJ Platform plugin.
//
// Repositories are declared in build.gradle.kts (project scope) — the
// IntelliJ Platform `intellijPlatform { defaultRepositories() }` DSL is
// only on the classpath where the `org.jetbrains.intellij.platform`
// plugin is applied, which is the build script, not here. The foojay
// resolver lets Gradle auto-provision the JDK 21 toolchain the build
// targets even when only a newer JDK is on the machine.

pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "0.9.0"
}

rootProject.name = "jarvy-intellij"
