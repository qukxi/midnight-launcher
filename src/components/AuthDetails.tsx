import { useState } from 'react';
import type { AuthDiagnostics } from '../api';
import type { Language } from '../model';
import { translate } from '../i18n';
import { formatAuthDiagnostics } from '../authDiagnostics';

export function AuthDetails({ diagnostics, language }: { diagnostics: AuthDiagnostics; language: Language }) {
  const t = (text: string) => translate(language, text);
  const [copyStatus, setCopyStatus] = useState('');
  const report = formatAuthDiagnostics(diagnostics);
  async function copy() {
    try { await navigator.clipboard.writeText(report); setCopyStatus('Скопировано'); }
    catch { setCopyStatus('Выдели и скопируй сведения вручную.'); }
  }
  return <details className="auth-diagnostics" open>
    <summary>{t('Диагностика входа')}</summary>
    <p>{t('Публичный ID приложения и ответы Minecraft Services. Пароли и токены сюда не попадают.')}</p>
    <pre>{report}</pre>
    <button className="secondary" onClick={() => void copy()}>{t('Копировать сведения')}</button>
    {copyStatus && <span role="status">{t(copyStatus)}</span>}
  </details>;
}
