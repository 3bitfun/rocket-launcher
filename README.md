# Rocket Launcher

**Beyond the Built Limit.** A native Windows Minecraft Java Edition launcher built with Tauri 2, Rust, React, TypeScript, Vite, Tailwind CSS, Motion, and Lucide.

## Getting started

1. Download the Windows x64 installer or standalone executable from [GitHub Releases](https://github.com/3bitfun/rocket-launcher/releases). Releases currently use GitHub's prerelease channel. Run the installer to install Rocket Launcher, or run the standalone EXE directly.
2. In **Settings → Accounts**, create a username under **New offline username**, or sign in with a Microsoft account that owns Minecraft Java Edition. Microsoft sign-in requires the configured app registration described below.
3. On the **Launch Pad**, choose **Default (Rocket)**, select a Minecraft version, and click **Install & play**. The launcher prepares the Rocket Pack, downloads Minecraft and Fabric as needed, checks Java and the player profile, and starts the game.
4. For another setup, open **Instances → Create instance** and choose Fabric Rocket Pack, Vanilla, Fabric, or Forge. Click an instance card to open its manager.

The first setup requires an internet connection. Once the files and a compatible Java runtime are installed, an offline profile can play single-player without Microsoft sign-in. Enable **Offline mode** in Settings to pause launcher network requests. Changing an offline username changes its offline player ID, which can affect world and server player data.

## Launcher features

Rocket Launcher manages Vanilla, Fabric, and Forge instances on Windows. Choose a local offline profile for single-player or sign in with Microsoft for authenticated play. Forge offers recommended and latest builds for Minecraft 1.20.1 and newer releases when listed by Forge.

- Launch Pad, Instances, Multiplayer, Mods, Downloads, and Settings navigation.
- **Ctrl+K** opens quick search for launcher pages and instances by name, game version, or loader. The search supports keyboard navigation.
- Star instances to keep favorites at the top of the Launch Pad picker and instance lists. Filter Instances to show favorites only. Rocket Pack instances are grouped in the **Rocket** folder in Instances.
- Click an instance to open its manager, where you can play, repair files, install Rocket Client, and open the instance’s folder or installed mods.
- Create, rename, edit, select, duplicate, and remove instances.
- Separate UUID-named game directories; configurable Minecraft version, RAM, and Java executable path.
- Native directory and executable pickers.
- Atomic Rust-managed configuration persistence and single-instance application behavior.
- Live official Mojang version catalogue, release/snapshot filters, local cache, bounded requests, timeout, one retry, and explicit failure/fallback states.
- Mojang client, libraries, assets, logging configuration and native archives, with streaming downloads, SHA-1 verification, shared cache reuse, and offline repair checks.
- Java discovery with exact version and architecture matching. A manually selected executable takes priority.
- Launch Pad **Install & play** for Rocket Pack, separate install and repair actions, measured Downloads progress, and native launch preparation. The header shows download progress and provides access to active Minecraft sessions.
- Abort a launch or stop a running game from the Launch Pad or instance manager. Running sessions are accessible from the header on other pages. The manager can display, refresh, and copy output and error logs from the latest session recorded by launcher v0.1.20 or later.
- **Diagnose crash** opens the instance’s crash and log viewer. It checks captured output, the Minecraft log, crash reports, and JVM reports for common Java, memory, mod, and graphics failures. The session picker lets you review up to 12 captured sessions, including earlier failures after relaunching. Use **Copy summary** to share the suggested causes without copying the full logs.
- Optional server list, keybind, and Rocket Client HUD sync across instances, enabled by default.
- Microsoft OAuth sign-in, Xbox/Minecraft ownership verification, Windows Credential Manager refresh-token storage, and authenticated launch sessions.
- Dark, light, and system themes; custom accent colours; animation and reduced-motion preferences.
- Built-in Modrinth mod and modpack browsing, compatible mod version selection, dependency resolution, and verified downloads.
- Local `.jar` listing, search, individual and bulk enable/disable controls, and a shortcut to each instance's mods folder.
- Saved multiplayer servers, status checks, direct joining on supported versions, and optional ViaFabric and e4mc installation for compatible Fabric instances.
- Local rotating structured logs and an in-app log viewer.
- Keyboard-accessible controls, native modal focus management, responsive desktop layout.

Java runtimes are selected from installed runtimes. Launcher updates are applied from Settings.

## Rocket Pack and instances

**Create an instance** defaults to **Fabric Rocket Pack**. You can also choose plain Fabric, Vanilla, or Forge. The Launch Pad's **Default (Rocket)** option exposes a version picker, while named instances use their own saved version. Choosing a version is immediate; **Install & play** begins downloading on first launch. An existing Rocket Pack instance for that version is reused. You can also create a pack through **Mods → Rocket Pack** or the instance editor.

The full 1.21.11 pack uses Fabric Loader 0.20.1 and pinned mods from the supplied Tuff pack. Other releases draw from 24 Tuff projects, including Sodium, AppleSkin, Jade, Zoomify, and Iris. Rocket Pack selects compatible Fabric builds and required dependencies when available, so its contents vary by Minecraft version. Litematica is excluded. Fabric and compatible project availability determine which versions can be installed.

The full pack downloads 68 mods and 28 resource and shader packs from the original pack index. It uses a newer Simple Voice Chat build to satisfy Flashback’s compatibility requirement. The original archive’s three local overrides are separate from these downloads. Rocket Launcher keeps verified downloads in a shared cache and copies them into new full-pack instances.

The version picker shows whether each Rocket Pack is installed, needs a download, or needs repair. New Rocket Packs live in the **Rocket** subfolder of your configured instance directory. On startup, the launcher moves existing Rocket Pack folders on the same drive into that folder and updates their saved paths. Instances that could not be moved remain usable at their original location.

Each instance has its own game directory and settings. The manager provides launch, installation, repair, Java and RAM settings, mod management, Rocket Client installation, and folder access. Duplicating copies the configuration into a new empty directory. Removing an instance removes its launcher entry while preserving its files. Rocket Pack game and loader versions are locked in the editor; choose another version on the Launch Pad to keep its files separate.

## Rocket Client

**Rocket Client** comes with the full pack and compatible starter packs. For another instance, click its card and select **Install Rocket Client**. The launcher picks the JAR built for that Minecraft release and installs Fabric API if needed. The instance manager reports availability and the minimum Fabric Loader version for the selected release.

The launcher bundles 34 Fabric builds covering the stable Minecraft Java releases from 1.17 through 26.3. For a manual installation, use the JAR for your exact Minecraft version alongside matching Fabric API.

Click **Rocket** on Minecraft’s title or pause menu to toggle FPS, coordinates, heading, world time, server ping, Java heap usage, movement keys, and armor. Select **Edit HUD layout** and drag the widgets into position. The layout scales with the screen, and **Reset positions** restores the defaults. Assign the optional **Open Rocket menu** shortcut in Minecraft’s Controls. The **Sync Rocket Client HUD** setting shares module choices and positions across compatible instances.

The **Crosshair Editor** offers vanilla, custom, and hidden modes with a live preview. Choose cross, dot, circle, T-shape, or combinations; adjust size, thickness, gap, opacity, outline, shadow, and RGB or hex colors; and enable dynamic expansion or a target color. Built-in presets and custom save slots persist between sessions. Rocket Client settings are shared across compatible instances when the launcher’s HUD sync is enabled.

To rebuild a mapped client JAR, run `.\gradlew.bat remapJar '-PmcVersion=1.21.11'` from `rocket-companion` on JDK 21. For 26.x, use JDK 25 and Gradle 9.9 with `gradle -p unobfuscated jar '-PmcVersion=26.3'`. Copy the built JAR to `src-tauri/resources/rocket-client/<minecraft-version>.jar` to bundle it.

For a custom Minecraft title screen or pause menu, install the matching Fabric release of [FancyMenu](https://modrinth.com/mod/fancymenu), open Minecraft and create a layout from the FancyMenu toolbar. Share your layout by copying its [configuration folder](https://docs.fancymenu.net/docs/en-US/modpacks) into the pack. [ImproperUI](https://modrinth.com/mod/improperui) is also available for scripted in-game screens.

## Mods and modpacks

In **Mods → Browse mods**, switch between **Modrinth** and **CurseForge**. Select a Fabric or Forge target to browse individual mods matched to that instance. Open **Versions** to see project previews and select a specific file.

For CurseForge, open **API key setup**, paste a key from the CurseForge for Studios developer console, and click **Save key**. The launcher keeps the key in Windows Credential Manager. You can also set `ROCKET_CURSEFORGE_API_KEY` before starting the app. CurseForge installations resolve required dependencies and verify downloads with SHA-1 when the publisher makes the files available to third-party launchers. Use Modrinth for the built-in modpack browser.

Create or select a Fabric or Forge instance, then open **Mods → Browse mods**. Click **Install** on a result to choose a compatible release automatically, or use **Versions** to pick one yourself. Required Modrinth dependencies are resolved against the same instance. An external dependency without a Modrinth version must already be in the instance's mods folder. To switch an installed project's release, disable its old JAR in **Installed mods** before installing the replacement. Offline mode keeps local mod files available and pauses catalogue requests and downloads.

Switch the browser to **Modpacks** to search Modrinth projects. Cards include project artwork, descriptions, downloads, update dates, loaders, and supported versions when available. Search results can be sorted and filtered by Minecraft version, loader, category, compatibility, and featured or popular collections. Filters can be combined, removed individually, or cleared. Open a pack to view its description, gallery, versions, dependencies, and installation choices.

Select a pack version, Minecraft version, and supported loader, then choose **Install in new instance**. The browser supports Fabric and supported Forge installations; Quilt and NeoForge can be used as browsing filters. Installation reports progress and errors, checks downloaded files, and creates a separate instance. To try a newer pack release, install it into a new instance and retain the old instance and worlds; updates do not migrate saves automatically.

Use **Installed mods** to filter the list, open the mods folder, or select and enable or disable JARs after closing Minecraft. Disabling a mod keeps its file as `.jar.disabled`.

Rocket Launcher keeps a copy of an instance’s original server and keybind files before applying shared values. Server and keybind sync allow one running instance at a time. **Repair files** is available as a separate action on the Launch Pad and in the instance manager.

## Multiplayer and shared worlds

The **Multiplayer** page stores server names and addresses, favorites, recent joins, and an optional preferred instance. Check a server's version, player count, and latency, copy its address, or join directly through Minecraft Quick Play when the selected Minecraft version supports it. On older versions, copy the address and use Minecraft's **Multiplayer → Direct Connection**.

For a compatible Fabric instance, **Server version compatibility** installs ViaFabricPlus or ViaFabric when a matching Modrinth build exists. **Share your world** installs e4mc and its required Fabric API when available. Open a single-player world, use **Share World** in the Rocket Client pause menu or Minecraft's **Open to LAN**, and share the address provided by e4mc. e4mc supplies the relay and temporary address. Anyone with that address can attempt to join, and server rules and matching gameplay mods still apply.

## Run locally on Windows 10/11

The **Launcher updates** row in Settings checks the project's GitHub releases, including prereleases, for a newer Windows x64 installer. If the GitHub API cannot be read, the launcher tries the repository's release manifest. Select **Download and install** to stream the installer to disk and verify its size and SHA-256 checksum against the release metadata. Rocket Launcher closes while the installer applies the update, then reopens. Close Minecraft before updating.

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
| `instance-favorites.json` | Favorite instance IDs |
| `instances\<UUID>\` | Default isolated game directories |
| `instances\Rocket\rocket-pack-…\` | Default Rocket Pack game directories |
| `cache\versions.json` | Validated Mojang catalogue with fetch timestamp |
| `cache\minecraft\` | Shared version metadata, client JARs, libraries, assets, extracted natives, and installation records |
| `instance-sync\rocket-client.properties` | Shared Rocket Client module preferences and HUD positions |
| `logs\game\<instance-ID>\<session-ID>\` | Minecraft stdout and stderr for sessions started with launcher v0.1.20 or newer |
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
  pages/               Launch Pad, Instances, Multiplayer, Mods, Downloads, Settings
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
