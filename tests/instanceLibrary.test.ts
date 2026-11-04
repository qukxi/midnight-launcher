import { describe, expect, it } from 'vitest';
import { filterInstances, forgetInstance } from '../src/instanceLibrary';
import { initialState, type Instance } from '../src/model';

const instances: Instance[] = [
  { id: 'old', name: 'Тихая долина', version: '1.21.1' },
  { id: 'fabric', name: 'Мир с друзьями', version: '1.21.1', loader: { kind: 'fabric', version: '0.16.14' }, modpack: { projectId: 'pack', versionId: 'v1', versionName: 'Cozy 2.0' } },
  { id: 'native', name: 'Ночной мир', version: '0.2.97', engine: 'horizon' },
];

describe('instance library search', () => {
  it('finds Forge and NeoForge by their engine names and pinned loader versions', () => {
    const modded: Instance[] = [
      { id: 'forge', name: 'Builder', version: '1.20.1', loader: { kind: 'forge', version: '47.4.28' } },
      { id: 'neo', name: 'Adventures', version: '1.21.1', loader: { kind: 'neoforge', version: '21.1.265' } },
    ];
    expect(filterInstances(modded, 'Forge 47.4.28', 'java')).toEqual([modded[0]]);
    expect(filterInstances(modded, 'NeoForge 1.21.1', 'all')).toEqual([modded[1]]);
  });
  it('searches names, versions and loaders together without changing collection order', () => {
    expect(filterInstances(instances, '  FABRIC   1.21.1 ', 'all')).toEqual([instances[1]]);
    expect(filterInstances(instances, 'ТИХАЯ', 'all')).toEqual([instances[0]]);
    expect(filterInstances(instances, 'cozy 2.0', 'all')).toEqual([instances[1]]);
    expect(filterInstances(instances, 'java', 'all')).toEqual(instances.slice(0, 2));
    expect(filterInstances(instances, 'rust', 'all')).toEqual([instances[2]]);
    expect(filterInstances(instances, 'missing', 'all')).toEqual([]);
  });
  it('treats legacy instances as Java and combines the query with the engine filter', () => {
    expect(filterInstances(instances, '', 'java')).toEqual(instances.slice(0, 2));
    expect(filterInstances(instances, '1.21.1', 'horizon')).toEqual([]);
    expect(filterInstances(instances, '', 'horizon')).toEqual([instances[2]]);
  });
});

describe('forgetting an instance', () => {
  it('selects an existing fallback when the current instance is removed and preserves preferences', () => {
    const before = { ...structuredClone(initialState), instances: structuredClone(instances), selected: 'fabric' };
    const snapshot = structuredClone(before);
    const after = forgetInstance(before, 'fabric');
    expect(after.selected).toBe('old');
    expect(after.instances).toEqual([instances[0], instances[2]]);
    expect(after.settings).toBe(before.settings);
    expect(before).toEqual(snapshot);
  });
  it('keeps the current instance when another one is removed and cannot remove the last one', () => {
    const state = { ...initialState, instances, selected: 'fabric' };
    expect(forgetInstance(state, 'old').selected).toBe('fabric');
    expect(forgetInstance(state, 'missing')).toBe(state);
    expect(forgetInstance(initialState, initialState.selected)).toBe(initialState);
  });
});
