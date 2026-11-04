import type { Instance } from './model';
import { invoke, isTauri } from '@tauri-apps/api/core';
export interface Version { id: string; type: string }
export interface Profile { id: string; name: string }
export interface DevicePrompt { id: string; userCode: string; verificationUri: string; expiresIn: number; interval: number }
export interface PollResult { state: 'pending' | 'connected'; profile: Profile | null; interval: number }
export interface Progress { stage: string; current: number; total: number }
export interface GameStatus { running: boolean; pid: number | null; exit_code: number | null }
export interface InstallReport { version: string; directory: string; javaPath: string }
export interface AuthStatus { configured: boolean; implemented: boolean; message: string }
export interface AuthDiagnostics { clientId: string; attempts: { endpoint: string; httpStatus: number; errorCode: string | null }[] }
export const desktop = isTauri();
function call<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop) return Promise.reject(new Error('Эта функция доступна в десктопном приложении. Откройте его командой npm run tauri dev.'));
  return invoke<T>(name, args);
}
export const listVersions = () => call<Version[]>('list_versions');
export const installGame = (version: string, javaPath: string, loader?: Instance['loader']) => call<InstallReport>('install_game', { version, javaPath, loader: loader || null });
export const launchGame = (instanceId: string, version: string, javaPath: string, memory: number, offlineName?: string, loader?: Instance['loader'], minimizeOnLaunch = false, restoreOnExit = true, engine: Instance['engine'] = 'java') => call<GameStatus>('launch_game', { instanceId, version, javaPath, memory, offlineName: offlineName || null, loader: loader || null, minimizeOnLaunch, restoreOnExit, engine });
export const gameStatus = () => call<GameStatus>('game_status');
export const stopGame = () => call<void>('stop_game');
export const cancelOperation = () => call<void>('cancel_operation');
export const authStatus = (clientId: string) => call<AuthStatus>('auth_status', { clientId });
export const authDiagnostics = () => call<AuthDiagnostics | null>('auth_diagnostics');
export const beginLogin = (clientId: string) => call<DevicePrompt>('begin_login', { clientId });
export const pollLogin = (id: string) => call<PollResult>('poll_login', { id });
export const openLogin = (id: string) => call<void>('open_login', { id });
export const cancelLogin = () => call<void>('cancel_login');
export const getAccount = () => call<Profile | null>('get_account');
export const getOfflineAccount = () => call<Profile | null>('get_offline_account');
export const selectOfflineProfile = (name: string) => call<Profile>('select_offline_profile', { name });
export const restoreAccount = () => call<Profile>('restore_account');
export const signOut = () => call<void>('sign_out');

export const readGameLog = (instanceId: string) => call<string>('read_game_log', { instanceId });

export const loaderVersions = (version: string, kind: import('./model').LoaderKind = 'fabric') => call<string[]>('loader_versions', { version, kind });

export interface ModFile { fileName: string; size: number; enabled: boolean }
export const maxModSize = 512 * 1024 * 1024;
export const listMods = (instanceId: string) => call<ModFile[]>('list_mods', { instanceId });
export const setModEnabled = (instanceId: string, fileName: string, enabled: boolean) => call<void>('set_mod_enabled', { instanceId, fileName, enabled });
export const openModsFolder = (instanceId: string) => call<void>('open_mods_folder', { instanceId });
export async function importMod(instanceId: string, file: File): Promise<void> {
  if (!desktop) return call<void>('import_mod');
  if (!/\.jar$/i.test(file.name)) throw new Error('MODS_JAR_REQUIRED');
  if (file.size > maxModSize) throw new Error('MODS_TOO_LARGE');
  // Binary IPC avoids expanding JAR contents into a large JSON array.
  const nameHeader = JSON.stringify(file.name).replace(/[^\x20-\x7e]/g, ch => `\\u${ch.charCodeAt(0).toString(16).padStart(4, '0')}`);
  await invoke<void>('import_mod', new Uint8Array(await file.arrayBuffer()), { headers: { 'x-instance-id': instanceId, 'x-mod-name': nameHeader } });
}

export type CatalogSource = 'modrinth' | 'curseforge';
export type CatalogSort = 'relevance' | 'downloads' | 'updated' | 'newest';
export interface CatalogProject { id: string; slug: string; title: string; description: string; author: string; downloads: number; canDownload: boolean; iconUrl?: string | null }
export interface CatalogPage { projects: CatalogProject[]; total: number; hasMore: boolean }
export interface CatalogRelease { id: string; name: string; number: string; published: string; channel: string; size: number; canDownload: boolean }
export interface CatalogInstallResult { installed: number; reused: number; disabled: number }
export const catalogStatus = () => call<boolean>('catalog_status');
export const connectCurseForge = (key: string) => call<void>('connect_curseforge', { key });
export const disconnectCurseForge = () => call<void>('disconnect_curseforge');
export const searchCatalog = (source: CatalogSource, query: string, gameVersion: string, offset = 0, sort: CatalogSort = 'downloads', loaderKind: import('./model').LoaderKind = 'fabric') => call<CatalogPage>('search_catalog', { source, query, gameVersion, offset, sort, loaderKind });
export const catalogVersions = (source: CatalogSource, projectId: string, gameVersion: string, loaderKind: import('./model').LoaderKind = 'fabric') => call<CatalogRelease[]>('catalog_versions', { source, projectId, gameVersion, loaderKind });
export const installCatalogMod = (source: CatalogSource, projectId: string, versionId: string, instance: Instance) => call<CatalogInstallResult>('install_catalog_mod', { source, projectId, versionId, instanceId: instance.id, gameVersion: instance.version, loader: instance.loader ?? null, engine: instance.engine ?? 'java' });
export const openCatalogProject = (source: CatalogSource, slug: string) => call<void>('open_catalog_project', { source, slug });
export const openCurseForgeSetup = () => call<void>('open_curseforge_setup');

export interface ModpackRelease { id: string; name: string; number: string; published: string; channel: string; gameVersions: string[]; loaders?: import('./model').LoaderKind[] }
export interface ModpackInstallResult { instance: Instance; files: number }
export const searchModpacks = (query: string, offset = 0, sort: CatalogSort = 'downloads') => call<CatalogPage>('search_modpacks', { query, offset, sort });
export const modpackVersions = (projectId: string) => call<ModpackRelease[]>('modpack_versions', { projectId });
export const installModpack = (projectId: string, versionId: string, includeOptional = true) => call<ModpackInstallResult>('install_modpack', { projectId, versionId, includeOptional });
export const openModpackProject = (projectId: string) => call<void>('open_modpack_project', { projectId });
