# Midnight Launcher

Independent Minecraft: Java Edition desktop launcher built with Tauri 2, React, TypeScript, and Rust.

## Platforms

Desktop builds and Minecraft Java installation target macOS Apple Silicon and Windows x64. Windows uses x64 Java and Windows native libraries. Linux desktop builds have not been verified.

### Build on Windows

Install Rust 1.99+ with the MSVC target, Microsoft C++ Build Tools with the Windows SDK, and Node.js 22. Ensure Cargo is on PATH, then run:

```powershell
npm ci
npm run build:win
```

The executable is written to `src-tauri/target/x86_64-pc-windows-msvc/release/midnight-launcher.exe`; the NSIS installer is under that directory's `bundle/nsis/`.

The Windows installer uses Midnight's rose and purple palette, mascot, pixel wordmark, rounded buttons and dark progress/details pages. Welcome, folder selection, completion and removal support Russian and English; NSIS selects the system language, falling back to Russian. Installation is for the current Windows user. The final page offers a desktop shortcut and launching Midnight. Game files and Java are downloaded later, when starting an instance.

The `beforeBundleCommand` builds a small x86 visual DLL for NSIS and renders the bundled artwork from the existing local mascot and licensed Jersey 10 font. Colors come from `src/theme.css`, and native installer translations come from `src/i18n.ts`. This requires the **MSVC x86/x64 C++ build tools** and **Windows SDK** components of Visual Studio Build Tools; no additional runtime is needed on the destination PC. To regenerate only the installer theme, run `npm run build:installer-art`. Sources are in `src-tauri/installer/` and `scripts/render-installer-art.ps1`; generated files stay in the ignored `src-tauri/target/installer/`. Tauri's stock installer continues to handle WebView2, upgrades, registration, shortcuts and uninstallation.

After `npm run build:win`, run `npm run verify:installer:win` to check silent installation into a path with spaces, byte-for-byte executable integrity, reinstallation, uninstallation and preservation of unrelated files. The check builds a temporary installer with its own product name and registry keys; results are saved under `artifacts/installer verification/`.

## Features

- Instance library with a selected-instance launch panel, Java/Horizon filters, search by name/version/loader, and per-instance settings. Removing an entry requires confirmation and keeps its game files on disk.
- Local mod manager with multi-file JAR import, search, enabled/disabled filters, and per-instance mod toggles.
- Native Horizon Rust client integration (pinned Horizon 0.2.97 / Minecraft 26.3).
- Official Vanilla game, libraries, assets, platform natives, and Java 17/21/25 installation with integrity verification.
- Fabric, Forge and NeoForge instances with compatible version lists, pinned loader versions and automatic preparation on launch.
- Microsoft device authorization, Xbox Live, XSTS, ownership and profile checks.
- Russian and English interface, themes, gradients, and responsive navigation.

## Local mods

Open **Mods** in the bottom navigation or **Instance mods** below the selected instance on Home. Choose a Minecraft Java instance and add one or more `.jar` files (up to 512 MiB each). The manager copies valid JAR archives into `instances/<id>/mods/`; original files are left intact. Existing enabled or disabled files with the same name are never overwritten.

Switches disable mods by appending `.disabled` and enable them by removing that suffix. Files remain in the instance folder. Use **Mods folder** to open it in your system file manager and **Refresh mod list** after external changes. Stop the game before importing or toggling mods.

Select Fabric, Forge or NeoForge in instance settings. The desktop app fetches compatible loader versions when you change Minecraft or the loader; the chosen version is saved with the instance. Forge uses the modern installer format for Minecraft 1.18+ (Forge 38+); NeoForge uses the `net.neoforged:neoforge` releases for Minecraft 1.20.2+ and the 26.x releases. Legacy Forge and NeoForge's 1.20.1 fork are not supported.

On first launch, Midnight downloads the official installer, checks SHA-1 and size, verifies its libraries and runs only its client processors with the selected Java. Generated files are isolated by loader, Minecraft version and loader version; subsequent starts recheck their SHA-256 receipt. Preparation supports progress and cancellation. Manually imported mods must match both Minecraft and the loader; their dependencies are not resolved automatically. Horizon does not support Java mods. Browser preview allows manual loader version entry without native installation.

### Modrinth and CurseForge catalogs

The **Mods** section has **Installed**, **Modrinth**, and **CurseForge** tabs. Select an instance, search the catalog, open a mod card, choose a release and install. Results and releases match the instance's Minecraft version and its Fabric, Forge or NeoForge loader. Vanilla instances can browse the Fabric catalog and must choose a supported loader before installing. Required dependencies are resolved automatically. Optional dependencies are not installed.

The catalog opens on Modrinth, with official project icons, quick search suggestions and a compact instance selector. On wider windows, version selection and installation stay beside the catalog; smaller windows place the installation panel above the results. Icons are accepted only from the providers' dedicated HTTPS image hosts and fall back to project initials when unavailable.

Modrinth works without credentials. CurseForge requires your own API key from the [official developer console](https://console.curseforge.com/). Connect it in the CurseForge tab. The launcher validates the key and keeps it only in Rust memory until disconnect or app exit; it is not saved to localStorage or disk and is not returned to the interface. CurseForge downloads also authenticate with the key in a sensitive request header. Author restrictions and missing download URLs are respected: open the official project page and import those files manually.

Downloads use provider-specific HTTPS host allowlists, bounded redirects within the same provider, exact sizes and SHA-1 (plus SHA-512 when supplied). Catalog files stream into a temporary folder on disk with byte progress and cancellation. The complete dependency set is verified before publication; large JAR contents are not buffered together in memory. Cancellation or a failed commit cleans up staged downloads and rolls back files created by that attempt. Existing files are not overwritten. Identical archives already present under another filename are reused; disabled archives stay disabled. Conflicting mod IDs from Fabric JSON or Forge/NeoForge TOML metadata are rejected. These checks do not replace full runtime compatibility checks against every installed mod.

Limits for individual mod installation: 64 dependency files, 512 MiB per file and 2 GiB per installation. Automated mod updates are not included. Real catalog verification is available via `cargo run --manifest-path src-tauri/Cargo.toml --release --example verify_catalog -- <separate-test-directory> [project-slug] [minecraft-version] [loader]`; on Windows, the example may require the same standard Tauri manifest described in `VERIFICATION.md`.

## Modrinth modpacks

Open **Instances → Modrinth modpacks**, browse Fabric, Forge and NeoForge packs, choose a release, and click **Install modpack**. Each download creates and selects a new instance. Minecraft and the exact loader version come from the pack's manifest; Minecraft, Java and loader libraries are prepared on first launch. Existing instances are kept separate.

The installer reads the official [`.mrpack` format](https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack), verifies the archive and every downloaded file using its size, SHA-1 and SHA-512, and applies author settings from `overrides` followed by `client-overrides`. Dedicated-server files and `server-overrides` are skipped. Optional client files are included by default; disable **Optional mods** before installing to skip them. Progress and cancellation are available during installation. Files are staged in a fresh directory; only a complete installation appears as a new instance. Pack provenance is saved in launcher state and `.midnight-modpack.json` inside the instance.

Supported packs use Fabric, modern Forge or NeoForge and downloads from `https://cdn.modrinth.com/data/`. Packs requiring mixed loaders, legacy Forge, Quilt or files from external hosts are rejected with an explanation. Modpack updates and local `.mrpack` import are not included. Limits: 2,048 listed files, 128 MiB per downloaded file or archive, 2 GiB total downloads, and 512 MiB uncompressed archive contents. Real installation verification uses `cargo run --manifest-path src-tauri/Cargo.toml --release --example verify_modpack -- <separate-test-directory> [project-id-or-slug]`. This check downloads a modpack; it does not launch Minecraft or verify authenticated gameplay.

## Horizon (Rust)

Open **Instances → Horizon · Rust** to create and select a Rust instance, or reuse the one you already configured, then Launch. You can also choose **Horizon · Rust** in the instance editor.
Midnight downloads and starts `horizon-client` directly; the Horizon launcher is not required.
The client runs in its own native window. It is not embedded in the launcher's WebView.

- Microsoft mode verifies Minecraft ownership before launch. Offline mode requires a previously verified account, as described below. The first launch downloads the pinned game archive from [Horizon releases](https://github.com/horizon-rust/horizon/releases/tag/v0.2.97) and checks its built-in SHA-256 and size before extraction. Updates require a code change to the pinned version and checksums.
- Original Minecraft client/server jars, libraries and sounds come from official Mojang hosts, with size/SHA-1 verification. The installer uses a version JSON explicitly accepted by Horizon 0.2.97, so later Mojang republishes do not silently change its inputs. The release's `horizon-prepare` imports them locally. Resources are shared within the pinned release; worlds/settings go to `instances/<id>/horizon/`, separate from Java worlds.
- Java 25 is needed for resource preparation, not to run the prepared Rust game. Midnight installs the appropriate Java automatically on Windows x64 and Apple Silicon. On Linux install Java 25 and set the per-instance Java executable path, `JAVA_HOME`, or PATH. A platform system `tar` is required for Horizon extraction. Windows/Linux need a Vulkan-capable GPU/driver.
- The native Horizon release supports Windows x64, Linux x64 and Apple Silicon macOS. Java instances support Windows x64 and Apple Silicon; Linux Java installation remains unsupported.
- Fabric/Forge Java mods and the JVM memory limit do not apply to Horizon. In Microsoft mode Midnight passes its authenticated Minecraft account to the native client using Horizon 0.2.97's single-use session-file protocol. The file is created with a protected current-user-only Windows ACL or Unix mode 0600 and is deleted as soon as the client reads it. Tokens are excluded from IPC, argv, environment variables and captured game output. Failed/cancelled starts clean up the handoff; expired files left by a crash are cleaned up on the next launch. Explicitly selected offline mode passes only the local nickname and cannot join authenticated servers.
- Cancellation, process status, Stop, launcher minimize/restore and the instance console use the existing Midnight controls. Preparation failures are written to that console.

Full Horizon resource preparation and the native Windows title screen have been verified against an existing Minecraft 26.3 installation, including Vulkan rendering on an RTX 4060 and a two-minute process check. Microsoft session handoff is covered with a private-file child-process test; authenticated server joining still requires a real-account check. The Minecraft API sign-in issue described below can still prevent the owning-account gate from completing. See `VERIFICATION.md`.

An isolated native verification uses `cargo run --manifest-path src-tauri/Cargo.toml --release --example verify_horizon -- <test-output-directory> <existing-minecraft-directory> <26.3-server-jar> [--launch]`. It prepares resources from your own copy; the optional launch checks the title screen without restoring credentials or claiming authenticated gameplay. Prepared game resources are private local files and are not included in the launcher EXE or installer.

## Settings

Settings has three sections: Appearance, Game & Java, and Launcher. All twelve palettes have previews; language, gradients and reduced motion apply immediately. Memory offers 2/4/6/8 GB presets plus the 1–16 GB slider. An empty Java path selects automatic installation. Instance overrides are accessible from the game section, and the launcher section includes minimize/restore behavior and the current instance's console. Changes use the existing local save mechanism. Section navigation moves above the cards in narrower windows.

## Offline profiles

Offline profiles for Java and Horizon unlock only after a successful Microsoft sign-in, Minecraft Java ownership check and valid player profile. The proof stays in the system credential store across restarts; a username in browser storage cannot grant access. Offline selection and launch are checked in Rust, including a second check after lengthy preparation. Selecting offline mode does not refresh Microsoft tokens; initial installation still needs internet. Signing out deletes the saved account, clears the selected offline username and locks offline mode again.

The Accounts page displays the previously verified account and allows signing out without an online refresh. Its local account lookup runs when opening Accounts or explicitly selecting/launching an offline profile; startup inspection does not read the credential store. Restore saved sign-in sits below the Microsoft button with a 12 px gap.

## Minecraft API access



API access is requested to complete Microsoft sign-in, verify Minecraft Java Edition ownership, and retrieve player profiles. Credentials are entered on Microsoft websites; Midnight does not collect Microsoft passwords. Refresh tokens use macOS Keychain.

The owner reported Mojang AppID approval on October 30, 2026, for Midnight's built-in Client ID. A previous build still returned HTTP 403 during the Minecraft token exchange. Midnight now distinguishes explicit registration rejection from a generic denial, supports the standard Xbox exchange when the launcher route gives an unspecified denial, and exposes public-only request diagnostics under Accounts. Successful authentication and authenticated gameplay still require end-to-end verification with a real account. See `MICROSOFT_SETUP.md`.

## Development status

Windows x64 EXE/NSIS and Apple Silicon app/DMG builds have been verified locally. On Windows, Vanilla 1.21.1 installation, automatic Java 21 x64 installation and an offline game window were verified, including the create-world screen. 13 frontend tests and 27 Rust tests passed. Microsoft-authenticated gameplay and an in-world game session remain unverified. See `VERIFICATION.md` for the scope and earlier platform checks.

This repository includes the launcher source code. A snapshot of existing Windows binaries and verification materials is available in [backup/](backup/README.md).

Midnight is not an official Minecraft product and is not approved by or associated with Mojang or Microsoft. A legitimate Minecraft Java Edition entitlement is required; there is no license bypass.
