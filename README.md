# Midnight Launcher

Independent Minecraft: Java Edition desktop launcher built with Tauri 2, React, TypeScript, and Rust.

## Platforms

The current development build targets macOS Apple Silicon. Windows and Linux support is planned.

## Features

- Local Minecraft instance management.
- Official Vanilla game, libraries, assets, and ARM64 Java installation with integrity verification.
- Microsoft device authorization, Xbox Live, XSTS, ownership and profile checks.
- Russian and English interface, themes, gradients, and responsive navigation.

## Minecraft API access



API access is requested to complete Microsoft sign-in, verify Minecraft Java Edition ownership, and retrieve player profiles. Credentials are entered on Microsoft websites; Midnight does not collect Microsoft passwords. Refresh tokens use macOS Keychain.

Minecraft Services currently rejects this application. Approval is being requested. Successful authentication and authenticated gameplay have not yet been verified end to end.

## Development status

Apple Silicon app and DMG builds, Vanilla 1.21.1 installation, and ARM64 Java 21 have been verified locally. A JVM probe loaded Minecraft classes, LWJGL, and GLFW without starting gameplay. Six frontend tests and thirteen Rust tests have passed.

This repository currently provides public project information for application review. Source code and releases will be published separately.

Midnight is not an official Minecraft product and is not approved by or associated with Mojang or Microsoft. A legitimate Minecraft Java Edition entitlement is required; there is no license bypass.
