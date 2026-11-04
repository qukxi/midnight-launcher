import { describe, expect, it } from 'vitest';
import { formatAuthDiagnostics } from '../src/authDiagnostics';
import { translate } from '../src/i18n';

describe('public sign-in diagnostics', () => {
  it('reports the actual application and both Minecraft routes without serializing unrelated credential fields', () => {
    const diagnostics = {
      clientId: '11111111-2222-4333-8444-555555555555',
      accessToken: 'secret-access', refreshToken: 'secret-refresh',
      attempts: [
        { endpoint: 'https://api.minecraftservices.com/launcher/login', httpStatus: 403, errorCode: 'AUTH_MINECRAFT_FORBIDDEN', body: 'secret-response' },
        { endpoint: 'https://api.minecraftservices.com/authentication/login_with_xbox', httpStatus: 200, errorCode: null },
      ],
    };
    const report = formatAuthDiagnostics(diagnostics);
    expect(report).toContain(diagnostics.clientId);
    expect(report).toContain('HTTP 403'); expect(report).toContain('HTTP 200');
    expect(report).toContain('login_with_xbox');
    expect(report).not.toContain('secret-');
  });
  it('does not display arbitrary URLs, response text or malformed client IDs as diagnostic details', () => {
    const report = formatAuthDiagnostics({ clientId: 'secret-client', attempts: [{ endpoint: 'https://example.org/?token=secret-token', httpStatus: 403, errorCode: 'arbitrary secret-response' }] });
    expect(report).not.toContain('secret');
    expect(report).toContain('Unknown endpoint');
    expect(report).toContain('Unknown error');
  });
  it('explains a generic denial without asserting that Mojang approval is missing', () => {
    expect(translate('en', 'AUTH_MINECRAFT_FORBIDDEN')).toContain('HTTP 403');
    expect(translate('en', 'AUTH_MINECRAFT_FORBIDDEN')).not.toContain('approved');
    expect(translate('ru', 'AUTH_MINECRAFT_FORBIDDEN')).toContain('Диагностика входа');
  });
});
