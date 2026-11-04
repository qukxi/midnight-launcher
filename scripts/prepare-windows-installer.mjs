import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// NSIS runs a 32-bit host even when it installs the 64-bit launcher.
// Keep its small, purely visual DLL independent of Rust and the installed app.
if (process.platform !== 'win32') process.exit(0);

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.join(root, 'src-tauri', 'target', 'installer');
mkdirSync(output, { recursive: true });

const { default: typescript } = await import('typescript');
const translations = typescript.transpileModule(readFileSync(path.join(root, 'src', 'i18n.ts'), 'utf8'), {
  compilerOptions: { module: typescript.ModuleKind.ESNext, target: typescript.ScriptTarget.ES2022 },
}).outputText;
const { installerStrings } = await import(`data:text/javascript;base64,${Buffer.from(translations).toString('base64')}`);
function nsisText(value) {
  return value.replaceAll('$', () => '$$').replaceAll('"', () => '$\\"').replace(/\r\n|\r|\n/g, () => '$\\r$\\n');
}
for (const [index, language] of ['Russian', 'English'].entries()) {
  const languageLines = ['; Generated from src/i18n.ts. Do not edit.'];
  for (const [key, values] of Object.entries(installerStrings)) {
    if (!/^Midnight[A-Za-z0-9]+$/.test(key)) throw new Error(`Invalid installer translation key: ${key}`);
    languageLines.push(`LangString ${key} \${LANG_${language.toUpperCase()}} "${nsisText(values[index])}"`);
  }
  writeFileSync(path.join(output, `strings-${language}.nsh`), languageLines.join('\n') + '\n', 'utf8');
}

// Share the default palette with the launcher rather than maintaining a second
// set of brand colors in native code or generated bitmap artwork.
const themeCss = readFileSync(path.join(root, 'src', 'theme.css'), 'utf8');
const rootTokens = themeCss.match(/:root\s*\{([^}]+)\}/)?.[1];
if (!rootTokens) throw new Error('The default :root palette is missing from src/theme.css.');
const tokenNames = {
  background: 'midnight', surface: 'surface', deep: 'surface-deep', rose: 'rose',
  roseLight: 'rose-light', ink: 'ink', muted: 'muted', border: 'line',
};
const palette = {};
for (const [name, token] of Object.entries(tokenNames)) {
  const hex = rootTokens.match(new RegExp(`--${token}:\\s*#([a-f0-9]{6}(?:[a-f0-9]{2})?)\\s*;`, 'i'))?.[1];
  if (!hex) throw new Error(`Installer color --${token} must be a hex color in src/theme.css.`);
  let channels = [0, 2, 4].map(offset => parseInt(hex.slice(offset, offset + 2), 16));
  if (hex.length === 8) {
    const alpha = parseInt(hex.slice(6), 16) / 255;
    const base = [1, 3, 5].map(offset => parseInt(palette.background.slice(offset, offset + 2), 16));
    channels = channels.map((channel, index) => Math.round(channel * alpha + base[index] * (1 - alpha)));
  }
  palette[name] = '#' + channels.map(channel => channel.toString(16).padStart(2, '0')).join('');
}
writeFileSync(path.join(output, 'palette.json'), JSON.stringify(palette, null, 2) + '\n', 'utf8');
const colorLines = ['// Generated from src/theme.css. Do not edit.'];
for (const [name, hex] of Object.entries(palette)) {
  const channels = [1, 3, 5].map(offset => parseInt(hex.slice(offset, offset + 2), 16));
  colorLines.push(`constexpr COLORREF ${name} = RGB(${channels.join(', ')});`);
}
writeFileSync(path.join(output, 'midnight-colors.h'), colorLines.join('\n') + '\n', 'utf8');
writeFileSync(path.join(output, 'colors.nsh'), Object.entries(palette)
  .map(([name, hex]) => `!define MIDNIGHT_${name.toUpperCase()} "${hex.slice(1)}"`).join('\n') + '\n', 'utf8');

function newestDirectory(parent, pattern) {
  const version = readdirSync(parent, { withFileTypes: true })
    .filter(entry => entry.isDirectory() && pattern.test(entry.name))
    .map(entry => entry.name)
    .sort((a, b) => b.localeCompare(a, undefined, { numeric: true }))[0];
  if (!version) throw new Error(`No supported build tools found in ${parent}`);
  return path.join(parent, version);
}

const programFiles = process.env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)';
const vswhere = path.join(programFiles, 'Microsoft Visual Studio', 'Installer', 'vswhere.exe');
if (!existsSync(vswhere)) throw new Error('Install Microsoft C++ Build Tools with the Windows SDK to build the Midnight installer.');
const visualStudio = execFileSync(vswhere, [
  '-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath',
], { encoding: 'utf8' }).trim();
if (!visualStudio) throw new Error('Microsoft C++ x86/x64 Build Tools are required for the installer theme.');

const msvc = newestDirectory(path.join(visualStudio, 'VC', 'Tools', 'MSVC'), /^\d+\./);
const sdkRoot = path.join(programFiles, 'Windows Kits', '10');
const sdkInclude = newestDirectory(path.join(sdkRoot, 'Include'), /^10\./);
const sdkLib = path.join(sdkRoot, 'Lib', path.basename(sdkInclude));
const compilerDirectory = path.join(msvc, 'bin', 'Hostx64', 'x86');
const environment = {
  ...process.env,
  INCLUDE: [path.join(msvc, 'include'), ...['ucrt', 'shared', 'um'].map(part => path.join(sdkInclude, part))].join(';'),
  LIB: [path.join(msvc, 'lib', 'x86'), ...['ucrt', 'um'].map(part => path.join(sdkLib, part, 'x86'))].join(';'),
  PATH: `${compilerDirectory};${process.env.PATH || process.env.Path || ''}`,
};

execFileSync(path.join(compilerDirectory, 'cl.exe'), [
  '/nologo', '/LD', '/O2', '/MT', '/W4', '/EHsc', '/std:c++17', '/utf-8',
  '/DUNICODE', '/D_UNICODE', '/DWIN32_LEAN_AND_MEAN', '/DNOMINMAX',
  `/I${output}`,
  `/Fo${path.join(output, 'midnight-theme.obj')}`,
  `/Fe${path.join(output, 'midnight-theme.dll')}`,
  path.join(root, 'src-tauri', 'installer', 'theme.cpp'),
  '/link', '/MACHINE:X86', 'user32.lib', 'gdi32.lib', 'comctl32.lib', 'gdiplus.lib', 'dwmapi.lib',
  '/EXPORT:Apply=_Apply@4',
], { cwd: output, env: environment, stdio: 'inherit' });

const config = JSON.parse(readFileSync(path.join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
writeFileSync(path.join(output, 'version.txt'), config.version, 'utf8');
execFileSync('powershell.exe', [
  '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(root, 'scripts', 'render-installer-art.ps1'),
  '-ProjectRoot', root, '-OutputDirectory', output,
], { cwd: root, stdio: 'inherit' });
console.log('Midnight installer theme and artwork are ready.');
