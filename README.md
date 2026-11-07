# Rocket Launcher

**Beyond the Built Limit.** A native Windows Minecraft Java Edition launcher built with Tauri 2, Rust, React, TypeScript, Vite, Tailwind CSS, Motion, and Lucide.

## Launcher features

Rocket Launcher manages Vanilla, Fabric, and Forge instances on Windows. Choose a local offline profile for single-player or sign in with Microsoft for authenticated play. Forge offers recommended and latest builds for Minecraft 1.20.1 and newer releases when listed by Forge.

- Launch Pad, Instances, Mods, Downloads, and Settings navigation.
- Create, rename, edit, select, duplicate, and remove instances.
- Separate UUID-named game directories; configurable Minecraft version, RAM, and Java executable path.
- Native directory and executable pickers.
- Atomic Rust-managed configuration persistence and single-instance application behavior.
- Live official Mojang version catalogue, release/snapshot filters, local cache, bounded requests, timeout, one retry, and explicit failure/fallback states.
- Mojang client, libraries, assets, logging configuration and native archives, with streaming downloads, SHA-1 verification, shared cache reuse, and offline repair checks.
- Java discovery with exact version and architecture matching. A manually selected executable takes priority.
- Launch Pad install and repair actions, measured Downloads progress, and native launch preparation behind an ownership-verified session boundary.
- Microsoft OAuth sign-in, Xbox/Minecraft ownership verification, Windows Credential Manager refresh-token storage, and authenticated launch sessions.
- Dark, light, and system themes; custom accent colours; animation and reduced-motion preferences.
- Built-in Modrinth search for Fabric and Forge instances, matching Minecraft versions and loader. Install directly from search results or choose a specific release. Rocket Launcher verifies the JAR and resolves required Modrinth dependencies.
- Local `.jar` file listing from each instance's mods directory.
- Local rotating structured logs and an in-app log viewer.
- Keyboard-accessible controls, native modal focus management, responsive desktop layout.

Java runtimes are selected from installed runtimes. Launcher updates are applied from Settings.

## Install mods

In **Mods**, pick a Minecraft version and select **Create Rocket Pack instance**. The full 1.21.11 pack uses Fabric Loader 0.20.1 and pinned mods from the supplied Tuff pack. Other Fabric releases use starter packs assembled from matching versions of Sodium, Lithium, and Fabric API, depending on availability. Install the Minecraft files from **Launch Pad** and use the Java version it requests.

For a custom Minecraft title screen or pause menu, install the matching Fabric release of [FancyMenu](https://modrinth.com/mod/fancymenu), open Minecraft and create a layout from the FancyMenu toolbar. Share your layout by copying its [configuration folder](https://docs.fancymenu.net/docs/en-US/modpacks) into the pack. [ImproperUI](https://modrinth.com/mod/improperui) is also available for scripted in-game screens.

Create or select a Fabric or Forge instance, then open **Mods**. Click **Install** on a result to choose a compatible release automatically, or use **Versions** to pick one yourself. Required Modrinth dependencies are resolved against the same instance. An external dependency without a Modrinth version must already be in the instance's mods folder. To switch an installed project's release, move its old JAR out of the mods folder before installing the replacement. Offline Mode keeps local mod files available and pauses catalogue requests and downloads.

## Run locally on Windows 10/11

The **Launcher updates** row in Settings checks the project's GitHub releases for a newer Windows x64 installer. Select **Download and install** to stream it to disk and verify its size and SHA-256 checksum against GitHub's release metadata. Rocket Launcher closes while the installer applies the update, then reopens. Close Minecraft before updating.

Prerequisites:

1. Node.js 24 LTS and npm.
2. A current stable Rust MSVC toolchain. This checkout was validated with Rust 1.97.1.
3. Visual Studio C++ Build Tools with **Desktop development with C++** and the Windows SDK.
4. Microsoft Edge WebView2 Runtime.

See the [official Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows).

```powershell
cd C:\Users\chazq\MEGA\Music\rocket-launcher
npm ci
npm run desktop
```

`npm run desktop` starts Vite and the native Tauri application together. The first Rust build takes longer while dependencies compile. `npm run dev` starts only the browser frontend; it intentionally displays a native-app requirement instead of substituting mock storage or a fake backend.

## Microsoft sign-in

Microsoft sign-in requires a public-client app registration approved for the Xbox/Minecraft authentication flow. Set its client ID before starting the desktop app:

```powershell
$env:ROCKET_MICROSOFT_CLIENT_ID = '<approved application client ID>'
npm run desktop
```

For your own registration, enable personal Microsoft accounts and add `http://localhost/callback` under the **Mobile and desktop applications** platform. Sign in with an account that owns Minecraft Java Edition. The launcher verifies ownership and stores refresh tokens in Windows Credential Manager. You can also create a local profile in Settings using an offline username. Local profiles use a stable offline identity for single-player. Download the game files and Java before playing without a network connection.

Build a standalone executable:

```powershell
npm run desktop:build
```

Output: `src-tauri\target\release\rocket-launcher.exe`. This executable embeds the frontend and does not need Vite. Generate the unsigned Windows NSIS installer with `npm run tauri -- build`.

## Validation

```powershell
npm run typecheck
npm run build
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
# Explicit network integration test: downloads Mojang's real manifest, then reads the cache.
cargo test --manifest-path src-tauri/Cargo.toml live_manifest_fetch -- --ignored
# Download to an isolated temporary directory, verify locally, delete one test JAR, and repair.
$env:ROCKET_TEST_VERSION = '1.21.1'
cargo test --manifest-path src-tauri/Cargo.toml live_install_and_offline_repair -- --ignored
# Search real Modrinth projects and install matched mods and dependencies into temporary instances.
cargo test --manifest-path src-tauri/Cargo.toml live_modrinth_matching_and_installation -- --ignored
# With Java 21 installed, also start the isolated Fabric client and check renderer initialization.
$env:ROCKET_SMOKE_GAME = '1'
cargo test --manifest-path src-tauri/Cargo.toml live_modrinth_matching_and_installation -- --ignored --nocapture
Remove-Item Env:ROCKET_SMOKE_GAME
```

Rust tests exercise configuration replacement/reopening, corruption preservation, validation, the instance lifecycle, file preservation, local mod listing, manifest parsing, and cache lookup. Frontend tests exercise submission, error/retry states, preferences, and feature availability. Mock data exists only under `src/test`.

See [PROGRESS.md](PROGRESS.md) for Phase 2 verification and [the Phase 1 validation record](docs/VALIDATION.md) for the foundation checks.

## Local data and behavior

The application uses Tauri's local application data directory, normally `%LOCALAPPDATA%\app.rocketlauncher.desktop` on Windows. The exact paths appear in Settings.

| Path | Contents |
| --- | --- |
| `launcher.json` | Versioned settings, instance configurations, and active selection |
| `instances\<UUID>\` | Default isolated game directories |
| `cache\versions.json` | Validated Mojang catalogue with fetch timestamp |
| `cache\minecraft\` | Shared version metadata, client JARs, libraries, assets, extracted natives, and installation records |
| `logs\game\` | Per-launch stdout and stderr |
| `logs\launcher.log.*` | Daily JSON logs, at most seven files |

Configuration writes use a same-directory temporary file, flush to disk, then atomically replace the destination. Invalid configuration is preserved rather than silently reset. Back up `launcher.json` before manual edits. A different launcher schema version is rejected.

Removing an instance **only removes its configuration**; game directories are preserved. Duplication copies settings into a **new empty directory**; it does not copy saves, mods, or game files. Existing directories cannot be relocated through the editor. Changing the default directory affects newly created instances only.

RAM is configurable between 1 and 64 GB in 256 MB increments. Java discovery checks the instance override, the launcher setting, `JAVA_HOME`, `PATH`, common Windows installations, and launcher runtime folders. Install the Java major reported on the Launch Pad and use a runtime matching the launcher's Windows architecture.

The catalogue is reused for one hour, then refreshed. Refresh always tries the network. Valid cached versions remain available when Mojang cannot be reached, with a visible warning. Without any valid cache, creating an instance requires the first successful fetch. Existing instances can be renamed, selected, removed, and edited without changing their Minecraft version while offline. Historical alpha/beta versions are intentionally excluded.

Create or select an instance on the Launch Pad, then click **Install Minecraft**. Downloads shows measured transfer speed and bytes, plus verified bytes including cache hits. Interrupted downloads resume at file boundaries: only verified files are reused. Select **Repair files** after failed local verification, or **Retry installation** after a download error. The launcher can validate installed files without contacting Mojang. Verified files and a compatible Java runtime do not grant Minecraft account access.

## Architecture

```text
src/
  app/                 Application composition, navigation, theme application
  components/          Layout, reusable controls, instance forms, vector artwork
  hooks/               Native state and operation lifecycle
  pages/               Launch Pad, Instances, Mods, Downloads, Settings
  services/            Typed Tauri command bridge; no browser fallback backend
  styles/              Design tokens, Tailwind entry point, responsive styles
  types/               IPC contracts
  test/                UI unit tests and test-only fixtures
src-tauri/src/
  commands/            Narrow IPC boundary
  errors.rs            Serializable, actionable errors
  instances/           Instance lifecycle and scoped mod enumeration
  minecraft/           Mojang catalogue, install planning, downloads, Java, and launch supervision
  models.rs            Serializable configuration/domain models
  storage/             Validation and atomic persistence
```

The frontend has no filesystem or shell plugin permissions. Rust downloads from allowed Mojang HTTPS hosts. No account credentials or telemetry are collected. Inter, Space Grotesk and JetBrains Mono are bundled for offline use under the SIL Open Font License; notices are in `public/licenses/`.

## Remaining authentication work

Microsoft sign-in and authenticated online services require a working Microsoft app registration. Local offline profiles can launch installed game files without signing in.

Rocket Launcher is an independent project, not an official Minecraft product or approved by Mojang or Microsoft.
