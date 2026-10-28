import { themes } from '../src/themes';
import { describe, it, expect } from 'vitest';
import { defaultClientId, initialState, restoreState, validateInstance } from '../src/model';
describe('persisted launcher state', () => {
  it('recovers from malformed or incompatible storage', () => {
    for (const raw of ['oops', '{}', 'null', JSON.stringify({ ...initialState, instances: [] }), JSON.stringify({ ...initialState, selected: 'missing' }), JSON.stringify({ ...initialState, settings: { ...initialState.settings, memory: -1 } })]) expect(restoreState(raw)).toEqual(initialState);
  });
  it('retains user instances and preferences across reloads', () => {
    const state = { instances: [{ id: 'custom', name: 'Ночной мир', version: '24w14a' }], selected: 'custom', settings: { language: 'en', theme: 'graphite', gradient: false, memory: 8192, javaPath: '/custom/java', clientId: '11111111-2222-4333-8444-555555555555', reduceMotion: true } };
    expect(restoreState(JSON.stringify(state))).toEqual(state);
  });
  it('rejects unsafe or empty instance input', () => {
    expect(validateInstance('', '1.21.1')).toBeTruthy();
    expect(validateInstance('World', '../outside')).toBeTruthy();
    expect(validateInstance('World', '1.21.1')).toBeNull();
  });
});

describe('appearance and language migrations', () => {
  it('keeps old instances and settings when new preferences are missing', () => {
    const { language: _language, theme: _theme, gradient: _gradient, ...oldSettings } = initialState.settings;
    const old = { ...initialState, settings: { ...oldSettings, javaPath: '/saved/java', memory: 8192 } };
    const result = restoreState(JSON.stringify(old));
    expect(result.settings).toMatchObject({ javaPath: '/saved/java', memory: 8192, language: 'ru', theme: 'rose', gradient: true });
    expect(result.instances).toEqual(old.instances);
  });
  it('recovers invalid preferences without dropping user data', () => {
    const result = restoreState(JSON.stringify({ ...initialState, settings: { ...initialState.settings, language: 'bad', theme: '<invalid>', gradient: 'false' } }));
    expect(result.settings).toMatchObject({ language: 'ru', theme: 'rose', gradient: true });
  });
});


describe('Microsoft application configuration', () => {
  it('fills an old empty Client ID while retaining existing preferences', () => {
    const old = { ...initialState, settings: { ...initialState.settings, clientId: '  ', memory: 8192, theme: 'graphite' } };
    expect(restoreState(JSON.stringify(old)).settings).toMatchObject({ clientId: defaultClientId, memory: 8192, theme: 'graphite' });
    expect(restoreState(null).settings.clientId).toBe(defaultClientId);
  });
});

describe('offline profiles', () => {
  it('retains valid offline names and rejects corrupted saved identities', () => {
    for (const offlineName of ['Player_1', '', '../test', 'bad name', 42]) {
      const restored = restoreState(JSON.stringify({ ...initialState, settings: { ...initialState.settings, offlineName } }));
      expect(restored.settings.offlineName).toBe(offlineName === 'Player_1' ? offlineName : undefined);
    }
  });
});


describe('instance loaders', () => {
  it('keeps Vanilla saves and retains a pinned Fabric loader', () => {
    const old = restoreState(JSON.stringify(initialState));
    expect(old.instances[0].loader).toBeUndefined();
    const state = { ...initialState, instances: [{ ...initialState.instances[0], loader: {kind: 'fabric', version: '0.16.14'} }] };
    expect(restoreState(JSON.stringify(state)).instances[0].loader).toEqual({ kind: 'fabric', version: '0.16.14' });
    state.instances[0].loader.version = '../bad';
    expect(restoreState(JSON.stringify(state))).toEqual(initialState);
  });
});

describe('instance customization and launcher behavior', () => {
  it('persists per-instance overrides and normalizes invalid optional values', () => {
    const state = { ...initialState, settings: { ...initialState.settings, minimizeOnLaunch: true, restoreOnExit: false }, instances: [{ ...initialState.instances[0], memory: 8192, javaPath: '/custom/java', accent: 'rose' }] };
    const result = restoreState(JSON.stringify(state));
    expect(result.instances[0]).toMatchObject({ memory: 8192, javaPath: '/custom/java', accent: 'rose' });
    expect(result.settings).toMatchObject({ minimizeOnLaunch: true, restoreOnExit: false });
    const invalid = restoreState(JSON.stringify({ ...state, instances: [{ ...state.instances[0], memory: -1, accent: 'bad' }], settings: { ...state.settings, minimizeOnLaunch: 'yes' } }));
    expect(invalid.instances[0].memory).toBeUndefined();
    expect(invalid.instances[0].accent).toBeUndefined();
    expect(invalid.instances[0].id).toBe(state.instances[0].id);
    expect(invalid.settings.minimizeOnLaunch).toBeUndefined();
  });
});


describe('community palettes', () => {
  it('restores every offered theme without changing instances', () => {
    expect(new Set(themes.map(t => t.id)).size).toBe(themes.length);
    for (const theme of themes) {
      const restored = restoreState(JSON.stringify({ ...initialState, settings: { ...initialState.settings, theme: theme.id } }));
      expect(restored.settings.theme).toBe(theme.id);
      expect(restored.instances).toEqual(initialState.instances);
    }
  });
});
