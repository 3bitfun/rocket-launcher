# Rocket Launcher progress

The Windows x64 NSIS installer built successfully. The packaged executable opened a Rocket Launcher window in a Windows startup smoke test. This build is unsigned. The public release roadmap still includes Forge, NeoForge, modpack installation, automatic updates, and crash reporting with privacy controls.

An isolated Windows smoke test installed Minecraft 1.21.1, Fabric, Iris, and Sodium, started the client on Java 21 using a debug-only local profile, and reached renderer initialization. The test closed Minecraft after it created the block texture atlas.

The Mods page now browses Modrinth using the selected Fabric instance's Minecraft version. The version picker lists matching downloads, and the native installer rechecks compatibility and file integrity before publishing the selected JAR and its required dependencies. It preserves existing JARs when a conflict is found. Offline Mode reads the cached Minecraft catalogue and leaves local mods available without browsing or downloading.

The live Modrinth test installed Sodium and Iris on Minecraft 1.21.1 in temporary instances and resolved Iris's required Sodium dependency. It also checked incompatible release rejection and preservation of a conflicting local file. The production frontend build, 11 UI tests, 32 local Rust tests, and strict Clippy passed.

Phase 3 connects Microsoft browser sign-in, Xbox and Minecraft entitlement verification, account selection, and verified offline sessions to Settings and IGNITE. Sign out removes saved credentials for the selected Microsoft account. A live sign-in and gameplay test still requires an approved client registration and an account that owns Minecraft Java Edition.

The working tree adds Fabric instances, compatible loader selection, and a Fabric installation layer on top of the shared vanilla cache. The app checks readiness quickly, then verifies hashes for both layers before launching. A failed integrity check offers file repair. A saved Fabric version remains selectable if the loader service is unavailable.

Fabric validation passed on Windows with Minecraft 1.21.1 in an isolated cache. The live test installed both layers and confirmed that full verification rejects a same-size change to a Fabric JAR. The frontend build, 10 UI tests, 32 local Rust tests, and strict Clippy passed. Gameplay was outside this installation test.

The OAuth URL and callback unit tests passed; a temporary Windows credential was written, read and deleted successfully. Strict Rust Clippy passed. Live Microsoft authentication and the token exchanges remain untested because no configured public client was supplied.

For desktop OAuth setup, see the Microsoft sign-in instructions in README.md.

Phase 2 implements real vanilla Minecraft installation on Windows, Java compatibility checks, and a process launcher behind an authenticated-session boundary. The existing Launch Pad, instance cards and Downloads page now show backend readiness and progress.

- Official Mojang version metadata and inheritance, Windows and architecture rules, streaming downloads, SHA-1 and size verification, native extraction, and persistent installation records. Cache hits are verified and reused; interrupted transfers retry from the last complete file.
- Java search covers explicit paths, environment settings, common Windows installations and launcher runtime directories. The detected Java major and architecture must match the game requirements. Manual selection uses the existing instance editor and settings.
- Modern and legacy arguments, native and classpath paths, per-instance working directories and RAM, duplicate-launch protection, captured output and exit status. A Windows job object ties supervised game processes to the launcher lifecycle.
- Local fonts use the SIL Open Font License; notices ship with the app.

Validation: 17 Rust tests and nine UI tests passed, with formatting, strict Clippy, TypeScript and the Vite production build clean. Live tests downloaded Minecraft **1.12.2**, **1.21.1**, and **26.3** into isolated caches, verified their files without network requests, detected a removed test client JAR, and repaired it. The live catalogue and x64 Java 21 discovery checks passed. The open desktop app showed the installation action and matching instance-card status without changing the user's instances.

The optimized standalone Windows build passed (`npm run desktop:build`). The release executable was built; the native UI smoke check used the development app.

Actual Minecraft gameplay was not tested. Phase 2 has no provider that can issue an authenticated session, so IGNITE cannot start Minecraft. Automatic Java downloads and authenticated offline play are also unavailable. Legacy per-instance resource copies are prepared at launch; the shared assets they are copied from are verified during installation.

Next: register an approved Microsoft public client, implement Microsoft OAuth and Xbox Live/XSTS exchanges, obtain a Minecraft Services token, verify the Java entitlement and player profile, and store and refresh credentials securely. Connect the native session provider, then verify an owned account can start Minecraft and report a clean exit. Add a separately validated session policy before enabling offline gameplay.
