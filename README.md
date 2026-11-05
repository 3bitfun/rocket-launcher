# Rocket Launcher

**Beyond the Built Limit.** A native Windows Minecraft Java Edition launcher built with Tauri 2, Rust, React, TypeScript, Vite, Tailwind CSS, Motion, and Lucide.

## Current milestone: Phase 1

This is a functional **desktop foundation**, not yet a playable Minecraft launcher.

- Launch Pad, Instances, Mods, Downloads, and Settings navigation.
- Create, rename, edit, select, duplicate, and remove instances.
- Separate UUID-named game directories; configurable Minecraft version, RAM, and Java executable path.
- Native directory and executable pickers.
- Atomic Rust-managed configuration persistence and single-instance application behavior.
- Live official Mojang version catalogue, release/snapshot filters, local cache, bounded requests, timeout, one retry, and explicit failure/fallback states.
- Dark, light, and system themes; custom accent colours; animation and reduced-motion preferences.
- Actual `.jar` file listing from each instance's mods directory.
- Local rotating structured logs and an in-app log viewer.
- Keyboard-accessible controls, native modal focus management, responsive desktop layout.

**Unavailable:** game installation, launch, Java compatibility detection, Microsoft authentication, authenticated offline play, mod loaders, Modrinth downloads, updates, and crash reporting. The UI labels these explicitly. No downloads, login sessions, speeds, or game launches are simulated.

## Run locally on Windows 10/11

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

Build a standalone executable:

```powershell
npm run desktop:build
```

Output: `src-tauri\target\release\rocket-launcher.exe`. This executable embeds the frontend and does not need Vite. An unsigned NSIS installer can also be generated with `npm run tauri -- build`; signing, updater configuration, and distribution hardening remain Phase 5 work.

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
```

Rust tests exercise configuration replacement/reopening, corruption preservation, validation, the instance lifecycle, file preservation, local mod listing, manifest parsing, and cache lookup. Frontend tests exercise submission, error/retry states, preferences, and feature availability. Mock data exists only under `src/test`.

See [the Phase 1 validation record](docs/VALIDATION.md) for the checks actually performed and their limits.

## Local data and behavior

The application uses Tauri's local application data directory, normally `%LOCALAPPDATA%\app.rocketlauncher.desktop` on Windows. The exact paths appear in Settings.

| Path | Contents |
| --- | --- |
| `launcher.json` | Versioned settings, instance configurations, and active selection |
| `instances\<UUID>\` | Default isolated game directories |
| `cache\versions.json` | Validated Mojang catalogue with fetch timestamp |
| `logs\launcher.log.*` | Daily JSON logs, at most seven files |

Configuration writes use a same-directory temporary file, flush to disk, then atomically replace the destination. Invalid configuration is preserved rather than silently reset. Back up `launcher.json` before manual edits. A different launcher schema version is rejected.

Removing an instance **only removes its configuration**; game directories are preserved. Duplication copies settings into a **new empty directory**; it does not copy saves, mods, or game files. Existing directories cannot be relocated through the editor. Changing the default directory affects newly created instances only.

RAM is configurable between 1 and 64 GB in 256 MB increments. This phase validates ranges, not available physical memory. Java overrides must point to an existing executable; runtime version and architecture checks are Phase 2. Leave the override blank to inherit the launcher default when launching is implemented.

The catalogue is reused for one hour, then refreshed. Refresh always tries the network. Valid cached versions remain available when Mojang cannot be reached, with a visible warning. Without any valid cache, creating an instance requires the first successful fetch. Existing instances can be renamed, selected, removed, and edited without changing their Minecraft version while offline. Historical alpha/beta versions are intentionally excluded.

Instances display **Requires download** when their local directory exists, or **Missing game directory** if it has disappeared. This build never certifies an instance as installed or offline-ready. A local workspace does not authenticate an account or grant Minecraft access.

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
  minecraft/versions.rs Official manifest fetching, validation, caching
  models.rs            Serializable configuration/domain models
  storage/             Validation and atomic persistence
```

The frontend has no filesystem or shell plugin permissions. HTTP requests are performed by Rust against a fixed HTTPS Mojang endpoint; response sizes are capped and metadata hosts validated. No secrets, authentication tokens, telemetry, or account credentials are collected in Phase 1. Native command arguments are revalidated in Rust.

## Next milestone: Phase 2

Implement the complete installation and launch pipeline: version metadata inheritance, rule-aware libraries, assets, native extraction, hash verification, cancellable/retryable download jobs with byte-based progress, Java discovery/version/architecture compatibility, and modern/legacy argument resolution. Build launch/process supervision behind an account/entitlement boundary so Phase 2 cannot expose an ownership bypass. Phase 3 adds legitimate Microsoft OAuth, Xbox/Minecraft Services authentication, ownership verification, OS-backed token storage, refresh, and authenticated offline behavior.

Microsoft app registration is not needed for Phase 1. No client ID or secret is invented or embedded. Exact public-client registration/setup instructions will accompany Phase 3's actual authentication implementation.

Rocket Launcher is an independent project, not an official Minecraft product or approved by Mojang or Microsoft.
