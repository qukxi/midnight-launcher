import { beforeEach, describe, expect, it, vi } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: vi.fn().mockResolvedValue(undefined) }));
import { invoke } from '@tauri-apps/api/core';
import { installModpack, modpackVersions, searchModpacks } from '../src/api';

describe('Modrinth modpack IPC', () => {
  beforeEach(() => vi.clearAllMocks());
  it('searches packs independently of the selected Minecraft instance', async () => {
    await searchModpacks('optimized', 20, 'updated');
    expect(invoke).toHaveBeenCalledWith('search_modpacks', { query: 'optimized', offset: 20, sort: 'updated' });
    await modpackVersions('pack1');
    expect(invoke).toHaveBeenLastCalledWith('modpack_versions', { projectId: 'pack1' });
  });
  it('lets Rust choose the new instance, paths and verified downloads, and forwards the optional-file choice', async () => {
    await installModpack('pack1', 'release1', false);
    expect(invoke).toHaveBeenLastCalledWith('install_modpack', { projectId: 'pack1', versionId: 'release1', includeOptional: false });
    const args = vi.mocked(invoke).mock.calls[0][1];
    expect(args).not.toHaveProperty('instanceId');
    expect(args).not.toHaveProperty('url');
    expect(args).not.toHaveProperty('path');
  });
});
