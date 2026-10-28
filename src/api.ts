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
export const desktop = isTauri();
function call<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop) return Promise.reject(new Error('Эта функция доступна в десктопном приложении. Откройте его командой npm run tauri dev.'));
  return invoke<T>(name, args);
}
export const listVersions = () => call<Version[]>('list_versions');
export const installGame = (version: string, javaPath: string, loader?: Instance['loader']) => call<InstallReport>('install_game', { version, javaPath, loader: loader || null });
export const launchGame = (instanceId: string, version: string, javaPath: string, memory: number, offlineName?: string, loader?: Instance['loader'], minimizeOnLaunch = false, restoreOnExit = true) => call<GameStatus>('launch_game', { instanceId, version, javaPath, memory, offlineName: offlineName || null, loader: loader || null, minimizeOnLaunch, restoreOnExit });
export const gameStatus = () => call<GameStatus>('game_status');
export const stopGame = () => call<void>('stop_game');
export const cancelOperation = () => call<void>('cancel_operation');
export const authStatus = (clientId: string) => call<AuthStatus>('auth_status', { clientId });
export const beginLogin = (clientId: string) => call<DevicePrompt>('begin_login', { clientId });
export const pollLogin = (id: string) => call<PollResult>('poll_login', { id });
export const openLogin = (id: string) => call<void>('open_login', { id });
export const cancelLogin = () => call<void>('cancel_login');
export const getAccount = () => call<Profile | null>('get_account');
export const restoreAccount = () => call<Profile>('restore_account');
export const signOut = () => call<void>('sign_out');

export const readGameLog = (instanceId: string) => call<string>('read_game_log', { instanceId });

export const loaderVersions = (version: string) => call<string[]>('loader_versions', { version });
