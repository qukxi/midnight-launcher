import { beforeEach, describe, expect, it, vi } from 'vitest';
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: vi.fn().mockResolvedValue(undefined) }));
import { invoke } from '@tauri-apps/api/core';
import { catalogVersions, connectCurseForge, importMod, installCatalogMod, maxModSize, searchCatalog, setModEnabled } from '../src/api';

describe('mod file boundary', () => {
  beforeEach(() => vi.clearAllMocks());
  it('sends binary data and preserves Unicode names through ASCII-safe headers', async () => {
    const bytes = new Uint8Array([80, 75, 3, 4]);
    const file = { name: 'Мой мод 🧩.jar', size: bytes.length, arrayBuffer: async () => bytes.buffer } as File;
    await importMod('instance-one', file);
    const [command, body, options] = vi.mocked(invoke).mock.calls[0];
    expect(command).toBe('import_mod');
    expect(body).toEqual(bytes);
    const headers = (options as { headers: Record<string, string> }).headers;
    expect(headers['x-instance-id']).toBe('instance-one');
    expect(headers['x-mod-name']).toMatch(/^[\x20-\x7e]+$/);
    expect(JSON.parse(headers['x-mod-name'])).toBe(file.name);
  });
  it('rejects invalid extensions and oversized files before reading them', async () => {
    const read = vi.fn();
    await expect(importMod('one', { name: 'mod.exe', size: 10, arrayBuffer: read } as unknown as File)).rejects.toThrow('MODS_JAR_REQUIRED');
    await expect(importMod('one', { name: 'mod.jar', size: maxModSize + 1, arrayBuffer: read } as unknown as File)).rejects.toThrow('MODS_TOO_LARGE');
    expect(read).not.toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
  });
  it('allows a Wonderland-sized local mod to reach the file read step', async () => {
    const read = vi.fn().mockRejectedValue(new Error('file read reached'));
    await expect(importMod('one', { name: 'wonderland.jar', size: 175374232, arrayBuffer: read } as unknown as File)).rejects.toThrow('file read reached');
    expect(read).toHaveBeenCalledOnce();
    expect(invoke).not.toHaveBeenCalled();
  });
  it('toggles the exact file inside the chosen instance', async () => {
    await setModEnabled('other-instance', 'mod.jar.disabled', true);
    expect(invoke).toHaveBeenCalledWith('set_mod_enabled', { instanceId: 'other-instance', fileName: 'mod.jar.disabled', enabled: true });
  });
  it('keeps catalog providers, pagination and game versions explicit', async () => {
    await searchCatalog('modrinth', 'Sodium', '1.21.1', 20);
    expect(invoke).toHaveBeenLastCalledWith('search_catalog', { source: 'modrinth', query: 'Sodium', gameVersion: '1.21.1', offset: 20, sort: 'downloads', loaderKind: 'fabric' });
    await searchCatalog('curseforge', 'Sodium', '1.21.1', 0, 'updated');
    expect(invoke).toHaveBeenLastCalledWith('search_catalog', { source: 'curseforge', query: 'Sodium', gameVersion: '1.21.1', offset: 0, sort: 'updated', loaderKind: 'fabric' });
    await catalogVersions('curseforge', '394468', '1.21.1');
    expect(invoke).toHaveBeenLastCalledWith('catalog_versions', { source: 'curseforge', projectId: '394468', gameVersion: '1.21.1', loaderKind: 'fabric' });
  });
  it('installs by trusted catalog identifiers and carries the selected loader to Rust', async () => {
    await installCatalogMod('modrinth', 'project', 'release', { id: 'second', name: 'Fabric', version: '1.21.1', loader: { kind: 'fabric', version: '0.16.14' } });
    expect(invoke).toHaveBeenLastCalledWith('install_catalog_mod', { source: 'modrinth', projectId: 'project', versionId: 'release', instanceId: 'second', gameVersion: '1.21.1', engine: 'java', loader: { kind: 'fabric', version: '0.16.14' } });
    const args = vi.mocked(invoke).mock.calls.at(-1)?.[1] as Record<string, unknown>;
    expect(args).not.toHaveProperty('url'); expect(args).not.toHaveProperty('hash');
  });
  it('filters catalogs and releases by Forge or NeoForge and preserves their loader pin', async () => {
    for (const kind of ['forge', 'neoforge'] as const) {
      await searchCatalog('modrinth', 'Sodium', '1.21.1', 0, 'downloads', kind);
      expect(invoke).toHaveBeenLastCalledWith('search_catalog', expect.objectContaining({ loaderKind: kind }));
      await catalogVersions('curseforge', '394468', '1.21.1', kind);
      expect(invoke).toHaveBeenLastCalledWith('catalog_versions', expect.objectContaining({ loaderKind: kind }));
      const loader = { kind, version: kind === 'forge' ? '52.1.14' : '21.1.265' };
      await installCatalogMod('modrinth', 'project', 'release', { id: 'modded', name: kind, version: '1.21.1', loader });
      expect(invoke).toHaveBeenLastCalledWith('install_catalog_mod', expect.objectContaining({ loader }));
    }
  });
  it('passes CurseForge credentials only to the dedicated connection command', async () => {
    await connectCurseForge('fixture-not-a-real-api-key');
    expect(invoke).toHaveBeenLastCalledWith('connect_curseforge', { key: 'fixture-not-a-real-api-key' });
    await searchCatalog('curseforge', '', '1.21.1');
    expect(invoke).toHaveBeenLastCalledWith('search_catalog', expect.not.objectContaining({ key: expect.anything() }));
  });
});
