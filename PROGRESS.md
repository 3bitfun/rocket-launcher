# Rocket Launcher progress

Phase 3 is started but incomplete. Native PKCE, loopback callback validation, Microsoft authorization URL generation, code redemption and token refresh primitives are implemented. A Windows Credential Manager wrapper is present for UUID-scoped launcher credentials. These pieces are not yet connected to the UI or an account service, and no verified account metadata is stored. Sign-in, account management, online gameplay and offline gameplay remain unavailable. The existing authentication boundary stays closed.

Xbox Live and XSTS token exchange, Minecraft Services login, entitlement checking, and profile validation have now been added as native primitives, together with a credential-store-backed verified identity record. These have not been integrated with the login flow, tested against a registered application, or connected to launch; no account is yet cached by the application. Offline launch remains unavailable.

The OAuth URL and callback unit tests passed; a temporary Windows credential was written, read and deleted successfully. Strict Rust Clippy passed. Live Microsoft authentication and the token exchanges remain untested because no configured public client was supplied.

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
