import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true, invoke: vi.fn().mockResolvedValue({ running: true, pid: 12, exit_code: null }) }));
import { invoke } from '@tauri-apps/api/core';
import { authDiagnostics, getOfflineAccount, selectOfflineProfile, launchGame, loaderVersions } from '../src/api';
import { horizonVersion } from '../src/model';

describe('launch engine IPC', () => {
  it('requests versions for the exact loader and carries the pin to the launch command', async () => {
    for (const kind of ['forge', 'neoforge'] as const) {
      await loaderVersions('1.21.1', kind);
      expect(invoke).toHaveBeenLastCalledWith('loader_versions', { version: '1.21.1', kind });
      const loader = { kind, version: kind === 'forge' ? '52.1.14' : '21.1.265' };
      await launchGame('modded', '1.21.1', '', 4096, undefined, loader);
      expect(invoke).toHaveBeenLastCalledWith('launch_game', expect.objectContaining({ loader, engine: 'java' }));
    }
  });
  beforeEach(() => vi.clearAllMocks());
  it('reads public sign-in diagnostics without submitting credentials or restoring a session', async () => {
    await authDiagnostics();
    expect(invoke).toHaveBeenCalledExactlyOnceWith('auth_diagnostics', undefined);
  });
  it('checks offline access in Rust without supplying a client-side authorization flag', async () => {
    await getOfflineAccount();
    expect(invoke).toHaveBeenCalledExactlyOnceWith('get_offline_account', undefined);
  });
  it('validates an offline selection in Rust before saving the selected username', async () => {
    vi.mocked(invoke).mockRejectedValueOnce('OFFLINE_LOGIN_REQUIRED');
    await expect(selectOfflineProfile('Player_1')).rejects.toBe('OFFLINE_LOGIN_REQUIRED');
    expect(invoke).toHaveBeenCalledExactlyOnceWith('select_offline_profile', { name: 'Player_1' });
  });
  it('keeps old Java launches compatible', async () => {
    await launchGame('old', '1.21.1', '', 4096);
    expect(invoke).toHaveBeenCalledWith('launch_game', expect.objectContaining({ engine: 'java', version: '1.21.1', loader: null }));
  });
  it('routes native Horizon through the same process lifecycle without a Java loader', async () => {
    await launchGame('native', horizonVersion, 'C:\\Java 25\\bin\\java.exe', 4096, undefined, undefined, true, true, 'horizon');
    expect(invoke).toHaveBeenCalledWith('launch_game', expect.objectContaining({ instanceId: 'native', engine: 'horizon', version: horizonVersion, javaPath: 'C:\\Java 25\\bin\\java.exe', loader: null, minimizeOnLaunch: true, restoreOnExit: true }));
  });
});
