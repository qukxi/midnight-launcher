import { themes } from './themes';
import type { Theme } from './themes';
export type { Theme } from './themes';
export interface Instance { id: string; name: string; version: string; loader?: { kind: 'fabric'; version: string }; memory?: number; javaPath?: string; accent?: 'rose' | 'purple' | 'graphite' }
export type Language = 'ru' | 'en';
export interface Settings { language: Language; theme: Theme; gradient: boolean; memory: number; javaPath: string; clientId: string; reduceMotion: boolean; offlineName?: string; minimizeOnLaunch?: boolean; restoreOnExit?: boolean }
export interface SavedState { instances: Instance[]; selected: string; settings: Settings }
// Public application identifier supplied by the Midnight project owner.
export const defaultClientId = '6b0fede0-1318-4335-908d-a668b7599747';
export const initialState: SavedState = {
  instances: [{ id: 'vanilla', name: 'Мой мир', version: '1.21.1' }], selected: 'vanilla',
  settings: { language: 'ru', theme: 'rose', gradient: true, memory: 4096, javaPath: '', clientId: defaultClientId, reduceMotion: false }
};
export const storageKey = 'midnight-launcher:v1';
export function validateInstance(name: string, version: string): string | null {
  if (!name.trim() || name.trim().length > 40) return 'Название должно содержать от 1 до 40 символов.';
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}$/.test(version)) return 'Укажите корректный ID версии, например 1.21.1.';
  return null;
}
export function restoreState(raw: string | null): SavedState {
  if (!raw) return structuredClone(initialState);
  try {
    const x = JSON.parse(raw);
    if (!Array.isArray(x.instances) || x.instances.length === 0 || x.instances.length > 100 ||
      !x.instances.every((i: Instance) => typeof i.id === 'string' && typeof i.name === 'string' && typeof i.version === 'string' && !validateInstance(i.name, i.version)) ||
      new Set(x.instances.map((i: Instance) => i.id)).size !== x.instances.length ||
      !x.instances.some((i: Instance) => i.id === x.selected) ||
      !Number.isInteger(x.settings?.memory) || x.settings.memory < 1024 || x.settings.memory > 16384 ||
      typeof x.settings.javaPath !== 'string' || typeof x.settings.clientId !== 'string' || typeof x.settings.reduceMotion !== 'boolean') return structuredClone(initialState);
    // Migrate v1 saves without discarding existing instances or preferences.
    x.settings.language = ['ru', 'en'].includes(x.settings.language) ? x.settings.language : 'ru';
    x.settings.theme = themes.some(theme => theme.id === x.settings.theme) ? x.settings.theme : 'rose';
    x.settings.gradient = typeof x.settings.gradient === 'boolean' ? x.settings.gradient : true;
    if (!x.settings.clientId.trim()) x.settings.clientId = defaultClientId;
    if (typeof x.settings.offlineName !== 'string' || !/^[A-Za-z0-9_]{1,16}$/.test(x.settings.offlineName)) delete x.settings.offlineName;
    for (const instance of x.instances) {
      if (instance.loader !== undefined && (instance.loader?.kind !== 'fabric' || typeof instance.loader?.version !== 'string' || !/^[A-Za-z0-9._+-]{1,64}$/.test(instance.loader.version))) return structuredClone(initialState);
    }
    for (const i of x.instances) {
      if (!Number.isInteger(i.memory) || i.memory < 1024 || i.memory > 16384) delete i.memory;
      if (typeof i.javaPath !== 'string') delete i.javaPath;
      if (!['rose', 'purple', 'graphite'].includes(i.accent)) delete i.accent;
    }
    for (const key of ['minimizeOnLaunch', 'restoreOnExit']) if (typeof x.settings[key] !== 'boolean') delete x.settings[key];
    return x;
  } catch { return structuredClone(initialState); }
}
