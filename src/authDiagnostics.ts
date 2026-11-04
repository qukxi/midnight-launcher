import type { AuthDiagnostics } from './api';

const endpoints = new Set([
  'https://api.minecraftservices.com/launcher/login',
  'https://api.minecraftservices.com/authentication/login_with_xbox',
]);
// Export only the public diagnostic contract, even if IPC gains other fields later.
export function formatAuthDiagnostics(diagnostics: AuthDiagnostics): string {
  const clientId = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(diagnostics.clientId) ? diagnostics.clientId : 'invalid';
  return [`Midnight Launcher 0.2.0`, `Client ID: ${clientId}`, ...diagnostics.attempts.map((attempt, index) => {
    const endpoint = endpoints.has(attempt.endpoint) ? attempt.endpoint : 'Unknown endpoint';
    const status = Number.isInteger(attempt.httpStatus) && attempt.httpStatus >= 100 && attempt.httpStatus <= 599 ? attempt.httpStatus : 'unknown';
    const code = attempt.errorCode === null ? 'OK' : /^(?:AUTH_RESPONSE_TOO_LARGE|AUTH_MINECRAFT_(?:APP_APPROVAL_REQUIRED|FORBIDDEN|HTTP_[1-5]\d{2}|INVALID_RESPONSE|NETWORK|REDIRECT_REJECTED))$/.test(attempt.errorCode) ? attempt.errorCode : 'Unknown error';
    return `${index + 1}. POST ${endpoint}\nHTTP ${status} · ${code}`;
  })].join('\n');
}
