import { SettingsPage } from './components/SettingsPage';
import { InstancesPage } from './components/InstancesPage';
import { Dropdown } from './components/Dropdown';
import { ModsManager } from './components/ModsManager';
import { ModpackCatalog } from './components/ModpackCatalog';
import { AuthDetails } from './components/AuthDetails';
import { authDiagnostics, type AuthDiagnostics } from './api';
import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { ArrowRight, Check, CircleHelp, Cpu, Gamepad2, Layers3, Play, Moon, Puzzle, Settings2, ShieldCheck, UserRound, X } from 'lucide-react';
import { loaderVersions, readGameLog, desktop, launchGame, listVersions, beginLogin, pollLogin, cancelLogin, openLogin, getAccount, getOfflineAccount, selectOfflineProfile, restoreAccount, signOut, gameStatus, stopGame, cancelOperation, type Version, type Profile, type DevicePrompt, type Progress } from './api';
import { listen } from '@tauri-apps/api/event';
import { translate } from './i18n';
import { connectHorizon, horizonVersion, loaderName, restoreState, storageKey, validateInstance, type LoaderKind } from './model';
import { forgetInstance } from './instanceLibrary';
type Screen = 'home' | 'instances' | 'modpacks' | 'mods' | 'accounts' | 'settings';
const tabs = [{ id: 'home', name: 'Главная', icon: Gamepad2 }, { id: 'instances', name: 'Сборки', icon: Layers3 }, { id: 'mods', name: 'Моды', icon: Puzzle }, { id: 'accounts', name: 'Аккаунты', icon: UserRound }, { id: 'settings', name: 'Настройки', icon: Settings2 }] as const;
const errorText = (e: unknown) => e instanceof Error ? e.message : String(e);
export default function App() {
  const [state, setState] = useState(() => { try { return restoreState(localStorage.getItem(storageKey)); } catch { return restoreState(null); } });
  const [screen, setScreen] = useState<Screen>(() => !desktop && window.location.hash === '#settings' ? 'settings' : !desktop && window.location.hash === '#instances' ? 'instances' : 'home');
  const activeScreen = screen === 'modpacks' ? 'instances' : screen;
  const [modsInstanceId, setModsInstanceId] = useState(state.selected);
  const [message, setMessage] = useState('');
  const [storageError, setStorageError] = useState(false);
  const [operationBusy, setBusy] = useState(false);
  const [modsBusy, setModsBusy] = useState(false);
  const busy = operationBusy || modsBusy;
  const [profile, setProfile] = useState<Profile | null>(null);
  const [offlineAccount, setOfflineAccount] = useState<Profile | null>(null);
  const accountRevision = useRef(0);
  const [device, setDevice] = useState<DevicePrompt | null>(null);
  const [offlineDraft, setOfflineDraft] = useState(state.settings.offlineName ?? '');
  const [loginBusy, setLoginBusy] = useState(false);
  const [authDetails, setAuthDetails] = useState<AuthDiagnostics | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [running, setRunning] = useState(false);
  const [versions, setVersions] = useState<Version[]>([]);
  const [name, setName] = useState(''); const [version, setVersion] = useState('1.21.1');
  const [loaderKind, setLoaderKind] = useState<'vanilla' | LoaderKind>('vanilla');
  const [engine, setEngine] = useState<'java' | 'horizon'>('java');
  const [loaderVersion, setLoaderVersion] = useState('');
  const [loaderChoices, setLoaderChoices] = useState<string[]>([]);
  const [loadingLoaders, setLoadingLoaders] = useState(false);
  const [loaderError, setLoaderError] = useState('');
  const [loaderRevision, setLoaderRevision] = useState(0);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [instanceMemory, setInstanceMemory] = useState('');
  const [instanceJava, setInstanceJava] = useState('');
  const [instanceAccent, setInstanceAccent] = useState<'rose' | 'purple' | 'graphite'>('purple');
  const [choosingInstance, setChoosingInstance] = useState(false);
  const chooserDialog = useRef<HTMLDialogElement>(null);
  const [creating, setCreating] = useState(false);
  const [logInstanceId, setLogInstanceId] = useState(state.selected);
  const [logsOpen, setLogsOpen] = useState(false);
  const [logText, setLogText] = useState('');
  const [logError, setLogError] = useState('');
  const [followLogs, setFollowLogs] = useState(true);
  const instanceDialog = useRef<HTMLDialogElement>(null);
  const logsDialog = useRef<HTMLDialogElement>(null);
  const logOutput = useRef<HTMLPreElement>(null);
  const [help, setHelp] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const content = useRef<HTMLElement>(null);
  useEffect(() => { if (content.current) content.current.scrollTop = 0; }, [screen]);
  const current = state.instances.find(i => i.id === state.selected)!;
  const logInstance = state.instances.find(i => i.id === logInstanceId) ?? current;
  useEffect(() => { try { localStorage.setItem(storageKey, JSON.stringify(state)); setStorageError(false); } catch { setStorageError(true); } }, [state]);
  useEffect(() => { if (help) dialog.current?.showModal(); else dialog.current?.close(); }, [help]);
  useEffect(() => { if (choosingInstance) chooserDialog.current?.showModal(); else chooserDialog.current?.close(); }, [choosingInstance]);
  useEffect(() => {
    if (creating) {
      instanceDialog.current?.showModal();
      instanceDialog.current?.querySelector<HTMLInputElement>('.instance-name-field input')?.focus();
    } else instanceDialog.current?.close();
  }, [creating]);
  useEffect(() => { if (logsOpen) logsDialog.current?.showModal(); else logsDialog.current?.close(); }, [logsOpen]);
  useEffect(() => {
    if (!logsOpen) return;
    let active = true; let timer: number;
    setLogText(''); setLogError('');
    const refresh = async () => {
      try { const text = await readGameLog(logInstance.id); if (active) { setLogText(text); setLogError(''); } }
      catch (error) { if (active) setLogError(errorText(error)); }
      if (active) timer = window.setTimeout(refresh, 1000);
    };
    void refresh();
    return () => { active = false; window.clearTimeout(timer); };
  }, [logsOpen, logInstance.id]);
  useEffect(() => { if (followLogs && logOutput.current) logOutput.current.scrollTop = logOutput.current.scrollHeight; }, [logText, followLogs, logsOpen]);
  const settings = state.settings;
  const account = profile ?? offlineAccount;
  const offlineAllowed = offlineAccount !== null;
  const t = (text: string) => translate(settings.language, text);
  useEffect(() => { document.documentElement.dataset.reduceMotion = String(settings.reduceMotion); return () => { delete document.documentElement.dataset.reduceMotion; }; }, [settings.reduceMotion]);
  useEffect(() => { document.documentElement.lang = settings.language; }, [settings.language]);
  useEffect(() => { setMessage(''); }, [settings.language]);
  useEffect(() => {
    if (!desktop) return;
    let active = true;
    getAccount().then(p => { if (active) setProfile(p); }).catch(e => { if (active) setMessage(t(errorText(e))); });
    const unlisten = listen<Progress>('launcher-progress', e => { if (active) setProgress(e.payload); });
    return () => { active = false; void unlisten.then(stop => stop()); };
  }, []);
  useEffect(() => {
    if (!desktop) return;
    let active = true;
    const check = () => gameStatus().then(s => { if (active) { setRunning(s.running); if (s.exit_code !== null) setMessage(t('Игра завершилась. Код: {code}').replace('{code}', String(s.exit_code))); } }).catch(() => {});
    void check(); const timer = window.setInterval(check, 2000);
    return () => { active = false; window.clearInterval(timer); };
  }, [settings.language]);
  useEffect(() => {
    if (!desktop || screen !== 'accounts') return;
    let active = true;
    const revision = accountRevision.current;
    getOfflineAccount().then(saved => {
      if (active && revision === accountRevision.current) setOfflineAccount(saved);
    }).catch(error => {
      if (active && revision === accountRevision.current) { setOfflineAccount(null); setMessage(t(errorText(error))); }
    });
    return () => { active = false; };
  }, [screen, profile]);
  useEffect(() => {
    if (!device) return;
    let active = true; let timer: number;
    const check = async () => {
      try {
        const result = await pollLogin(device.id);
        if (!active) return;
        if (result.state === 'connected' && result.profile) { setConnectedAccount(result.profile); setAuthDetails(null); updateSettings({ offlineName: undefined }); setDevice(null); setMessage(t('Аккаунт Minecraft подключён.')); }
        else timer = window.setTimeout(check, Math.max(5, result.interval) * 1000);
      } catch (error) { if (active) { setDevice(null); reportLoginError(error); } }
    };
    timer = window.setTimeout(check, Math.max(5, device.interval) * 1000);
    return () => { active = false; window.clearTimeout(timer); };
  }, [device, settings.language]);
  useEffect(() => {
    if (!creating || !desktop || engine === 'horizon') return;
    let active = true;
    listVersions().then(items => { if (active) setVersions(items); }).catch(error => { if (active) setMessage(errorText(error)); });
    return () => { active = false; };
  }, [creating, engine]);
  useEffect(() => {
    if (!creating || engine !== 'java' || loaderKind === 'vanilla' || !desktop) { setLoadingLoaders(false); return; }
    let active = true;
    setLoadingLoaders(true); setLoaderError(''); setLoaderChoices([]);
    const timer = window.setTimeout(() => {
      loaderVersions(version, loaderKind).then(values => {
        if (active) { setLoaderChoices(values); setLoaderVersion(value => value || values[0] || ''); }
      }).catch(error => { if (active) setLoaderError(errorText(error)); })
        .finally(() => { if (active) setLoadingLoaders(false); });
    }, 250);
    return () => { active = false; window.clearTimeout(timer); };
  }, [creating, engine, version, loaderKind, loaderRevision]);
  function editInstance(id: string) {
    setMessage('');
    const instance = state.instances.find(i => i.id === id)!;
    setEditingId(id); setName(instance.name); setVersion(instance.version);
    setEngine(instance.engine ?? 'java');
    setLoaderKind(instance.loader?.kind ?? 'vanilla'); setLoaderVersion(instance.loader?.version ?? '');
    setLoaderChoices([]); setInstanceMemory(instance.memory ? String(instance.memory) : '');
    setInstanceJava(instance.javaPath ?? ''); setInstanceAccent(instance.accent ?? 'purple');
    setCreating(true); setScreen('instances');
  }
  function newInstance() {
    setMessage('');
    setEditingId(null); setName(''); setVersion('1.21.1'); setLoaderKind('vanilla'); setLoaderVersion(''); setLoaderChoices([]);
    setEngine('java');
    setInstanceMemory(''); setInstanceJava(''); setInstanceAccent('purple'); setCreating(true);
  }
  function openMods(id: string) { setModsInstanceId(id); setScreen('mods'); }
  function selectInstance(id: string) {
    if (busy || running) return;
    setState(s => s.selected === id || !s.instances.some(instance => instance.id === id) ? s : { ...s, selected: id });
  }
  function playInstance(id: string) {
    if (busy || running) return;
    const instance = state.instances.find(item => item.id === id);
    if (!instance) return;
    selectInstance(id);
    void run(async () => {
      const result = await launchGame(instance.id, instance.version, instance.javaPath ?? settings.javaPath, instance.memory ?? settings.memory, settings.offlineName, instance.loader, settings.minimizeOnLaunch ?? false, settings.restoreOnExit ?? true, instance.engine);
      setRunning(result.running);
      return t('Игра запущена.');
    });
  }
  function selectHorizon() {
    if (busy || running) return;
    try { setState(connectHorizon(state, crypto.randomUUID())); setScreen('home'); setMessage(t('Horizon подключён. Нажми Launch: клиент и ресурсы подготовятся автоматически.')); }
    catch (error) { setMessage(t(errorText(error))); }
  }
  function reportLoginError(error: unknown) {
    setMessage(t(errorText(error)));
    void authDiagnostics().then(setAuthDetails).catch(() => setAuthDetails(null));
  }
  async function startLogin() {
    if (loginBusy || device) return;
    setLoginBusy(true); setMessage(''); setAuthDetails(null);
    try { setDevice(await beginLogin(settings.clientId)); } catch (error) { reportLoginError(error); } finally { setLoginBusy(false); }
  }
  function setConnectedAccount(connected: Profile | null) {
    accountRevision.current += 1;
    setProfile(connected); setOfflineAccount(connected);
  }
  async function restoreSavedAccount() {
    const saved = await restoreAccount();
    setConnectedAccount(saved); setAuthDetails(null); updateSettings({ offlineName: undefined });
    return t('Аккаунт Minecraft подключён.');
  }
  async function stopLogin() { setDevice(null); try { await cancelLogin(); setConnectedAccount(null); updateSettings({ offlineName: undefined }); } catch (error) { setMessage(t(errorText(error))); } }

  function updateSettings(patch: Partial<typeof settings>) { setState(s => ({ ...s, settings: { ...s.settings, ...patch } })); }
  async function run(action: () => Promise<string>) { if (busy) return; setBusy(true); setMessage(''); setProgress(null); try { setMessage(await action()); } catch (e) { if (/^(AUTH_|KEYCHAIN_)/.test(errorText(e))) reportLoginError(e); else setMessage(t(errorText(e))); } finally { setBusy(false); setProgress(null); } }
  function addInstance(e: React.FormEvent) {
    e.preventDefault(); const error = validateInstance(name, version); if (error) { setMessage(t(error)); return; }
    if (!editingId && state.instances.length >= 100) { setMessage(t('Достигнут лимит: 100 инстансов.')); return; }
    if (engine === 'java' && loaderKind !== 'vanilla' && !/^[A-Za-z0-9._+-]{1,64}$/.test(loaderVersion)) { setMessage(t('Выбери версию загрузчика.')); return; }
    const instance = { id: editingId ?? crypto.randomUUID(), name: name.trim(), version: engine === 'horizon' ? horizonVersion : version, engine, memory: engine === 'java' && instanceMemory ? Number(instanceMemory) : undefined, javaPath: instanceJava.trim() || undefined, accent: instanceAccent, modpack: engine === 'java' ? state.instances.find(item => item.id === editingId)?.modpack : undefined, ...(engine === 'java' && loaderKind !== 'vanilla' ? { loader: { kind: loaderKind, version: loaderVersion } } : {}) };
    if (instance.memory !== undefined && (!Number.isInteger(instance.memory) || instance.memory < 1024 || instance.memory > 16384)) { setMessage(t('Укажите память от 1024 до 16384 МБ.')); return; }
    setState(s => ({ ...s, selected: instance.id, instances: editingId ? s.instances.map(i => i.id === editingId ? instance : i) : [...s.instances, instance] })); setCreating(false); setEditingId(null); setName(''); setMessage(t('Сборка сохранена. Недостающие файлы установятся при запуске.'));
  }
  return <div className={`app ${settings.reduceMotion ? 'reduce-motion' : ''}`} data-theme={settings.theme} data-gradient={settings.gradient}>
    <header className="topbar"><button className="brand" disabled={modsBusy} onClick={() => setScreen('home')} aria-label={t("Midnight Launcher — главная")}><span className="brand-mark"><Moon size={20}/></span><span>MIDNIGHT</span></button><span className="top-caption">{t("ТВОИ МИРЫ. ТВОЁ ВРЕМЯ.")}</span><button className="icon-button help" aria-label={t("О проекте")} onClick={() => setHelp(true)}><CircleHelp size={19}/></button></header>
    <main ref={content}>
      {screen === 'home' && <section className="home page">
        <div className="home-title"><h1>MIDNIGHT</h1><img src="/reference/app-icon.png" width="60" height="60" alt="" className="reference-icon"/></div>
        <div className="launch-area">
        <div className="launch-row"><button className="launch-button" aria-busy={busy} disabled={busy || running} onClick={() => playInstance(current.id)}><img src="/reference/launch-overlay.svg" className="launch-overlay" alt=""/><span><Play size={17} fill="currentColor"/>{running ? t('Игра запущена') : busy ? t('Подождите…') : 'Launch'}</span></button></div>
        <button className="home-instance-card" onClick={() => setChoosingInstance(true)} disabled={busy || running} aria-label={t('Выбрать сборку')}>
          <span className={`instance-miniature miniature-${current.accent ?? 'purple'}`} aria-hidden="true"><Layers3 size={21}/></span>
          <span className="home-instance-info"><strong>{current.name}</strong><span>{current.version}<span className="instance-meta-dot">·</span>{current.engine === 'horizon' ? 'Horizon · Rust' : current.loader ? loaderName(current.loader.kind) : 'Vanilla'}</span></span>
          <span className="instance-change">{t('Сменить')}<ArrowRight size={15}/></span>
        </button>{current.engine !== 'horizon' && <button className="home-mods-button" onClick={() => openMods(current.id)}><Puzzle size={14}/>{t('Моды сборки')}<ArrowRight size={13}/></button>}</div>

      </section>}
      {screen === 'instances' && <InstancesPage instances={state.instances} selected={state.selected} settings={settings} busy={busy} running={running}
        onSelect={selectInstance} onPlay={playInstance} onCreate={newInstance} onEdit={editInstance} onMods={openMods}
        onRemove={id => { if (!busy && !running) setState(s => forgetInstance(s, id)); }}
        onBrowse={() => setScreen('modpacks')} onHorizon={selectHorizon}/>}
      {screen === 'modpacks' && <ModpackCatalog language={settings.language} locked={busy} running={running} instanceCount={state.instances.length} onBusyChange={setModsBusy} onBack={() => setScreen('instances')} onInstalled={(instance, files) => {
        setState(s => ({ ...s, selected: instance.id, instances: [...s.instances, instance] }));
        setScreen('instances');
        setMessage(t('Сборка «{name}» установлена. Файлов: {count}.').replace('{name}', instance.name).replace('{count}', String(files)));
      }}/>}
      {screen === 'mods' && <ModsManager instances={state.instances} instanceId={state.instances.some(i => i.id === modsInstanceId) ? modsInstanceId : current.id} onInstanceChange={setModsInstanceId} language={settings.language} locked={busy} running={running} onBusyChange={setModsBusy} onConfigure={editInstance}/>}
      {screen === 'accounts' && <section className="page content-page"><div className="section-heading"><div><div className="eyebrow">{t('ТВОЯ ИДЕНТИЧНОСТЬ')}</div><h2>{t('Аккаунты')}<span>.</span></h2><p>{t('Один профиль. Бесконечно много миров.')}</p></div></div>
        <div className="account-panel panel"><div className="account-avatar"><UserRound size={34}/></div>
          {device ? <><span className="badge">{t('ПОДТВЕРЖДЕНИЕ MICROSOFT')}</span><h3>{t('Введите код на сайте Microsoft')}</h3><p>{t('Откройте официальный сайт, введите код и завершите вход. Пароль вводится только на Microsoft.')}</p><code className="device-code">{device.userCode}</code><p className="device-uri">{device.verificationUri}</p><p>{t('Код действует {minutes} мин. Ожидаем подтверждение…').replace('{minutes}', String(Math.ceil(device.expiresIn / 60)))}</p><button className="primary small" onClick={() => run(async () => { await openLogin(device.id); return t('Microsoft открыт в системном браузере.'); })}>{t('Открыть Microsoft')}</button><button className="secondary" onClick={() => void stopLogin()}>{t('Отменить вход')}</button></>
          : account ? <><span className="badge">MINECRAFT JAVA</span><h3>{account.name}</h3><p>{t('Аккаунт сохранён. Для онлайн-игры сессия обновляется и лицензия проверяется.')}</p><div className="account-session-actions">{!profile && <button className="secondary account-restore" disabled={busy || loginBusy} onClick={() => run(restoreSavedAccount)}>{t('Восстановить сохранённый вход')}</button>}<button className="secondary" disabled={busy || loginBusy} onClick={() => run(async () => { await signOut(); setConnectedAccount(null); updateSettings({ offlineName: undefined }); return t('Вы вышли из аккаунта.'); })}>{t('Выйти из аккаунта')}</button></div></>
          : <><span className="badge">{t('НЕТ ПОДКЛЮЧЁННОГО АККАУНТА')}</span><h3>{t('Твоё место в мире.')}</h3><p>{t('Вход через Microsoft. Нужна лицензия Minecraft Java Edition.')}</p><div className="account-signin-actions"><button className="microsoft-button" disabled={busy || loginBusy || !!device} onClick={() => void startLogin()}><span className="ms-logo"><i/><i/><i/><i/></span>{loginBusy ? t('Подождите…') : t('Войти через Microsoft')}</button><button className="secondary account-restore" disabled={busy || loginBusy || !!device} onClick={() => run(restoreSavedAccount)}>{t('Восстановить сохранённый вход')}</button></div><span className="muted">{t('Вход через официальную страницу Microsoft.')}</span></>}
          {!profile && authDetails && authDetails.attempts.length > 0 && <AuthDetails diagnostics={authDetails} language={settings.language}/>}
          <div className="account-note"><ShieldCheck size={20}/><span>{t('Refresh token хранится в системном хранилище. Пароль и токены не передаются в интерфейс.')}</span></div>
        </div>
        <form className="panel offline-account" onSubmit={e => { e.preventDefault(); if (!offlineAllowed) { setMessage(t('OFFLINE_LOGIN_REQUIRED')); return; } if (!/^[A-Za-z0-9_]{1,16}$/.test(offlineDraft)) { setMessage(t('Ник: 1–16 латинских букв, цифр или _.')); return; } void run(async () => { const local = await selectOfflineProfile(offlineDraft); updateSettings({ offlineName: local.name }); return t('Оффлайн-профиль выбран.'); }); }}>
          <div className="offline-account-heading"><UserRound size={22}/><h3>{t('Оффлайн-профиль')}</h3></div><p>{t('Оффлайн-режим доступен после Microsoft-входа и проверки лицензии Minecraft Java. Выход из аккаунта блокирует его снова.')}</p>
          {offlineAllowed ? <p>{t('Можно играть без повторного входа. Серверы с проверкой лицензии недоступны. Первая установка требует интернета.')}</p> : <p className="offline-locked" role="status">{t('Сначала войдите через Microsoft или восстановите сохранённый вход.')}</p>}
          <label>{t('Ник')}<input value={offlineDraft} onChange={e => setOfflineDraft(e.target.value)} disabled={!offlineAllowed || busy || loginBusy || !!device} maxLength={16} pattern="[A-Za-z0-9_]{1,16}" required placeholder="Player"/></label>
          <button className="primary small" disabled={!offlineAllowed || busy || loginBusy || !!device}>{t('Выбрать оффлайн-профиль')}</button>
          {offlineAllowed && settings.offlineName && <><p>Offline: {settings.offlineName}</p><button className="secondary" type="button" disabled={busy || loginBusy || !!device} onClick={() => updateSettings({ offlineName: undefined })}>{t('Использовать Microsoft')}</button></>}
        </form>

      </section>}
      {screen === 'settings' && <SettingsPage settings={settings} current={current} storageError={storageError} onChange={updateSettings}
        onEditInstance={() => editInstance(current.id)} onOpenLogs={() => { setLogInstanceId(current.id); setLogsOpen(true); }}/>}
    </main>
    {(operationBusy || running) && <div className="operation-panel" role="status"><span>{running && !busy ? t('Игра запущена') : t(`stage:${progress?.stage ?? 'preparing'}`)}{busy && progress && progress.total > 0 ? ` · ${Math.round(progress.current / progress.total * 100)}%` : ''}</span>{busy && <progress value={progress && progress.total > 0 ? progress.current : undefined} max={progress?.total || 1}/>}{running && !busy && <button className="secondary" onClick={() => run(async () => { if (running) { await stopGame(); setRunning(false); return t('Игра остановлена.'); } await cancelOperation(); return t('Отмена запрошена.'); })} disabled={busy}>{t('Остановить игру')}</button>}{busy && <button className="secondary" onClick={() => { void cancelOperation().catch(e => setMessage(t(errorText(e)))); }}>{t('Отменить')}</button>}</div>}
    {storageError && <div className="storage-error" role="alert">{t("Не удалось сохранить настройки. Данные сохранятся только до закрытия окна.")}</div>}
    {message && <div className="notice" role="status"><span>{message}</span><button className="icon-button" onClick={() => setMessage('')} aria-label={t("Закрыть уведомление")}><X size={17}/></button></div>}
    <footer className="bottom-bar"><span className="build-label"><span className="status-dot"/>{desktop ? 'DESKTOP' : 'UI PREVIEW'} <span>0.2.0</span></span><nav aria-label={t("Основная навигация")} style={{ '--nav-index': tabs.findIndex(tab => tab.id === activeScreen) } as CSSProperties}>{tabs.map(tab => <button key={tab.id} disabled={modsBusy} aria-label={t(tab.name)} title={t(tab.name)} aria-current={activeScreen === tab.id ? 'page' : undefined} className={activeScreen === tab.id ? 'active' : ''} onClick={() => setScreen(tab.id)}><tab.icon size={19}/><span>{t(tab.name)}</span></button>)}</nav><span className="edition">MIDNIGHT <span>α</span></span></footer>
    <dialog className="instance-chooser" ref={chooserDialog} aria-label={t('Выбрать сборку')} onCancel={() => setChoosingInstance(false)}>
      <button className="icon-button dialog-close" aria-label={t('Закрыть')} onClick={() => setChoosingInstance(false)}><X size={20}/></button>
      <h2>{t('Выбрать сборку')}</h2>
      <div className="instance-choice-list">{state.instances.map(i => <button className={`instance-choice ${i.id === state.selected ? 'is-selected' : ''}`} key={i.id} aria-pressed={i.id === state.selected} disabled={busy || running} onClick={() => { setState(s => ({ ...s, selected: i.id })); setChoosingInstance(false); }}>
        <span className={`instance-miniature miniature-${i.accent ?? 'purple'}`} aria-hidden="true"><Layers3 size={21}/></span>
        <span className="home-instance-info"><strong>{i.name}</strong><span>{i.version} · {i.engine === 'horizon' ? 'Horizon · Rust' : i.loader ? `${loaderName(i.loader.kind)} ${i.loader.version}` : 'Vanilla'}</span></span>
        {i.id === state.selected && <Check size={18}/>}
      </button>)}</div>
      <button className="secondary instance-manage" onClick={() => { setChoosingInstance(false); setScreen('instances'); }}>{t('Управление сборками')}<ArrowRight size={16}/></button>
    </dialog>
    <dialog className="instance-dialog" ref={instanceDialog} aria-label={editingId ? t('Настроить сборку') : t('Новая сборка')} onCancel={() => { setCreating(false); setEditingId(null); }}>
      <button className="icon-button dialog-close" type="button" aria-label={t('Закрыть')} onClick={() => { setCreating(false); setEditingId(null); }}><X size={20}/></button>
      <form className="panel create-form instance-editor" onSubmit={addInstance}>
        <header className="instance-editor-heading"><span><Layers3 size={23}/></span><div><h3>{editingId ? t('Настроить сборку') : t('Новая сборка')}</h3><p>{t('Выбери версию. Остальное подготовим при запуске.')}</p></div></header>
        {message && <p className="editor-message" role="alert">{message}</p>}
        <label className="instance-name-field">{t('Название')}<input autoFocus maxLength={40} value={name} onChange={e => setName(e.target.value)} placeholder={t('Например, Тихая долина')} required/></label>
        <label>{t('Движок игры')}<Dropdown label={t('Движок игры')} value={engine} onChange={value => { setEngine(value as 'java' | 'horizon'); setVersion(value === 'horizon' ? horizonVersion : '1.21.1'); setLoaderKind('vanilla'); setLoaderVersion(''); }} options={[{value: 'java', label: 'Minecraft Java'}, {value: 'horizon', label: 'Horizon · Rust'}]}/></label>
        {engine === 'horizon' ? <div className="instance-engine-note"><Cpu size={19}/><p>{t('Horizon 0.2.97 · Minecraft 26.3. Клиент установится автоматически. Для подготовки нужны Microsoft-аккаунт с лицензией и Java 25. Моды Fabric и Forge не поддерживаются.')}</p></div> : <>
          <label>{t('Версия Minecraft')}<Dropdown editable label={t('Версия Minecraft')} value={version} onChange={value => { setVersion(value); setLoaderChoices([]); setLoaderVersion(''); }} options={versions.map(v => ({value: v.id, label: v.id}))} emptyLabel={t('Введите версию вручную или обновите список.')} required/></label>
          <label>{t('Загрузчик')}<Dropdown label={t('Загрузчик')} value={loaderKind} onChange={value => { setLoaderKind(value as 'vanilla' | LoaderKind); setLoaderVersion(''); setLoaderChoices([]); setLoaderError(''); }} options={[{value: 'vanilla', label: 'Vanilla'}, {value: 'fabric', label: 'Fabric'}, {value: 'forge', label: 'Forge'}, {value: 'neoforge', label: 'NeoForge'}, ...['Quilt'].map(name => ({value: name.toLowerCase(), label: `${name} — ${t('позже')}`, disabled: true}))]}/></label>
          {loaderKind !== 'vanilla' && <div className="instance-loader-fields">
            <label>{t('Версия загрузчика')}<Dropdown editable label={t('Версия загрузчика')} value={loaderVersion} onChange={setLoaderVersion} options={loaderChoices.map(value => ({value, label: value}))} emptyLabel={t(!desktop ? 'Введи версию загрузчика вручную.' : loadingLoaders ? 'Ищем версии загрузчика…' : 'Для этой версии Minecraft загрузчик не найден.')} required placeholder={loadingLoaders ? t('Загрузка…') : loaderName(loaderKind)}/></label>
            <button className="secondary" type="button" disabled={loadingLoaders || !desktop} onClick={() => setLoaderRevision(value => value + 1)}>{t(loadingLoaders ? 'Ищем версии загрузчика…' : 'Обновить версии')}</button>
            <p className={`instance-loader-status${loaderError ? ' has-error' : ''}`} role="status">{loaderError ? t(loaderError) : !desktop ? t('Список версий доступен в приложении. Версию можно ввести вручную.') : loadingLoaders ? t('Подбираем версии под Minecraft {version}.').replace('{version}', version) : loaderChoices.length ? t('Совместимые версии для Minecraft {version}.').replace('{version}', version) : t('Для этой версии Minecraft загрузчик не найден.')}</p>
          </div>}
        </>}
        <details className="instance-editor-advanced" open={Boolean(editingId)}><summary><Settings2 size={17}/><span>{t('Дополнительные параметры')}<small>{t('Память, Java и цвет карточки')}</small></span></summary><div className="instance-advanced-grid">
          {engine !== 'horizon' && <label>{t('Память сборки, МБ')}<input type="number" min="1024" max="16384" step="512" value={instanceMemory} placeholder={t('Как в настройках')} onChange={e => setInstanceMemory(e.target.value)}/></label>}
          <label>{t(engine === 'horizon' ? 'Java 25 для подготовки Horizon' : 'Java для сборки')}<input value={instanceJava} placeholder={t('Как в настройках')} onChange={e => setInstanceJava(e.target.value)}/></label>
          <label>{t('Цвет карточки')}<Dropdown label={t('Цвет карточки')} value={instanceAccent} onChange={v => setInstanceAccent(v as typeof instanceAccent)} options={[{value:'rose',label:t('Розовый сумрак')},{value:'purple',label:t('Классический фиолетовый')},{value:'graphite',label:t('Графит')}]}/></label>
        </div></details>
        <div className="instance-editor-actions"><button className="secondary" type="button" onClick={() => { setCreating(false); setEditingId(null); }}>{t('Отменить')}</button><button className="primary" type="submit" disabled={busy || running || (engine === 'java' && loaderKind !== 'vanilla' && loadingLoaders)}>{editingId ? t('Сохранить сборку') : t('Создать сборку')}<ArrowRight size={16}/></button></div>
      </form>
    </dialog>
    <dialog className="logs-dialog" ref={logsDialog} onCancel={() => setLogsOpen(false)}>
      <button className="icon-button dialog-close" aria-label={t('Закрыть')} onClick={() => setLogsOpen(false)}><X size={20}/></button>
      <h2>{t('Консоль игры')}</h2><div className="logs-toolbar"><Dropdown label={t('Сборка для просмотра логов')} value={logInstance.id} onChange={setLogInstanceId} options={state.instances.map(i => ({ value: i.id, label: `${i.name} · ${i.version}` }))}/><span className="logs-live-label"><span className="status-dot"/>{t('Обновляется автоматически')}</span></div>
      <label className="log-follow"><input type="checkbox" checked={followLogs} onChange={e => setFollowLogs(e.target.checked)}/>{t('Прокручивать к новым строкам')}</label>
      {message && <p role="status">{message}</p>}
      <pre ref={logOutput} className="log-output" tabIndex={0} aria-label={t('Вывод Minecraft')}>{logError ? t(logError) : logText || t('Логов пока нет. Они появятся после запуска Minecraft.')}</pre>
      <p className="muted">{t('Обновление каждую секунду · последние 256 КБ · stdout и stderr')}</p>
    </dialog>
    <dialog ref={dialog} onCancel={() => setHelp(false)} onClick={e => { if (e.target === e.currentTarget) setHelp(false); }}><button className="icon-button dialog-close" onClick={() => setHelp(false)} aria-label={t("Закрыть")}><X size={20}/></button><Moon size={30}/><h2>Midnight Launcher</h2><p>{t("Десктопный Minecraft Java лаунчер для Windows x64 и Apple Silicon.")}<br/>Tauri 2 · React · TypeScript · Rust</p><p>{t("Инстансы и настройки сохраняются локально.")}<br/>{t("Для запуска нужны подходящая Java, лицензия Minecraft и настроенный Microsoft Client ID.")}<br/>{t("Установщик загружает Minecraft, библиотеки, ресурсы и Java для вашей платформы.")}</p><p className="about-quote" lang="ru">Если одноклассницы снимают липсинги, тогда бедварсеры снимают клипсинги</p><button className="primary small" onClick={() => setHelp(false)}>{t("Понятно")}</button></dialog>
  </div>;
}
