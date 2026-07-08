// Jarvy IntelliJ Platform plugin build script.
//
// Uses the IntelliJ Platform Gradle Plugin 2.x (`org.jetbrains.intellij.platform`),
// NOT the legacy `intellij {}` block. Targets IntelliJ IDEA Community; the
// concrete version is pinned in gradle.properties so the whole matrix
// (platform version, since/until build) lives in one place.

plugins {
    id("java")
    id("org.jetbrains.kotlin.jvm") version "2.1.0"
    id("org.jetbrains.intellij.platform") version "2.5.0"
}

group = providers.gradleProperty("pluginGroup").get()
version = providers.gradleProperty("pluginVersion").get()

repositories {
    mavenCentral()
    // IntelliJ Platform artifacts (IDE distributions, bundled plugins,
    // the plugin verifier, marketplace metadata). The `intellijPlatform`
    // repository DSL is provided by the plugin applied above; it must
    // live here (project scope) rather than in settings.gradle.kts,
    // where the extension isn't on the classpath under 2.5.0.
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        // Target IDE: IntelliJ IDEA Community (platformVersion in gradle.properties).
        intellijIdeaCommunity(providers.gradleProperty("platformVersion"))

        // The bundled TOML language support. Our ExternalAnnotator attaches
        // to the "TOML" language so it can highlight problems in jarvy.toml.
        // TOML support ships bundled in every IntelliJ-based IDE since 2023.1.
        bundledPlugin("org.toml.lang")

        // Tooling used by the `verifyPlugin` and `signPlugin` tasks.
        pluginVerifier()
        zipSigner()
    }

    // Gson is used to parse `jarvy validate --format json`. Declared (and
    // bundled into the plugin's lib/) so compilation never depends on whether
    // the target IDE exposes Gson on the compile classpath.
    implementation("com.google.code.gson:gson:2.11.0")
}

kotlin {
    // IntelliJ plugins target JDK 21 even on machines running a newer JDK.
    // The foojay resolver (see settings.gradle.kts) provisions it if absent.
    jvmToolchain(21)
}

intellijPlatform {
    pluginConfiguration {
        name = providers.gradleProperty("pluginName")
        version = providers.gradleProperty("pluginVersion")

        ideaVersion {
            sinceBuild = providers.gradleProperty("pluginSinceBuild")
            untilBuild = providers.gradleProperty("pluginUntilBuild")
        }
    }

    pluginVerification {
        ides {
            recommended()
        }
    }

    // Marketplace signing + publishing, driven entirely by env vars so the
    // config is inert in local/CI builds that don't set them (the
    // jetbrains-publish workflow supplies them from repo secrets). The
    // plugin version is `pluginVersion` in gradle.properties — independent
    // of the jarvy CLI's git tags.
    signing {
        certificateChainFile = providers.environmentVariable("CERTIFICATE_CHAIN")
            .map { file(it) }.orNull
        privateKeyFile = providers.environmentVariable("PRIVATE_KEY")
            .map { file(it) }.orNull
        password = providers.environmentVariable("PRIVATE_KEY_PASSWORD").orNull
    }

    publishing {
        token = providers.environmentVariable("PUBLISH_TOKEN").orNull
        // Marketplace release channel: default "stable"; a version like
        // 0.1.0-beta.1 auto-routes to the matching pre-release channel.
        channels = providers.gradleProperty("pluginVersion").map {
            listOf(it.substringAfter('-', "").substringBefore('.').ifEmpty { "stable" })
        }
    }
}
