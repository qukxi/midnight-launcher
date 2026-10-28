// Package the signed local ARM64 app without requiring Finder automation.
import { mkdtempSync, mkdirSync, symlinkSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
if (process.platform !== 'darwin') throw new Error('macOS packaging requires a Mac.');
const root = fileURLToPath(new URL('../', import.meta.url));
const bundle = join(root, 'src-tauri/target/aarch64-apple-darwin/release/bundle');
const app = join(bundle, 'macos/Midnight Launcher.app');
const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
const image = join(bundle, `dmg/Midnight Launcher_${version}_aarch64.dmg`);
execFileSync('codesign', ['--verify', '--deep', '--strict', app], { stdio: 'inherit' });
const stage = mkdtempSync(join(tmpdir(), 'midnight-package-'));
try {
  execFileSync('ditto', [app, join(stage, 'Midnight Launcher.app')], { stdio: 'inherit' });
  symlinkSync('/Applications', join(stage, 'Applications'));
  mkdirSync(dirname(image), { recursive: true });
  execFileSync('hdiutil', ['create', '-ov', '-volname', 'Midnight Launcher', '-srcfolder', stage, '-format', 'UDZO', image], { stdio: 'inherit' });
  execFileSync('hdiutil', ['verify', image], { stdio: 'inherit' });
  console.log(`Created ${image}`);
} finally {
  rmSync(stage, { recursive: true, force: true });
}
