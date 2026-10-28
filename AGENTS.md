# Repository Guidelines

## Project Structure & Module Organization

- `src/` contains React/TypeScript UI, IPC wrappers (`api.ts`), persisted state (`model.ts`), translations (`i18n.ts`), and CSS. Keep theme tokens in `theme.css` and responsive layout in `layout.css`.
- `src-tauri/src/auth/` implements Microsoft/Xbox/Minecraft authentication; `minecraft/` handles manifests, installation, Java, and process launching. `lib.rs` registers Tauri commands.
- `tests/*.test.ts` contains Vitest tests. Rust unit tests live beside implementation; `src-tauri/tests/fixtures/` contains official metadata fixtures. `examples/prepare.rs` under `src-tauri/` exercises real installation.
- `public/` stores local images and fonts. Preserve included licenses. `scripts/package-macos.mjs` creates the ARM64 DMG.

## Build, Test, and Development Commands

Run commands from the project root. Use Node.js 22 LTS, Rust 1.99+, and macOS Command Line Tools for native builds.

- `npm ci` — install locked frontend dependencies.
- `npm run dev` — browser preview; native commands are unavailable.
- `npm run tauri -- dev` — run the desktop application.
- `npm run build` — TypeScript checking and production frontend build.
- `npm test` — run Vitest.
- `cargo fmt --manifest-path src-tauri/Cargo.toml` — format Rust.
- `cargo test --manifest-path src-tauri/Cargo.toml --release --target aarch64-apple-darwin --locked` — run ARM64 Rust tests.
- `npm run build:mac` — build and package the signed local `.app` and `.dmg`.

## Coding Style & Naming Conventions

Use two-space indentation for TypeScript and standard `cargo fmt` formatting for Rust. Use PascalCase for React components/types, camelCase for JavaScript functions, and snake_case for Rust functions/modules. Keep IPC payloads synchronized across languages. Translate interface text through `i18n.ts`. No dedicated lint command is configured.

## Testing Guidelines

Name frontend tests `*.test.ts`; use descriptive Rust `#[test]` functions. Test state migration, platform rules, integrity checks, cancellation, and credential redaction. No numeric coverage threshold exists. Verify responsive navigation visually. Real installation downloads approximately 1 GB; distinguish that check from authenticated gameplay in `VERIFICATION.md`.

## Commit & Pull Request Guidelines

This checkout has no Git history, so no existing commit convention can be established. Use concise imperative subjects, such as `Fix navigation in small windows`. PRs should explain behavior, link relevant issues, report validation and limitations, and include screenshots for UI changes. Update documentation when setup or supported behavior changes.

## Security & Configuration

Never commit passwords, tokens, client secrets, or borrowed Client IDs. Keep refresh tokens in Keychain and access tokens in Rust. Preserve download host restrictions, hash verification, and safe paths. Follow `MICROSOFT_SETUP.md`; do not claim successful authentication without a real configured account.
