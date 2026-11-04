import { useEffect, useRef, useState } from 'react';
import { ArrowRight, Check, ChevronRight, Compass, FileArchive, Flame, FolderOpen, Info, Layers3, LoaderCircle, PackageOpen, Plus, Puzzle, RefreshCw, Search, X } from 'lucide-react';
import { desktop, importMod, listMods, openModsFolder, setModEnabled, type ModFile } from '../api';
import { loaderName, type Instance, type Language } from '../model';
import { translate } from '../i18n';
import { Dropdown } from './Dropdown';
import { ModCatalog } from './ModCatalog';

interface Props {
  instances: Instance[];
  instanceId: string;
  onInstanceChange: (id: string) => void;
  language: Language;
  locked: boolean;
  running: boolean;
  onBusyChange: (busy: boolean) => void;
  onConfigure: (id: string) => void;
}
type Filter = 'all' | 'enabled' | 'disabled';

function fileSize(bytes: number, language: Language) {
  const unit = bytes >= 1024 * 1024 ? 1024 * 1024 : 1024;
  return `${new Intl.NumberFormat(language, { maximumFractionDigits: 1 }).format(bytes / unit)} ${unit === 1024 ? (language === 'ru' ? 'КБ' : 'KB') : (language === 'ru' ? 'МБ' : 'MB')}`;
}

export function ModsManager({ instances, instanceId, onInstanceChange, language, locked, running, onBusyChange, onConfigure }: Props) {
  const t = (text: string) => translate(language, text);
  const instance = instances.find(i => i.id === instanceId) ?? instances[0];
  const native = instance.engine === 'horizon';
  const [mods, setMods] = useState<ModFile[]>([]);
  const [loadedFor, setLoadedFor] = useState('');
  const [loading, setLoading] = useState(false);
  const [working, setWorking] = useState(false);
  const [pendingFile, setPendingFile] = useState('');
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<Filter>('all');
  const [revision, setRevision] = useState(0);
  const [view, setView] = useState<'installed' | 'modrinth' | 'curseforge'>('modrinth');
  const input = useRef<HTMLInputElement>(null);
  const mutation = useRef(false);
  const operation = useRef(0);
  const shown = loadedFor === instance.id ? mods : [];
  const enabled = shown.filter(m => m.enabled).length;
  const available = desktop && !native;
  const canChange = available && !locked && !working && !running;

  useEffect(() => {
    setQuery(''); setFilter('all'); setError(''); setNotice('');
    operation.current += 1;
    return () => { operation.current += 1; };
  }, [instance.id]);

  useEffect(() => {
    if (!available) return;
    let active = true;
    setLoading(true);
    listMods(instance.id).then(files => {
      if (active) { setMods(files); setLoadedFor(instance.id); }
    }).catch(e => { if (active) setError(String(e instanceof Error ? e.message : e)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [instance.id, available, revision]);

  async function change(action: () => Promise<void>) {
    if (!canChange || mutation.current) return;
    mutation.current = true;
    setWorking(true); onBusyChange(true); setError(''); setNotice('');
    try { await action(); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { mutation.current = false; setWorking(false); setPendingFile(''); onBusyChange(false); setRevision(n => n + 1); }
  }

  async function addFiles(files: File[]) {
    await change(async () => {
      const errors: string[] = [];
      let added = 0;
      const token = operation.current;
      for (const file of files) {
        // A batch never follows the user into another instance or a closed view.
        if (token !== operation.current) break;
        setPendingFile(file.name);
        try { await importMod(instance.id, file); added += 1; }
        catch (e) { errors.push(`${file.name}: ${t(e instanceof Error ? e.message : String(e))}`); }
      }
      if (added) setNotice(t('Добавлено файлов: {count}').replace('{count}', String(added)));
      if (errors.length) setError(errors.join('\n'));
    });
  }

  const filtered = shown.filter(m => (filter === 'all' || m.enabled === (filter === 'enabled')) && m.fileName.toLowerCase().includes(query.trim().toLowerCase()));
  const filters: { id: Filter; label: string; count: number }[] = [
    { id: 'all', label: 'Все', count: shown.length },
    { id: 'enabled', label: 'Активные', count: enabled },
    { id: 'disabled', label: 'Отключённые', count: shown.length - enabled },
  ];

  return <section className={`page content-page mods-page ${view !== 'installed' ? 'is-browsing' : ''}`}>
    <div className="section-heading mods-heading">
      <div><div className="eyebrow">{t('БИБЛИОТЕКА MIDNIGHT')}</div><h2>{t('Моды')}<span>.</span></h2><p>{t('Твоя сборка. Твои правила.')}</p></div>
      <div className="mods-file-actions"><button className="secondary mods-folder" disabled={!available || locked || working} onClick={async () => { setError(''); try { await openModsFolder(instance.id); } catch (e) { setError(e instanceof Error ? e.message : String(e)); } }}><FolderOpen size={16}/>{t('Папка модов')}</button><button className="secondary mods-import" disabled={!canChange} onClick={() => input.current?.click()}><Plus size={16}/>{t('Из файла')}</button></div>
      <input ref={input} className="mods-file-input" type="file" accept=".jar" multiple tabIndex={-1} aria-label={t('Выбрать JAR-файлы')} onChange={e => { const files = Array.from(e.currentTarget.files ?? []); e.currentTarget.value = ''; if (files.length) void addFiles(files); }}/>
    </div>

    <div className="mods-workspace">
      <div className="mods-source-tabs" role="group" aria-label={t('Источник модов')}>
        <button aria-pressed={native || view === 'installed'} disabled={working} onClick={() => setView('installed')}><PackageOpen size={15}/>{t('Установленные')}<span>{shown.length}</span></button>
        <button aria-pressed={!native && view === 'modrinth'} disabled={working || native} onClick={() => setView('modrinth')}><Compass size={15}/>Modrinth</button>
        <button aria-pressed={!native && view === 'curseforge'} disabled={working || native} onClick={() => setView('curseforge')}><Flame size={15}/>CurseForge</button>
      </div>
      <div className="mods-target"><Layers3 size={17}/><div><span className="mods-caption">{t('ТЕКУЩАЯ СБОРКА')}</span><Dropdown label={t('Сборка для модов')} value={instance.id} onChange={id => { if (!working) onInstanceChange(id); }} options={instances.map(i => ({ value: i.id, label: i.name, disabled: working }))}/></div><span className="mods-target-version">{instance.version}<span>{native ? 'Horizon' : instance.loader ? loaderName(instance.loader.kind) : 'Vanilla'}</span></span></div>
    </div>

    {native ? <div className="mods-empty panel"><span className="mods-empty-icon"><PackageOpen size={32}/></span><h3>{t('Другой движок. Другие возможности.')}</h3><p>{t('Horizon не поддерживает Java-моды. Выбери сборку Minecraft Java, чтобы управлять файлами .jar.')}</p></div>
      : <>
        {!instance.loader && view === 'installed' && <div className="mods-hint"><Info size={17}/><p>{t('Выбери Fabric, Forge или NeoForge в настройках сборки. Файлы модов можно добавить заранее.')}</p><button onClick={() => onConfigure(instance.id)} disabled={locked || running}>{t('Настроить')}<ArrowRight size={14}/></button></div>}
        {running && <div className="mods-hint"><Info size={17}/><p>{t('Останови игру, чтобы добавить моды или изменить их состояние.')}</p></div>}
        {error && <div className="mods-feedback mods-error" role="alert"><Info size={17}/><span>{t(error)}</span><button className="icon-button" aria-label={t('Закрыть')} onClick={() => setError('')}><X size={16}/></button></div>}
        {notice && <div className="mods-feedback" role="status"><Check size={17}/><span>{notice}</span><button className="icon-button" aria-label={t('Закрыть')} onClick={() => setNotice('')}><X size={16}/></button></div>}
        {view !== 'installed' ? <ModCatalog key={`${view}/${instance.id}/${instance.version}`} source={view} instance={instance} language={language} working={working} canChange={canChange} onAction={change} onNotice={setNotice} onConfigure={() => onConfigure(instance.id)}/> : <div className="mods-library panel" aria-busy={loading || working}>
          <div className="mods-toolbar">
            <div className="mods-filters" role="group" aria-label={t('Фильтр модов')}>{filters.map(f => <button key={f.id} aria-pressed={filter === f.id} onClick={() => setFilter(f.id)}>{t(f.label)}<span>{loadedFor === instance.id ? f.count : '—'}</span></button>)}</div>
            <div className="mods-search"><Search size={16}/><input aria-label={t('Поиск модов')} placeholder={t('Поиск модов')} value={query} onChange={e => setQuery(e.target.value)}/>{query && <button className="icon-button" aria-label={t('Очистить поиск')} onClick={() => setQuery('')}><X size={14}/></button>}</div>
            <button className="icon-button mods-refresh" aria-label={t('Обновить список модов')} title={t('Обновить список модов')} disabled={!available || loading || working} onClick={() => { setError(''); setRevision(n => n + 1); }}><RefreshCw size={16}/></button>
          </div>

          {!desktop ? <div className="mods-empty"><span className="mods-empty-icon"><FolderOpen size={32}/></span><h3>{t('Твои моды — в десктопном приложении')}</h3><p>{t('Открой Midnight на компьютере, чтобы добавлять файлы и управлять модами своей сборки.')}</p><span className="mods-empty-detail">.jar <ChevronRight size={12}/> Minecraft Java</span></div>
            : loading && loadedFor !== instance.id ? <div className="mods-empty" role="status"><LoaderCircle className="mods-spinner" size={28}/><p>{t('Читаем папку модов…')}</p></div>
            : !shown.length ? <div className="mods-empty"><span className="mods-empty-icon"><Puzzle size={34}/></span><h3>{t('Всё начинается с одного мода')}</h3><p>{t('Добавь любимые .jar-файлы. Включай нужные и сохраняй остальные на потом.')}</p><button className="secondary" disabled={!canChange} onClick={() => input.current?.click()}><Plus size={16}/>{t('Выбрать файлы')}</button><span className="mods-empty-detail">{t('Несколько файлов сразу · до 512 МиБ каждый')}</span></div>
            : !filtered.length ? <div className="mods-empty"><Search size={28}/><h3>{t('Ничего не нашлось')}</h3><p>{t('Попробуй другое название или измени фильтр.')}</p><button className="secondary" onClick={() => { setQuery(''); setFilter('all'); }}>{t('Сбросить фильтры')}</button></div>
            : <ul className="mods-list" aria-label={t('Установленные моды')}>{filtered.map(mod => <li key={mod.fileName} className={`mod-row ${mod.enabled ? '' : 'is-disabled'}`}>
              <span className="mod-file-icon" aria-hidden="true"><FileArchive size={22}/></span>
              <div className="mod-file-info"><strong title={mod.fileName}>{mod.fileName.replace(/\.jar(?:\.disabled)?$/i, '')}</strong><span title={mod.fileName}>{mod.fileName}<span className="mod-file-size">{fileSize(mod.size, language)}</span></span></div>
              <span className="mod-state"><span/>{t(mod.enabled ? 'Включён' : 'Отключён')}</span>
              <button className={`toggle ${mod.enabled ? 'on' : ''}`} role="switch" aria-checked={mod.enabled} aria-label={`${t('Переключить мод')} ${mod.fileName}`} disabled={!canChange} onClick={() => void change(async () => { setPendingFile(mod.fileName); await setModEnabled(instance.id, mod.fileName, !mod.enabled); })}><span/></button>
            </li>)}</ul>}

          <div className="mods-library-footer"><span>{working ? <><LoaderCircle className="mods-spinner" size={13}/>{pendingFile || t('Подождите…')}</> : <><PackageOpen size={14}/>{t('Отключённые моды остаются в папке сборки')}</>}</span><span>{shown.length ? fileSize(shown.reduce((sum, m) => sum + m.size, 0), language) : '.JAR'}</span></div>
        </div>}
      </>}
  </section>;
}

