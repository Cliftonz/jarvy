# Jarvy — JetBrains / IntelliJ Platform plugin

IDE integration for [Jarvy](../../), the cross-platform CLI that provisions
dev environments from `jarvy.toml`. The plugin shells out to the `jarvy`
binary on your `PATH` and surfaces it inside JetBrains IDEs (IntelliJ IDEA,
GoLand, PyCharm, RustRover, WebStorm, …).

## Features

- **Live validation** of `jarvy.toml`: an `ExternalAnnotator` runs
  `jarvy validate --format json` in the background and highlights errors,
  warnings, and info notes right in the editor (with the CLI's suggestions).
- **Tools ▸ Jarvy** menu actions:
  - **Setup** — `jarvy setup --file <jarvy.toml>`
  - **Doctor** — `jarvy doctor`
  - **Validate jarvy.toml** — `jarvy validate --format json` (summary balloon)

The plugin locates the executable via `PATH` (cross-platform `which`/`where`).
Override the location with the `JARVY_BINARY` environment variable if `jarvy`
lives somewhere non-standard.

## Requirements

- A recent JetBrains IDE (build `242`/2024.2 or newer — see `gradle.properties`).
- The `jarvy` binary installed and on your `PATH` (or `JARVY_BINARY` set).

## Build

This project uses the [IntelliJ Platform Gradle Plugin 2.x](https://plugins.jetbrains.com/docs/intellij/tools-intellij-platform-gradle-plugin.html)
and targets JDK 21.

```bash
cd editors/jetbrains

# One-time: generate the Gradle wrapper JAR if it is not present
# (the binary gradle-wrapper.jar is not checked in). Requires a system Gradle:
gradle wrapper --gradle-version 8.10.2

# Build the distributable plugin ZIP
./gradlew buildPlugin
# -> build/distributions/Jarvy-0.1.0.zip
```

Lighter checks (no full plugin ZIP):

```bash
./gradlew tasks           # list available tasks (fast, verifies config)
./gradlew compileKotlin   # compile the Kotlin sources only
./gradlew verifyPlugin    # run the IntelliJ Plugin Verifier
```

> **Note:** The first build downloads the unified IntelliJ IDEA platform
> distribution (~1 GB; since 2025.3 Community and Ultimate ship as one) plus
> the Gradle distribution. Subsequent builds are cached.

## Run in a sandbox IDE

```bash
./gradlew runIde
```

Launches a throwaway IDE instance with the plugin installed so you can try
the actions and live validation against a real `jarvy.toml`.

## Install from disk

1. Build the ZIP with `./gradlew buildPlugin`.
2. In the IDE: **Settings ▸ Plugins ▸ ⚙ ▸ Install Plugin from Disk…**
3. Choose `build/distributions/Jarvy-0.1.0.zip` and restart the IDE.

## Layout

```
editors/jetbrains/
├── build.gradle.kts            # IntelliJ Platform Gradle Plugin 2.x config
├── settings.gradle.kts         # repositories + toolchain resolver
├── gradle.properties           # plugin/platform versions, build range
├── gradle/wrapper/…            # wrapper properties (jar generated separately)
├── src/main/resources/META-INF/plugin.xml
└── src/main/kotlin/com/jarvy/intellij/
    ├── JarvyCli.kt             # locate + run the jarvy binary
    ├── JarvyConfig.kt          # resolve the project's jarvy.toml
    ├── JarvyNotifier.kt        # balloon notifications
    ├── actions/                # Setup / Doctor / Validate actions
    └── validation/             # ExternalAnnotator + JSON model
```

## Versioning & releasing (maintainers)

The plugin version lives in `gradle.properties` (`pluginVersion`) and is
**independent of the jarvy CLI's git tags** — the JetBrains Marketplace reads
the version from the uploaded plugin zip, not from a git tag.

To release:

1. Bump `pluginVersion` in `editors/jetbrains/gradle.properties`.
2. Tag `jetbrains-vX.Y.Z` and push it (the `jetbrains-` prefix keeps CLI
   `vX.Y.Z` releases from triggering the publish workflow).

That push triggers `.github/workflows/jetbrains-publish.yml`, which runs
`buildPlugin` + `verifyPlugin`, then `signPlugin` + `publishPlugin` to the
**JetBrains Marketplace**. Publishing needs repo secrets: `PUBLISH_TOKEN`
(Marketplace permanent token) and the signing trio `CERTIFICATE_CHAIN` /
`PRIVATE_KEY` / `PRIVATE_KEY_PASSWORD`. The publish step is skipped with a
notice if `PUBLISH_TOKEN` is absent, and `workflow_dispatch` offers a
build-only dry run. A `pluginVersion` like `0.2.0-beta.1` auto-routes to the
matching Marketplace pre-release channel.
