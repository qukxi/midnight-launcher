import { loaderName, type Instance, type SavedState } from './model';

export type InstanceFilter = 'all' | 'java' | 'horizon';

export function filterInstances(instances: Instance[], query: string, filter: InstanceFilter): Instance[] {
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return instances.filter(instance => {
    const engine = instance.engine ?? 'java';
    if (filter !== 'all' && engine !== filter) return false;
    const text = [instance.name, instance.version, engine === 'horizon' ? 'Horizon Rust' : 'Minecraft Java',
      instance.loader ? `${loaderName(instance.loader.kind)} ${instance.loader.version}` : engine === 'java' ? 'Vanilla' : '', instance.modpack?.versionName ?? ''].join(' ').toLowerCase();
    return terms.every(term => text.includes(term));
  });
}

export function forgetInstance(state: SavedState, id: string): SavedState {
  if (state.instances.length <= 1 || !state.instances.some(instance => instance.id === id)) return state;
  const instances = state.instances.filter(instance => instance.id !== id);
  return { ...state, instances, selected: state.selected === id ? instances[0].id : state.selected };
}
