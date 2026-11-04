import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, ArrowRight, ArrowUpRight, Box, Check, CheckCheck, ChevronRight, Compass, Download, ExternalLink, Flame, Info, KeyRound, LoaderCircle, PackageOpen, Search, ShieldCheck, Sparkles, X } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { cancelOperation, catalogStatus, catalogVersions, connectCurseForge, desktop, disconnectCurseForge, installCatalogMod, openCatalogProject, openCurseForgeSetup, searchCatalog, type CatalogPage, type CatalogProject, type CatalogRelease, type CatalogSource, type Progress } from '../api';
import { loaderName, type Instance, type Language } from '../model';
import type { CatalogSort } from '../api';
import { translate } from '../i18n';
import { Dropdown } from './Dropdown';

interface Props {
  source: CatalogSource;
  instance: Instance;
  language: Language;
  working: boolean;
  canChange: boolean;
  onAction: (action: () => Promise<void>) => Promise<void>;
  onNotice: (message: string) => void;
  onConfigure: () => void;
}
const messageOf = (error: unknown) => error instanceof Error ? error.message : String(error);
const suggestions = ['Sodium', 'Iris', 'Mod Menu'];

export function ProjectIcon({ project, large = false }: { project: CatalogProject; large?: boolean }) {
  const [failed, setFailed] = useState(false);
  const initials = project.title.replace(/[^\p{L}\p{N}\s]/gu, '').split(/\s+/).filter(Boolean).slice(0, 2).map(word => word[0]).join('').toUpperCase();
  return <span className={`catalog-project-icon ${large ? 'is-large' : ''}`} aria-hidden="true">
    {project.iconUrl && !failed ? <img src={project.iconUrl} alt="" loading="lazy" decoding="async" referrerPolicy="no-referrer" onError={() => setFailed(true)}/> : <span>{initials || <Box size={24}/>}</span>}
  </span>;
}

export function ModCatalog({ source, instance, language, working, canChange, onAction, onNotice, onConfigure }: Props) {
  const t = (text: string) => translate(language, text);
  const curse = source === 'curseforge';
  const Icon = curse ? Flame : Compass;
  const [connected, setConnected] = useState(false);
  const [statusReady, setStatusReady] = useState(!curse);
  const [key, setKey] = useState('');
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState('');
  const [query, setQuery] = useState('');
  const [submitted, setSubmitted] = useState('');
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState<CatalogSort>('downloads');
  const [revision, setRevision] = useState(0);
  const [page, setPage] = useState<CatalogPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [project, setProject] = useState<CatalogProject | null>(null);
  const [releases, setReleases] = useState<CatalogRelease[]>([]);
  const [releasesFor, setReleasesFor] = useState('');
  const [releaseId, setReleaseId] = useState('');
  const [versionsLoading, setVersionsLoading] = useState(false);
  const [versionError, setVersionError] = useState('');
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<Progress | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const detail = useRef<HTMLElement>(null);
  const searchField = useRef<HTMLInputElement>(null);
  const results = useRef<HTMLDivElement>(null);
  const selected = project && releasesFor === project.id ? releases.find(v => v.id === releaseId) : undefined;
  const alreadyInstalled = project && installed.has(`${project.id}/${releaseId}`);
  const ready = desktop && (!curse || connected);
  const number = (value: number) => new Intl.NumberFormat(language, { notation: 'compact', maximumFractionDigits: 1 }).format(value);
  const downloading = progress?.stage === 'catalog-download' && progress.total > 0;
  const progressDetail = !progress?.total ? '' : downloading
    ? `${new Intl.NumberFormat(language, { maximumFractionDigits: 1 }).format(progress.current / 1024 / 1024)} / ${new Intl.NumberFormat(language, { maximumFractionDigits: 1 }).format(progress.total / 1024 / 1024)} ${t('МиБ')}`
    : `${progress.current}/${progress.total}`;
  const date = (value: string) => {
    const parsed = new Date(value);
    return Number.isNaN(parsed.valueOf()) ? '—' : new Intl.DateTimeFormat(language, { day: 'numeric', month: 'short', year: 'numeric' }).format(parsed);
  };
  const channel = (value: string) => t(value === 'release' ? 'Релиз' : value === 'beta' ? 'Бета' : 'Альфа');

  useEffect(() => {
    if (project && window.matchMedia('(max-width: 940px), (max-height: 680px)').matches) detail.current?.scrollIntoView({ block: 'nearest' });
  }, [project?.id]);

  useEffect(() => {
    if (!curse || !desktop) return;
    let active = true;
    catalogStatus().then(value => { if (active) setConnected(value); })
      .catch(e => { if (active) setError(messageOf(e)); })
      .finally(() => { if (active) setStatusReady(true); });
    return () => { active = false; };
  }, [curse]);

  useEffect(() => {
    if (!ready) return;
    let active = true;
    setLoading(true); setError(''); setPage(null); setProject(null);
    searchCatalog(source, submitted, instance.version, offset, sort, instance.loader?.kind).then(value => { if (active) setPage(value); })
      .catch(e => { if (active) setError(messageOf(e)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [source, ready, submitted, offset, sort, instance.version, instance.loader?.kind, revision]);

  useEffect(() => {
    setReleases([]); setReleaseId(''); setReleasesFor(''); setVersionError('');
    if (!project) return;
    let active = true;
    setVersionsLoading(true);
    catalogVersions(source, project.id, instance.version, instance.loader?.kind).then(values => {
      if (active) {
        setReleases(values); setReleasesFor(project.id);
        setReleaseId((values.find(v => v.canDownload) ?? values[0])?.id ?? '');
      }
    }).catch(e => { if (active) setVersionError(messageOf(e)); })
      .finally(() => { if (active) setVersionsLoading(false); });
    return () => { active = false; };
  }, [source, project?.id, instance.version, instance.loader?.kind]);

  useEffect(() => {
    if (!working || !desktop) { setProgress(null); setCancelling(false); return; }
    let active = true;
    const unlisten = listen<Progress>('launcher-progress', e => {
      if (active && e.payload.stage.startsWith('catalog-')) setProgress(e.payload);
    });
    return () => { active = false; void unlisten.then(stop => stop()).catch(() => {}); };
  }, [working]);

  function search(value: string) {
    if (working || loading) return;
    setQuery(value); setOffset(0); setSubmitted(value.trim()); setRevision(n => n + 1);
  }
  function selectProject(value: CatalogProject) {
    if (working) return;
    setProject(value);
  }
  async function connect(e: React.FormEvent) {
    e.preventDefault();
    if (connecting) return;
    const draft = key;
    setKey(''); setConnecting(true); setError('');
    try { await connectCurseForge(draft); setConnected(true); }
    catch (e) { setError(messageOf(e)); }
    finally { setConnecting(false); }
  }
  async function external(action: () => Promise<void>) {
    try { await action(); } catch (e) { setError(messageOf(e)); }
  }
  async function installSelected() {
    if (!project || !selected || !canChange || !instance.loader || versionsLoading) return;
    await onAction(async () => {
      const result = await installCatalogMod(source, project.id, selected.id, instance);
      setInstalled(s => new Set(s).add(`${project.id}/${selected.id}`));
      let message = t('Установлено: {installed}. Уже в сборке: {reused}.').replace('{installed}', String(result.installed)).replace('{reused}', String(result.reused));
      if (result.disabled) message += ` ${t('Отключённых файлов: {count}. Включи их во вкладке «Установленные».').replace('{count}', String(result.disabled))}`;
      onNotice(message);
    });
  }

  return <div className={`catalog catalog-${source} ${project ? 'has-selection' : ''}`}>
    <div className="catalog-main">
    {error && <div className="mods-feedback mods-error" role="alert"><Info size={16}/><span>{t(error)}</span><button className="icon-button" aria-label={t('Закрыть')} onClick={() => setError('')}><X size={16}/></button></div>}

    <section className="catalog-discovery" aria-label={t('Поиск модов')}>
      <div className="catalog-discovery-copy">
        <span className="catalog-provider-label"><Icon size={13}/>{curse ? 'CURSEFORGE' : 'MODRINTH'}<span/>{t('КАТАЛОГ МОДОВ')}</span>
        <h3>{t('Больше, чем просто Minecraft.')}<Sparkles size={21} aria-hidden="true"/></h3>
        <p>{t('Новые возможности. Знакомые миры. Найди мод для своей следующей истории.')}</p>
      </div>
      <div className="catalog-discovery-search">
        <form className="catalog-search-row" onSubmit={e => { e.preventDefault(); search(query); }}>
          <div className="catalog-search-field"><Search size={19}/><input ref={searchField} aria-label={t('Поиск в каталоге')} placeholder={t('Какой мод ищем?')} maxLength={200} value={query} onChange={e => setQuery(e.target.value)} disabled={working || !ready}/>{query && <button type="button" className="icon-button" aria-label={t('Очистить поиск')} disabled={working || loading} onClick={() => { search(''); searchField.current?.focus(); }}><X size={15}/></button>}</div>
          <button className="catalog-search-submit" disabled={working || loading || !ready} aria-label={t('Найти')}><ArrowRight size={19}/></button>
        </form>
        <div className="catalog-suggestions"><span>{t('Попробуй:')}</span>{suggestions.map(value => <button key={value} disabled={!ready || working || loading} onClick={() => search(value)}>{value}<ArrowUpRight size={10}/></button>)}</div>
      </div>
      <div className="catalog-discovery-orbit" aria-hidden="true"><Box/><span/><span/></div>
    </section>

    {!desktop ? <div className="mods-empty catalog-state-panel"><Compass size={32}/><h3>{t('Каталоги доступны в приложении')}</h3><p>{t('Открой Midnight на компьютере, чтобы искать и устанавливать моды.')}</p></div>
      : !statusReady ? <div className="mods-empty catalog-state-panel" role="status"><LoaderCircle className="mods-spinner" size={26}/><p>{t('Подождите…')}</p></div>
      : curse && !connected ? <section className="catalog-connect-layout">
        <div className="catalog-connect-story"><span className="catalog-connect-emblem"><Flame size={42}/></span><span className="catalog-kicker">{t('ЕЩЁ ОДИН МИР МОДОВ')}</span><h3>{t('Твои любимые моды.')}<br/><span>{t('Теперь и с CurseForge.')}</span></h3><p>{t('Подключи свой ключ, чтобы искать моды и устанавливать их прямо в сборку.')}</p><div className="catalog-connect-benefits"><span><CheckCheck size={16}/>{t('Версии под твою сборку')}</span><span><PackageOpen size={16}/>{t('Зависимости автоматически')}</span></div></div>
        <form className="catalog-connect" onSubmit={connect}><div><KeyRound size={22}/><h3>{t('Подключить CurseForge')}</h3><p>{t('Нужен собственный API-ключ из консоли CurseForge.')}</p></div>
          <label>{t('API-ключ CurseForge')}<input type="password" autoComplete="off" spellCheck={false} maxLength={512} value={key} onChange={e => setKey(e.target.value)} disabled={connecting} required placeholder={t('Вставь свой ключ')}/></label>
          <button className="primary" disabled={connecting || !key.trim()}>{connecting ? <LoaderCircle className="mods-spinner" size={15}/> : <KeyRound size={15}/>} {t(connecting ? 'Подключаем…' : 'Подключить')}</button>
          <button className="catalog-text-link" type="button" onClick={() => void external(openCurseForgeSetup)}>{t('Получить API-ключ')}<ExternalLink size={13}/></button>
          <span className="catalog-key-note"><ShieldCheck size={15}/>{t('Ключ хранится только до закрытия лаунчера. В настройки и файлы он не записывается.')}</span>
        </form>
      </section>
      : <>
        <div className="catalog-results-heading"><div><h3>{t(submitted ? 'Результаты поиска' : sort === 'downloads' ? 'Популярное в каталоге' : 'Каталог модов')}</h3><span className="catalog-result-count">{loading ? '…' : page ? number(page.total) : '—'}</span>{submitted && <span className="catalog-query-label">{submitted}</span>}</div>
          <label className="catalog-sort"><span>{t('Сортировка')}</span><Dropdown label={t('Сортировка модов')} value={sort} onChange={value => { if (!working) { setSort(value as CatalogSort); setOffset(0); } }} options={[
            { value: 'relevance', label: t(curse ? 'По популярности' : 'По релевантности'), disabled: working },
            { value: 'downloads', label: t('По загрузкам'), disabled: working },
            { value: 'updated', label: t('Недавно обновлённые'), disabled: working },
            { value: 'newest', label: t('Сначала новые'), disabled: working },
          ]}/></label>
          <span className="catalog-match-label"><CheckCheck size={14}/>{t('Для Minecraft {version} · {loader}').replace('{version}', instance.version).replace('{loader}', loaderName(instance.loader?.kind ?? 'fabric'))}</span>
          {curse && <button className="catalog-disconnect" disabled={working} onClick={() => void external(async () => { await disconnectCurseForge(); setConnected(false); setPage(null); setProject(null); })}>{t('Отключить CurseForge')}</button>}
        </div>

        <div className={`catalog-browse-layout ${project ? 'has-selection' : ''}`}>
          <div className="catalog-results" ref={results}>
            {loading ? <div className="catalog-grid catalog-skeletons" role="status" aria-label={t('Загружаем каталог…')}>{Array.from({ length: 6 }, (_, index) => <div className="catalog-skeleton" key={index}><span/><i/><i/><i/></div>)}</div>
              : page && !page.projects.length ? <div className="mods-empty catalog-state-panel"><Search size={28}/><h3>{t('Ничего не нашлось')}</h3><p>{t('Попробуй другое название или другую версию Minecraft.')}</p><button className="secondary" onClick={() => search('')}>{t('Показать популярное')}</button></div>
              : page ? <div className="catalog-grid">{page.projects.map(p => <button className={`catalog-card ${project?.id === p.id ? 'is-selected' : ''}`} key={p.id} disabled={working} onClick={() => selectProject(p)} aria-pressed={project?.id === p.id} aria-label={`${t('Выбрать мод')} ${p.title}`}>
                <span className="catalog-card-top"><ProjectIcon key={p.iconUrl ?? p.id} project={p}/><span className="catalog-loader-tag">{loaderName(instance.loader?.kind ?? 'fabric')}</span></span>
                <span className="catalog-card-body"><strong>{p.title}</strong><span className="catalog-author">{p.author}</span><span className="catalog-description">{p.description}</span></span>
                <span className="catalog-card-footer"><span><Download size={12}/>{number(p.downloads)}</span><span className="catalog-card-open">{p.canDownload ? t('Подробнее') : t('На сайте')}{project?.id === p.id ? <Check size={14}/> : <ArrowUpRight size={14}/>}</span></span>
              </button>)}</div> : !error && <div className="mods-empty catalog-state-panel"><button className="secondary" onClick={() => setRevision(n => n + 1)}>{t('Обновить каталог')}</button></div>}

            {page && <div className="catalog-pagination"><span>{page.total ? `${offset + 1}–${Math.min(offset + page.projects.length, page.total)} / ${number(page.total)}` : '0'}</span><div><button className="secondary" disabled={offset === 0 || loading || working} onClick={() => { setOffset(n => Math.max(0, n - 20)); results.current?.scrollIntoView({ block: 'start' }); }}><ArrowLeft size={14}/>{t('Назад')}</button><button className="secondary" disabled={!page.hasMore || loading || working} onClick={() => { setOffset(n => n + 20); results.current?.scrollIntoView({ block: 'start' }); }}>{t('Далее')}<ArrowRight size={14}/></button></div></div>}
          </div>


        </div>
      </>}
    </div>
          {project && <aside ref={detail} className="catalog-detail" aria-label={t('Установка мода')}>
            <div className="catalog-detail-top"><span className="catalog-kicker">{t('ДОБАВИТЬ В СБОРКУ')}</span><button className="icon-button" aria-label={t('Закрыть карточку мода')} disabled={working} onClick={() => setProject(null)}><X size={17}/></button></div>
            <div className="catalog-detail-identity"><ProjectIcon key={project.iconUrl ?? project.id} project={project} large/><div><h3>{project.title}</h3><p className="catalog-detail-author">{project.author}</p><span className="catalog-detail-downloads"><Download size={12}/>{number(project.downloads)} {t('загрузок')}</span></div></div>
            <div className="catalog-detail-content">
            <div className="catalog-detail-target"><Box size={17}/><div><span>{t('Установить в')}</span><strong>{instance.name}</strong></div><span>{instance.version}</span></div>
            {versionsLoading || !versionError && releasesFor !== project.id ? <p className="catalog-loading" role="status"><LoaderCircle className="mods-spinner" size={15}/>{t('Ищем совместимые версии…')}</p>
              : versionError ? <p className="catalog-version-error" role="alert">{t(versionError)}</p>
              : !releases.length ? <p className="catalog-version-error">{t('Совместимых файлов для этой версии Minecraft нет.')}</p>
              : <label className="catalog-version-select"><span>{t('Версия мода')}{selected && <span className="catalog-release-tag">{channel(selected.channel)}</span>}</span><Dropdown label={t('Версия мода')} value={releaseId} onChange={value => { if (!working) setReleaseId(value); }} options={releases.map(v => ({ value: v.id, label: v.number || v.name, disabled: working }))}/></label>}
            {selected && <dl className="catalog-file-details"><div><dt>{t('Обновлён')}</dt><dd>{date(selected.published)}</dd></div><div><dt>{t('Размер файла')}</dt><dd>{number(selected.size / 1024 / 1024)} {t('МБ')}</dd></div></dl>}
            {(!project.canDownload || selected && !selected.canDownload) && <div className="catalog-restriction"><Info size={16}/><span>{t('Автор ограничил стороннюю загрузку. Скачай файл на сайте и добавь его через «Добавить моды».')}</span></div>}
            </div>
            <div className="catalog-detail-footer">
            {!instance.loader && <div className="catalog-loader-required"><Info size={14}/><span>{t('Для установки выбери Fabric, Forge или NeoForge.')}</span><button disabled={working} onClick={onConfigure}>{t('Настроить')}<ChevronRight size={12}/></button></div>}
            <button className="primary catalog-install-button" disabled={!canChange || !instance.loader || !project.canDownload || !selected?.canDownload || versionsLoading || !!alreadyInstalled} onClick={() => void installSelected()}>{working ? <LoaderCircle className="mods-spinner" size={16}/> : alreadyInstalled ? <Check size={16}/> : <Download size={16}/>} {t(working ? 'Устанавливаем…' : alreadyInstalled ? 'В сборке' : 'Установить мод')}</button>
            <span className="catalog-install-note"><ShieldCheck size={14}/>{t('Обязательные зависимости установятся вместе с модом.')}</span>
            <button className="catalog-text-link catalog-project-page" disabled={working} onClick={() => void external(() => openCatalogProject(source, project.slug))}>{t('Страница мода')}<ExternalLink size={12}/></button>
            {working && <div className="catalog-progress" role="status"><span>{t(`stage:${progress?.stage ?? 'catalog-resolving'}`)}{progressDetail ? ` · ${progressDetail}` : ''}</span><button className="secondary" disabled={cancelling} onClick={() => { setCancelling(true); void external(cancelOperation); }}>{t(cancelling ? 'Отменяем…' : 'Отменить')}</button>{downloading && <progress aria-label={t('stage:catalog-download')} value={progress.current} max={progress.total}/>}</div>}
            </div>
          </aside>}
  </div>;
}
