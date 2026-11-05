# Rocket Launcher progress

Phase 3 is started but incomplete. A native PKCE attempt and strict loopback callback parser are implemented and tested independently; they are not yet connected to the UI or a token exchange. No credentials or verified account metadata are stored yet. Sign-in, account management, online gameplay and offline gameplay remain unavailable. The existing authentication boundary stays closed.

For eventual desktop OAuth setup, register a public client that supports personal Microsoft accounts in Microsoft Entra, add the Mobile and desktop platform with the `http://localhost` redirect URI, and configure the application client ID locally. No client secret should be embedded. Xbox and Minecraft Services access and the Minecraft entitlement flow still need to be implemented and verified with an appropriately configured application and an account that owns Minecraft Java Edition.

Phase 2 implements real vanilla Minecraft installation on Windows, Java compatibility checks, and a process launcher behind an authenticated-session boundary. The existing Launch Pad, instance cards and Downloads page now show backend readiness and progress.

- Official Mojang version metadata and inheritance, Windows and architecture rules, streaming downloads, SHA-1 and size verification, native extraction, and persistent installation records. Cache hits are verified and reused; interrupted transfers retry from the last complete file.
- Java search covers explicit paths, environment settings, common Windows installations and launcher runtime directories. The detected Java major and architecture must match the game requirements. Manual selection uses the existing instance editor and settings.
- Modern and legacy arguments, native and classpath paths, per-instance working directories and RAM, duplicate-launch protection, captured output and exit status. A Windows job object ties supervised game processes to the launcher lifecycle.
- Local fonts use the SIL Open Font License; notices ship with the app.

Validation: 17 Rust tests and nine UI tests passed, with formatting, strict Clippy, TypeScript and the Vite production build clean. Live tests downloaded Minecraft **1.12.2**, **1.21.1**, and **26.3** into isolated caches, verified their files without network requests, detected a removed test client JAR, and repaired it. The live catalogue and x64 Java 21 discovery checks passed. The open desktop app showed the installation action and matching instance-card status without changing the user's instances.

The optimized standalone Windows build passed (`npm run desktop:build`). The release executable was built; the native UI smoke check used the development app.

Actual Minecraft gameplay was not tested. Phase 2 has no provider that can issue an authenticated session, so IGNITE cannot start Minecraft. Automatic Java downloads and authenticated offline play are also unavailable. Legacy per-instance resource copies are prepared at launch; the shared assets they are copied from are verified during installation.

Next: register an approved Microsoft public client, implement Microsoft OAuth and Xbox Live/XSTS exchanges, obtain a Minecraft Services token, verify the Java entitlement and player profile, and store and refresh credentials securely. Connect the native session provider, then verify an owned account can start Minecraft and report a clean exit. Add a separately validated session policy before enabling offline gameplay.
