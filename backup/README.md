# Backup before reinstalling Windows — 2026-11-04

The repository contains the current source code, lockfiles, local assets, build
scripts, tests, and project documentation.

`windows/` preserves the existing Windows x64 installer and a ZIP containing the
standalone launcher EXE from the local release directory (built on 2026-11-01).
These binaries were copied without rebuilding. `verification/` contains screenshots
and selected local verification reports. SHA-256 checksums for the binary archives
and installer are in `SHA256SUMS.txt`.

To restore development, clone `https://github.com/qukxi/midnight-launcher.git`,
install Node.js 22 and Rust 1.99 or newer, then run `npm ci`. See the root README
for native build prerequisites and platform-specific commands.

Backup-time checks: all 39 frontend tests passed and `npm run build` succeeded.
Rust tests, the native build, and authenticated gameplay were not rerun for this
backup; earlier results and limitations remain in `VERIFICATION.md`.

Downloaded Minecraft/Horizon resources, Java runtimes, dependency/build caches,
temporary logs, and isolated test installations are excluded. They can be
downloaded or generated again. This is a backup of this project, not the whole PC:
saved accounts, Windows credentials, launcher settings, and personal worlds
outside this repository are not included. Sign in again after reinstalling.
