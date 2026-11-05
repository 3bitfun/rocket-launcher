# Phase 1 validation

Validated on Windows on 5 November 2026 with Node 24.19.0, npm 11.17.0, Rust/Cargo 1.97.1, and Visual Studio 2022 C++ tools.

## Automated checks

- TypeScript project check and Vite production build.
- Eight frontend tests: instance submission and snapshot filtering, unavailable catalogue, preserving drafts on failure, visible errors inside native dialogs, retry behavior, disabled game launching, settings persistence requests, and actionable error messages.
- Eight local Rust tests: settings save/reopen and invalid input, corrupt configuration preservation, path/RAM validation, instance create/edit/duplicate/select/remove/reopen, preserved game files, local mod enumeration, manifest parsing/cache lookup, and malformed/untrusted manifest rejection.
- Explicit live integration test against Mojang's official manifest endpoint, followed by a cached read. The network test is ignored in the default Rust test suite and must be requested explicitly.
- Rust formatting, `cargo check`, and Clippy with warnings denied.
- npm dependency audit.
- Windows standalone release executable build (`npm run desktop:build`).

## Native application smoke check

The actual Tauri Windows application was launched and inspected through Windows UI automation. The Launch Pad rendered, the Rust bridge returned local configuration, and the UI displayed the official version catalogue fetched by Rust. The instance creation dialog displayed the fetched latest release, RAM controls, and native path selection controls.

The first development run exposed an outdated Vite optimized-dependency cache. App and test caches are now separated, and required runtime dependencies are explicitly included in pre-bundling. Both browser startup and the native application rendered after the fix. The browser intentionally shows a native-app requirement, not a replacement backend.

Instance persistence and removal were tested in isolated temporary directories through Rust tests; native UI form behavior was tested using the frontend suite. A full manual UI create/restart/delete sequence was not performed because the user began interacting with the open launcher. No user-created instances or active drafts were removed for testing.

## Limits

Minecraft installation and gameplay, authentication, Java runtime compatibility, mod loaders, installer distribution, auto-updating, and crash reporting are outside Phase 1 and were not tested. The standalone release executable was built; its own UI was not separately launched while the user was interacting with the development app. An NSIS installer and code signing were not validated for this milestone. Windows 10 was not separately exercised on another machine.
