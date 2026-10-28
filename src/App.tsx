import { themes } from './themes';
import { Dropdown } from './components/Dropdown';
import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { Terminal, ArrowRight, Check, CircleHelp, Cpu, Gamepad2, Layers3, Play, Search, Pencil, Moon, Plus, Settings2, ShieldCheck, Trash2, UserRound, X } from 'lucide-react';
import { loaderVersions, readGameLog, desktop, launchGame, listVersions, beginLogin, pollLogin, cancelLogin, openLogin, getAccount, restoreAccount, signOut, gameStatus, stopGame, cancelOperation, type Version, type Profile, type DevicePrompt, type Progress } from './api';
import { listen } from '@tauri-apps/api/event';
import { translate } from './i18n';
import { restoreState, storageKey, validateInstance } from './model';
type Screen = 'home' | 'instances' | 'accounts' | 'settings';
const tabs = [{ id: 'home', name: 'Главная', icon: Gamepad2 }, { id: 'instances', name: 'Инстансы', icon: Layers3 }, { id: 'accounts', name: 'Аккаунты', icon: UserRound }, { id: 'settings', name: 'Настройки', icon: Settings2 }] as const;
const errorText = (e: unknown) => e instanceof Error ? e.message : String(e);
export default function App() {
  const [state, setState] = useState(() => { try { return restoreState(localStorage.getItem(storageKey)); } catch { return restoreState(null); } });
  const [screen, setScreen] = useState<Screen>('home');
  const [message, setMessage] = useState('');
  const [storageError, setStorageError] = useState(false);
  const [busy, setBusy] = useState(false);
  const [profile, setProfile] = useState<Profile | null>(null);
  const [device, setDevice] = useState<DevicePrompt | null>(null);
  const [offlineDraft, setOfflineDraft] = useState('');
  const [loginBusy, setLoginBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [running, setRunning] = useState(false);
  const [versions, setVersions] = useState<Version[]>([]);
  const [name, setName] = useState(''); const [version, setVersion] = useState('1.21.1');
  const [loaderKind, setLoaderKind] = useState<'vanilla' | 'fabric'>('vanilla');
  const [loaderVersion, setLoaderVersion] = useState('');
  const [loaderChoices, setLoaderChoices] = useState<string[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [instanceMemory, setInstanceMemory] = useState('');
  const [instanceJava, setInstanceJava] = useState('');
  const [instanceAccent, setInstanceAccent] = useState<'rose' | 'purple' | 'graphite'>('purple');
  const [choosingInstance, setChoosingInstance] = useState(false);
  const chooserDialog = useRef<HTMLDialogElement>(null);
  const [creating, setCreating] = useState(false);
  const [search, setSearch] = useState('');
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
  useEffect(() => { if (creating) instanceDialog.current?.showModal(); else instanceDialog.current?.close(); }, [creating]);
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
    if (!device) return;
    let active = true; let timer: number;
    const check = async () => {
      try {
        const result = await pollLogin(device.id);
        if (!active) return;
        if (result.state === 'connected' && result.profile) { setProfile(result.profile); updateSettings({ offlineName: undefined }); setDevice(null); setMessage(t('Аккаунт Minecraft подключён.')); }
        else timer = window.setTimeout(check, Math.max(5, result.interval) * 1000);
      } catch (error) { if (active) { setDevice(null); setMessage(t(errorText(error))); } }
    };
    timer = window.setTimeout(check, Math.max(5, device.interval) * 1000);
    return () => { active = false; window.clearTimeout(timer); };
  }, [device, settings.language]);
  useEffect(() => {
    if (!creating || !desktop) return;
    let active = true;
    listVersions().then(items => { if (active) setVersions(items); }).catch(error => { if (active) setMessage(errorText(error)); });
    return () => { active = false; };
  }, [creating]);
  function editInstance(id: string) {
    setMessage('');
    const instance = state.instances.find(i => i.id === id)!;
    setEditingId(id); setName(instance.name); setVersion(instance.version);
    setLoaderKind(instance.loader ? 'fabric' : 'vanilla'); setLoaderVersion(instance.loader?.version ?? '');
    setLoaderChoices([]); setInstanceMemory(instance.memory ? String(instance.memory) : '');
    setInstanceJava(instance.javaPath ?? ''); setInstanceAccent(instance.accent ?? 'purple');
    setCreating(true); setScreen('instances');
  }
  function newInstance() {
    setMessage('');
    setEditingId(null); setName(''); setVersion('1.21.1'); setLoaderKind('vanilla'); setLoaderVersion(''); setLoaderChoices([]);
    setInstanceMemory(''); setInstanceJava(''); setInstanceAccent('purple'); setCreating(true);
  }
  async function startLogin() {
    if (loginBusy || device) return;
    setLoginBusy(true); setMessage('');
    try { setDevice(await beginLogin(settings.clientId)); } catch (error) { setMessage(t(errorText(error))); } finally { setLoginBusy(false); }
  }
  async function stopLogin() { setDevice(null); try { await cancelLogin(); } catch (error) { setMessage(t(errorText(error))); } }

  function updateSettings(patch: Partial<typeof settings>) { setState(s => ({ ...s, settings: { ...s.settings, ...patch } })); }
  async function run(action: () => Promise<string>) { if (busy) return; setBusy(true); setMessage(''); setProgress(null); try { setMessage(await action()); } catch (e) { setMessage(t(errorText(e))); } finally { setBusy(false); setProgress(null); } }
  function addInstance(e: React.FormEvent) {
    e.preventDefault(); const error = validateInstance(name, version); if (error) { setMessage(t(error)); return; }
    if (!editingId && state.instances.length >= 100) { setMessage(t('Достигнут лимит: 100 инстансов.')); return; }
    if (loaderKind === 'fabric' && !/^[A-Za-z0-9._+-]{1,64}$/.test(loaderVersion)) { setMessage(t('Выберите версию Fabric.')); return; }
    const instance = { id: editingId ?? crypto.randomUUID(), name: name.trim(), version, memory: instanceMemory ? Number(instanceMemory) : undefined, javaPath: instanceJava.trim() || undefined, accent: instanceAccent, ...(loaderKind === 'fabric' ? { loader: { kind: 'fabric' as const, version: loaderVersion } } : {}) };
    if (instance.memory !== undefined && (!Number.isInteger(instance.memory) || instance.memory < 1024 || instance.memory > 16384)) { setMessage(t('Укажите память от 1024 до 16384 МБ.')); return; }
    setState(s => ({ ...s, selected: instance.id, instances: editingId ? s.instances.map(i => i.id === editingId ? instance : i) : [...s.instances, instance] })); setCreating(false); setEditingId(null); setName(''); setMessage(t('Сборка сохранена. Недостающие файлы установятся при запуске.'));
  }
  return <div className={`app ${settings.reduceMotion ? 'reduce-motion' : ''}`} data-theme={settings.theme} data-gradient={settings.gradient}>
    <header className="topbar"><button className="brand" onClick={() => setScreen('home')} aria-label={t("Midnight Launcher — главная")}><span className="brand-mark"><Moon size={20}/></span><span>MIDNIGHT</span></button><span className="top-caption">{t("ТВОИ МИРЫ. ТВОЁ ВРЕМЯ.")}</span><button className="icon-button help" aria-label={t("О проекте")} onClick={() => setHelp(true)}><CircleHelp size={19}/></button></header>
    <main ref={content}>
      {screen === 'home' && <section className="home page">
        <div className="home-title"><h1>MIDNIGHT</h1><img src="/reference/app-icon.png" width="60" height="60" alt="" className="reference-icon"/></div>
        <div className="launch-area">
        <div className="launch-row"><button className="launch-button" aria-busy={busy} disabled={busy || running} onClick={() => run(async () => { const result = await launchGame(current.id, current.version, current.javaPath ?? settings.javaPath, current.memory ?? settings.memory, settings.offlineName, current.loader, settings.minimizeOnLaunch ?? false, settings.restoreOnExit ?? true); setRunning(result.running); return t('Игра запущена.'); })}><img src="/reference/launch-overlay.svg" className="launch-overlay" alt=""/><span><Play size={17} fill="currentColor"/>{running ? t('Игра запущена') : busy ? t('Подождите…') : 'Launch'}</span></button></div>
        <button className="home-instance-card" onClick={() => setChoosingInstance(true)} disabled={busy || running} aria-label={t('Выбрать сборку')}>
          <span className={`instance-miniature miniature-${current.accent ?? 'purple'}`} aria-hidden="true"><Layers3 size={21}/></span>
          <span className="home-instance-info"><strong>{current.name}</strong><span>{current.version}<span className="instance-meta-dot">·</span>{current.loader ? 'Fabric' : 'Vanilla'}</span></span>
          <span className="instance-change">{t('Сменить')}<ArrowRight size={15}/></span>
        </button></div>

      </section>}
      {screen === 'instances' && <section className="page content-page"><div className="section-heading"><div><div className="eyebrow">{t("ТВОЯ КОЛЛЕКЦИЯ")}</div><h2>{t("Инстансы")}<span>.</span></h2><p>{t("Отдельное место для каждой истории.")}</p></div><button className="primary small" onClick={newInstance}><Plus size={17}/>{t("Создать")}</button></div>

        <div className="instance-search"><Search size={17}/><input aria-label={t("Поиск инстансов")} placeholder={t("Поиск инстансов")} value={search} onChange={e => setSearch(e.target.value)}/></div>
        {!state.instances.some(i => i.name.toLowerCase().includes(search.toLowerCase())) && <p className="empty-state">{t("Инстансы не найдены.")}</p>}
        <div className="instance-grid">{state.instances.filter(i => i.name.toLowerCase().includes(search.toLowerCase())).map((i, index) => <article className={`instance-card ${i.id === state.selected ? 'selected' : ''}`} key={i.id} style={{ '--card-order': Math.min(index, 6) } as CSSProperties}><div className={`card-art art-${index % 3} cover-${i.accent ?? 'purple'}`}><div className="voxel v1"/><div className="voxel v2"/><div className="voxel v3"/><span>JAVA EDITION</span>{i.id === state.selected && <span className="selected-tag"><Check size={12}/>{t("Выбран")}</span>}</div><div className="card-body"><h3>{i.name}</h3><p>{i.loader ? `Fabric ${i.loader.version}` : 'Vanilla'} · {i.version}</p><div className="card-actions"><button className="icon-button" aria-label={`${t('Настроить сборку')} ${i.name}`} disabled={busy || running} onClick={() => editInstance(i.id)}><Pencil size={15}/></button><button onClick={() => setState(s => ({ ...s, selected: i.id }))} disabled={i.id === state.selected}>{i.id === state.selected ? t('Активный') : t('Выбрать')}<ArrowRight size={15}/></button><button className="icon-button" aria-label={`${t('Удалить')} ${i.name}`} disabled={state.instances.length === 1} onClick={() => setState(s => ({ ...s, instances: s.instances.filter(x => x.id !== i.id), selected: s.selected === i.id ? s.instances.find(x => x.id !== i.id)!.id : s.selected }))}><Trash2 size={15}/></button></div></div></article>)}</div>

      </section>}
      {screen === 'accounts' && <section className="page content-page"><div className="section-heading"><div><div className="eyebrow">{t('ТВОЯ ИДЕНТИЧНОСТЬ')}</div><h2>{t('Аккаунты')}<span>.</span></h2><p>{t('Один профиль. Бесконечно много миров.')}</p></div></div>
        <div className="account-panel panel"><div className="account-avatar"><UserRound size={34}/></div>
          {device ? <><span className="badge">{t('ПОДТВЕРЖДЕНИЕ MICROSOFT')}</span><h3>{t('Введите код на сайте Microsoft')}</h3><p>{t('Откройте официальный сайт, введите код и завершите вход. Пароль вводится только на Microsoft.')}</p><code className="device-code">{device.userCode}</code><p className="device-uri">{device.verificationUri}</p><p>{t('Код действует {minutes} мин. Ожидаем подтверждение…').replace('{minutes}', String(Math.ceil(device.expiresIn / 60)))}</p><button className="primary small" onClick={() => run(async () => { await openLogin(device.id); return t('Microsoft открыт в системном браузере.'); })}>{t('Открыть Microsoft')}</button><button className="secondary" onClick={() => void stopLogin()}>{t('Отменить вход')}</button></>
          : profile ? <><span className="badge">MINECRAFT JAVA</span><h3>{profile.name}</h3><p>{t('Аккаунт сохранён. Перед запуском сессия обновляется и лицензия проверяется.')}</p><button className="secondary" disabled={busy} onClick={() => run(async () => { await signOut(); setProfile(null); return t('Вы вышли из аккаунта.'); })}>{t('Выйти из аккаунта')}</button></>
          : <><span className="badge">{t('НЕТ ПОДКЛЮЧЁННОГО АККАУНТА')}</span><h3>{t('Твоё место в мире.')}</h3><p>{t('Вход через Microsoft. Нужна лицензия Minecraft Java Edition.')}</p><button className="microsoft-button" disabled={loginBusy || !!device} onClick={() => void startLogin()}><span className="ms-logo"><i/><i/><i/><i/></span>{loginBusy ? t('Подождите…') : t('Войти через Microsoft')}</button><button className="secondary" disabled={busy || loginBusy || !!device} onClick={() => run(async () => { const saved = await restoreAccount(); setProfile(saved); updateSettings({ offlineName: undefined }); return t('Аккаунт Minecraft подключён.'); })}>{t('Восстановить сохранённый вход')}</button><span className="muted">{t('Вход через официальную страницу Microsoft.')}</span></>}
          <div className="account-note"><ShieldCheck size={20}/><span>{t('Refresh token хранится в Keychain. Пароль и токены не передаются в интерфейс.')}</span></div>
        </div>
        <form className="panel offline-account" onSubmit={e => { e.preventDefault(); if (!/^[A-Za-z0-9_]{1,16}$/.test(offlineDraft)) { setMessage(t('Ник: 1–16 латинских букв, цифр или _.')); return; } updateSettings({ offlineName: offlineDraft }); setMessage(t('Оффлайн-профиль выбран.')); }}>
          <div className="offline-account-heading"><UserRound size={22}/><h3>{t('Оффлайн-профиль')}</h3></div><p>{t('Локальная игра без Microsoft. Серверы с проверкой лицензии недоступны. Первая установка требует интернета.')}</p>
          <label>{t('Ник')}<input value={offlineDraft} onChange={e => setOfflineDraft(e.target.value)} maxLength={16} pattern="[A-Za-z0-9_]{1,16}" required placeholder="Player"/></label>
          <button className="primary small" disabled={busy || loginBusy || !!device}>{t('Выбрать оффлайн-профиль')}</button>
          {settings.offlineName && <><p>Offline: {settings.offlineName}</p><button className="secondary" type="button" onClick={() => updateSettings({ offlineName: undefined })}>{t('Использовать Microsoft')}</button></>}
        </form>

      </section>}
      {screen === 'settings' && <section className="page content-page"><div className="section-heading"><div><div className="eyebrow">{t("ВСЁ ПОД КОНТРОЛЕМ")}</div><h2>{t("Настройки")}<span>.</span></h2><p>{t("Твой лаунчер. Твои правила.")}</p></div><span className="saved"><Check size={14}/>{t("Автосохранение")}</span></div><div className="settings-stack">
        <div className="panel setting launcher-options"><h3>{t('Поведение лаунчера')}</h3>
          <label>{t('После запуска игры')}<Dropdown label={t('После запуска игры')} value={settings.minimizeOnLaunch ? 'minimize' : 'keep'} onChange={v => updateSettings({minimizeOnLaunch: v === 'minimize'})} options={[{value:'keep',label:t('Оставлять открытым')},{value:'minimize',label:t('Сворачивать лаунчер')}]}/></label>
          <label className="launcher-checkbox"><input type="checkbox" checked={settings.restoreOnExit ?? true} onChange={e => updateSettings({restoreOnExit:e.target.checked})}/>{t('Возвращать окно после выхода из игры')}</label>
        </div>
        <button type="button" className="logs-action" aria-label={t('Открыть логи')} onClick={() => { setLogInstanceId(current.id); setLogsOpen(true); }}>
          <span className="logs-action-icon"><Terminal size={22}/></span>
          <span className="logs-action-copy"><strong>{t('Консоль игры')}</strong><span>{t('Вывод запуска и ошибки Minecraft')}</span></span>
          <span className="logs-action-instance">{current.name}</span><ArrowRight size={18}/>
        </button>

        <div className="panel setting preference-row settings-language"><div><h3>{t('Язык интерфейса')}</h3><p>{t('Перевод применяется сразу и сохраняется на этом устройстве.')}</p></div><Dropdown label={t('Язык интерфейса')} value={settings.language} onChange={value => updateSettings({ language: value as 'ru' | 'en' })} options={[{value: 'ru', label: 'Русский'}, {value: 'en', label: 'English'}]}/></div>
        <div className="panel setting settings-appearance"><h3>{t('Тема оформления')}</h3><p>{t('Выберите палитру для всех экранов лаунчера.')}</p><div className="theme-options" role="group" aria-label={t('Тема оформления')}>
          {themes.map(theme => <button key={theme.id} className={`theme-choice ${theme.id === settings.theme ? 'chosen' : ''}`} aria-pressed={theme.id === settings.theme} onClick={() => updateSettings({ theme: theme.id })}><span className={`theme-swatch swatch-${theme.id}`} aria-hidden="true"/><span>{t(theme.name)}</span>{theme.id === settings.theme && <Check size={16}/>}</button>)}
        </div><div className="theme-gradient toggle-row"><div><h3>{t('Градиент фона')}</h3><p>{t('Плавное переливание оттенков выбранной темы.')}</p></div><button role="switch" aria-checked={settings.gradient} aria-label={t('Градиент фона')} className={`toggle ${settings.gradient ? 'on' : ''}`} onClick={() => updateSettings({ gradient: !settings.gradient })}><span/></button></div></div><div className="panel setting settings-memory"><div className="setting-title"><Cpu size={21}/><div><h3>{t("Оперативная память")}</h3><p>{t("Максимальная память для запуска Minecraft.")}</p></div><strong>{(settings.memory / 1024).toFixed(1)} GB</strong></div><input aria-label={t("Оперативная память")} type="range" min="1024" max="16384" step="512" value={settings.memory} onChange={e => updateSettings({ memory: Number(e.target.value) })}/><div className="range-labels"><span>1 GB</span><span>16 GB</span></div></div><div className="panel setting settings-java"><label>{t("Путь к Java")}<input value={settings.javaPath} onChange={e => updateSettings({ javaPath: e.target.value })} placeholder={t("Автоматически (ARM64 Java по версии Minecraft)")}/></label><p>{t("Оставьте пустым для автоматической установки Java. Собственный путь проверяется по версии и архитектуре.")}</p></div><div className="panel setting toggle-row settings-motion"><div><h3>{t("Меньше движения")}</h3><p>{t("Отключить декоративную анимацию интерфейса.")}</p></div><button role="switch" aria-checked={settings.reduceMotion} aria-label={t("Меньше движения")} className={`toggle ${settings.reduceMotion ? 'on' : ''}`} onClick={() => updateSettings({ reduceMotion: !settings.reduceMotion })}><span/></button></div></div></section>}
    </main>
    {(busy || running) && <div className="operation-panel" role="status"><span>{running && !busy ? t('Игра запущена') : t(`stage:${progress?.stage ?? 'preparing'}`)}{busy && progress && progress.total > 0 ? ` · ${Math.round(progress.current / progress.total * 100)}%` : ''}</span>{busy && <progress value={progress && progress.total > 0 ? progress.current : undefined} max={progress?.total || 1}/>}{running && !busy && <button className="secondary" onClick={() => run(async () => { if (running) { await stopGame(); setRunning(false); return t('Игра остановлена.'); } await cancelOperation(); return t('Отмена запрошена.'); })} disabled={busy}>{t('Остановить игру')}</button>}{busy && <button className="secondary" onClick={() => { void cancelOperation().catch(e => setMessage(t(errorText(e)))); }}>{t('Отменить')}</button>}</div>}
    {storageError && <div className="storage-error" role="alert">{t("Не удалось сохранить настройки. Данные сохранятся только до закрытия окна.")}</div>}
    {message && <div className="notice" role="status"><span>{message}</span><button className="icon-button" onClick={() => setMessage('')} aria-label={t("Закрыть уведомление")}><X size={17}/></button></div>}
    <footer className="bottom-bar"><span className="build-label"><span className="status-dot"/>{desktop ? 'DESKTOP' : 'UI PREVIEW'} <span>0.2.0</span></span><nav aria-label={t("Основная навигация")} style={{ '--nav-index': tabs.findIndex(tab => tab.id === screen) } as CSSProperties}>{tabs.map(tab => <button key={tab.id} aria-label={t(tab.name)} title={t(tab.name)} aria-current={screen === tab.id ? 'page' : undefined} className={screen === tab.id ? 'active' : ''} onClick={() => setScreen(tab.id)}><tab.icon size={19}/><span>{t(tab.name)}</span></button>)}</nav><span className="edition">MIDNIGHT <span>α</span></span></footer>
    <dialog className="instance-chooser" ref={chooserDialog} aria-label={t('Выбрать сборку')} onCancel={() => setChoosingInstance(false)}>
      <button className="icon-button dialog-close" aria-label={t('Закрыть')} onClick={() => setChoosingInstance(false)}><X size={20}/></button>
      <h2>{t('Выбрать сборку')}</h2>
      <div className="instance-choice-list">{state.instances.map(i => <button className={`instance-choice ${i.id === state.selected ? 'is-selected' : ''}`} key={i.id} aria-pressed={i.id === state.selected} disabled={busy || running} onClick={() => { setState(s => ({ ...s, selected: i.id })); setChoosingInstance(false); }}>
        <span className={`instance-miniature miniature-${i.accent ?? 'purple'}`} aria-hidden="true"><Layers3 size={21}/></span>
        <span className="home-instance-info"><strong>{i.name}</strong><span>{i.version} · {i.loader ? `Fabric ${i.loader.version}` : 'Vanilla'}</span></span>
        {i.id === state.selected && <Check size={18}/>}
      </button>)}</div>
      <button className="secondary instance-manage" onClick={() => { setChoosingInstance(false); setScreen('instances'); }}>{t('Управление сборками')}<ArrowRight size={16}/></button>
    </dialog>
    <dialog className="instance-dialog" ref={instanceDialog} aria-label={editingId ? t('Настроить сборку') : t('Создать инстанс')} onCancel={() => { setCreating(false); setEditingId(null); }}>
      <button className="icon-button dialog-close" type="button" aria-label={t('Закрыть')} onClick={() => { setCreating(false); setEditingId(null); }}><X size={20}/></button>
        <form className="panel create-form instance-editor" onSubmit={addInstance}><h3>{editingId ? t('Настроить сборку') : t('Создать инстанс')}</h3>{message && <p className="editor-message" role="alert">{message}</p>}<label>{t("Название")}<input autoFocus maxLength={40} value={name} onChange={e => setName(e.target.value)} placeholder={t("Например, Тихая долина")} required/></label><label>{t("Версия")}<Dropdown editable label={t('Версия')} value={version} onChange={value => { setVersion(value); setLoaderChoices([]); setLoaderVersion(''); }} options={versions.map(v => ({value: v.id, label: v.id}))} emptyLabel={t('Введите версию вручную или обновите список.')} required/></label><label>{t('Загрузчик')}<Dropdown label={t('Загрузчик')} value={loaderKind} onChange={value => setLoaderKind(value as 'vanilla' | 'fabric')} options={[{value: 'vanilla', label: 'Vanilla'}, {value: 'fabric', label: 'Fabric'}, ...['Quilt', 'Forge', 'NeoForge'].map(name => ({value: name.toLowerCase(), label: `${name} — ${t('позже')}`, disabled: true}))]}/></label>
        {loaderKind === 'fabric' && <><label>{t('Версия загрузчика')}<Dropdown editable label={t('Версия загрузчика')} value={loaderVersion} onChange={setLoaderVersion} options={loaderChoices.map(v => ({value: v, label: v}))} emptyLabel={t('Получите список версий или введите вручную.')} required placeholder="0.16.14"/></label><button className="secondary" type="button" disabled={busy} onClick={() => run(async () => { const choices = await loaderVersions(version); setLoaderChoices(choices); setLoaderVersion(choices[0] ?? ''); return t('Версии загрузчика обновлены.'); })}>{t('Получить версии Fabric')}</button></>}
        <label>{t('Память сборки, МБ')}<input type="number" min="1024" max="16384" step="512" value={instanceMemory} placeholder={t('Как в настройках')} onChange={e => setInstanceMemory(e.target.value)}/></label>
        <label>{t('Java для сборки')}<input value={instanceJava} placeholder={t('Как в настройках')} onChange={e => setInstanceJava(e.target.value)}/></label>
        <label>{t('Цвет карточки')}<Dropdown label={t('Цвет карточки')} value={instanceAccent} onChange={v => setInstanceAccent(v as typeof instanceAccent)} options={[{value:'rose',label:t('Розовый сумрак')},{value:'purple',label:t('Классический фиолетовый')},{value:'graphite',label:t('Графит')}]}/></label>
        <button className="primary small" type="submit" disabled={busy || running}>{editingId ? t('Сохранить сборку') : t('Создать инстанс')}</button><button className="secondary" type="button" onClick={() => { setCreating(false); setEditingId(null); }}>{t('Отменить')}</button></form>
    </dialog>
    <dialog className="logs-dialog" ref={logsDialog} onCancel={() => setLogsOpen(false)}>
      <button className="icon-button dialog-close" aria-label={t('Закрыть')} onClick={() => setLogsOpen(false)}><X size={20}/></button>
      <h2>{t('Консоль игры')}</h2><div className="logs-toolbar"><Dropdown label={t('Сборка для просмотра логов')} value={logInstance.id} onChange={setLogInstanceId} options={state.instances.map(i => ({ value: i.id, label: `${i.name} · ${i.version}` }))}/><span className="logs-live-label"><span className="status-dot"/>{t('Обновляется автоматически')}</span></div>
      <label className="log-follow"><input type="checkbox" checked={followLogs} onChange={e => setFollowLogs(e.target.checked)}/>{t('Прокручивать к новым строкам')}</label>
      {message && <p role="status">{message}</p>}
      <pre ref={logOutput} className="log-output" tabIndex={0} aria-label={t('Вывод Minecraft')}>{logError ? t(logError) : logText || t('Логов пока нет. Они появятся после запуска Minecraft.')}</pre>
      <p className="muted">{t('Обновление каждую секунду · последние 256 КБ · stdout и stderr')}</p>
    </dialog>
    <dialog ref={dialog} onCancel={() => setHelp(false)} onClick={e => { if (e.target === e.currentTarget) setHelp(false); }}><button className="icon-button dialog-close" onClick={() => setHelp(false)} aria-label={t("Закрыть")}><X size={20}/></button><Moon size={30}/><h2>Midnight Launcher</h2><p>{t("Десктопный Minecraft Java лаунчер для Apple Silicon.")}<br/>Tauri 2 · React · TypeScript · Rust</p><p>{t("Инстансы и настройки сохраняются локально.")}<br/>{t("Для запуска нужны подходящая Java, лицензия Minecraft и настроенный Microsoft Client ID.")}<br/>{t("Установщик загружает Minecraft, библиотеки, ресурсы и ARM64 Java.")}</p><p className="about-quote" lang="ru">Если одноклассницы снимают липсинги, тогда бедварсеры снимают клипсинги</p><button className="primary small" onClick={() => setHelp(false)}>{t("Понятно")}</button></dialog>
  </div>;
}
