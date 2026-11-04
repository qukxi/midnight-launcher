import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, ArrowRight, ArrowUpRight, Check, Compass, Download, ExternalLink, Info, Layers3, LoaderCircle, Search, ShieldCheck, Sparkles, X } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { cancelOperation, desktop, installModpack, modpackVersions, openModpackProject, searchModpacks, type CatalogPage, type CatalogProject, type CatalogSort, type ModpackRelease, type Progress } from '../api';
import { loaderName, type Instance, type Language } from '../model';
import { translate } from '../i18n';
import { Dropdown } from './Dropdown';
import { ProjectIcon } from './ModCatalog';

interface Props {
  language: Language;
  locked: boolean;
  running: boolean;
  instanceCount: number;
  onBusyChange: (busy: boolean) => void;
  onInstalled: (instance: Instance, files: number) => void;
  onBack: () => void;
}
const messageOf = (error: unknown) => error instanceof Error ? error.message : String(error);

export function ModpackCatalog({ language, locked, running, instanceCount, onBusyChange, onInstalled, onBack }: Props) {
  const t = (text: string) => translate(language, text);
  const [query, setQuery] = useState('');
  const [submitted, setSubmitted] = useState('');
  const [offset, setOffset] = useState(0);
  const [sort, setSort] = useState<CatalogSort>('downloads');
  const [revision, setRevision] = useState(0);
  const [page, setPage] = useState<CatalogPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [project, setProject] = useState<CatalogProject | null>(null);
  const [releases, setReleases] = useState<ModpackRelease[]>([]);
  const [releasesFor, setReleasesFor] = useState('');
  const [releaseId, setReleaseId] = useState('');
  const [versionsLoading, setVersionsLoading] = useState(false);
  const [versionError, setVersionError] = useState('');
  const [includeOptional, setIncludeOptional] = useState(true);
  const [working, setWorking] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const mutation = useRef(false);
  const detail = useRef<HTMLElement>(null);
  const results = useRef<HTMLDivElement>(null);
  const selected = project && releasesFor === project.id ? releases.find(v => v.id === releaseId) : undefined;
  const canInstall = desktop && !locked && !working && !running && instanceCount < 100;
  const number = (value: number) => new Intl.NumberFormat(language, { notation: 'compact', maximumFractionDigits: 1 }).format(value);

  useEffect(() => {
    if (!desktop) return;
    let active = true;
    setLoading(true); setError(''); setPage(null); setProject(null);
    searchModpacks(submitted, offset, sort).then(value => { if (active) setPage(value); })
      .catch(e => { if (active) setError(messageOf(e)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [submitted, offset, sort, revision]);

  useEffect(() => {
    setReleases([]); setReleaseId(''); setReleasesFor(''); setVersionError('');
    if (!project) return;
    let active = true;
    setVersionsLoading(true);
    modpackVersions(project.id).then(values => {
      if (active) { setReleases(values); setReleasesFor(project.id); setReleaseId(values[0]?.id ?? ''); }
    }).catch(e => { if (active) setVersionError(messageOf(e)); })
      .finally(() => { if (active) setVersionsLoading(false); });
    return () => { active = false; };
  }, [project?.id]);

  useEffect(() => {
    if (project && window.matchMedia('(max-width: 940px), (max-height: 680px)').matches) detail.current?.scrollIntoView({ block: 'nearest' });
  }, [project?.id]);

  useEffect(() => {
    if (!working) return;
    let active = true;
    const unlisten = listen<Progress>('launcher-progress', e => {
      if (active && e.payload.stage.startsWith('modpack-')) setProgress(e.payload);
    });
    return () => { active = false; void unlisten.then(stop => stop()).catch(() => {}); };
  }, [working]);

  function search(value: string) {
    if (working || loading) return;
    setQuery(value); setSubmitted(value.trim()); setOffset(0); setRevision(n => n + 1);
  }
  async function installSelected() {
    if (!project || !selected || !canInstall || mutation.current) return;
    mutation.current = true;
    setWorking(true); onBusyChange(true); setError(''); setProgress(null); setCancelling(false);
    try {
      const result = await installModpack(project.id, selected.id, includeOptional);
      onInstalled(result.instance, result.files);
    } catch (e) { setError(messageOf(e)); }
    finally { mutation.current = false; setWorking(false); setCancelling(false); onBusyChange(false); }
  }
  async function cancel() {
    setCancelling(true);
    try { await cancelOperation(); }
    catch (e) { setError(messageOf(e)); setCancelling(false); }
  }

  return <section className="page content-page mods-page is-browsing modpack-page">
    <div className="section-heading mods-heading"><div><div className="eyebrow">MODRINTH</div><h2>{t('Готовые сборки')}<span>.</span></h2><p>{t('Моды уже подобраны. Осталось выбрать свой мир.')}</p></div><button className="secondary modpack-back" disabled={working} onClick={onBack}><ArrowLeft size={15}/>{t('Мои инстансы')}</button></div>
    {error && <div className="mods-feedback mods-error" role="alert"><Info size={16}/><span>{t(error)}</span><button className="icon-button" aria-label={t('Закрыть')} onClick={() => setError('')}><X size={16}/></button></div>}
    <div className={`catalog catalog-modrinth ${project ? 'has-selection' : ''}`}>
      <div className="catalog-main">
        <section className="catalog-discovery" aria-label={t('Поиск готовых сборок')}>
          <div className="catalog-discovery-copy"><span className="catalog-provider-label"><Compass size={13}/>MODRINTH<span/>{t('ГОТОВЫЕ СБОРКИ')}</span><h3>{t('Новая история в один клик.')}<Sparkles size={21}/></h3><p>{t('Сборки Fabric, Forge и NeoForge. Моды и настройки автора установятся в новый инстанс.')}</p></div>
          <div className="catalog-discovery-search">
            <form className="catalog-search-row" onSubmit={e => { e.preventDefault(); search(query); }}><div className="catalog-search-field"><Search size={19}/><input aria-label={t('Поиск готовых сборок')} placeholder={t('Какую сборку ищем?')} maxLength={200} value={query} disabled={working || !desktop} onChange={e => setQuery(e.target.value)}/>{query && <button className="icon-button" type="button" aria-label={t('Очистить поиск')} disabled={working || loading} onClick={() => search('')}><X size={15}/></button>}</div><button className="catalog-search-submit" aria-label={t('Найти')} disabled={working || loading || !desktop}><ArrowRight size={19}/></button></form>
            <div className="catalog-suggestions"><span>{t('Попробуй:')}</span>{['Fabulously Optimized', 'Simply Optimized'].map(value => <button key={value} disabled={working || loading || !desktop} onClick={() => search(value)}>{value}<ArrowUpRight size={10}/></button>)}</div>
          </div>
        </section>
        <div className="catalog-results-heading"><div><h3>{t(submitted ? 'Результаты поиска' : 'Сборки Modrinth')}</h3><span className="catalog-result-count">{loading ? '…' : page ? number(page.total) : '—'}</span></div><label className="catalog-sort"><span>{t('Сортировка')}</span><Dropdown label={t('Сортировка сборок')} value={sort} onChange={value => { if (!working) { setSort(value as CatalogSort); setOffset(0); } }} options={[
          { value: 'relevance', label: t('По релевантности'), disabled: working || !desktop },
          { value: 'downloads', label: t('По загрузкам'), disabled: working || !desktop },
          { value: 'updated', label: t('Недавно обновлённые'), disabled: working || !desktop },
          { value: 'newest', label: t('Сначала новые'), disabled: working || !desktop },
        ]}/></label></div>
        <div className="catalog-results" ref={results} aria-busy={loading}>
          {!desktop ? <div className="mods-empty catalog-state-panel"><Layers3 size={32}/><h3>{t('Сборки доступны в приложении')}</h3><p>{t('Открой Midnight на компьютере, чтобы скачать готовую сборку.')}</p></div>
            : loading ? <div className="catalog-grid catalog-skeletons" role="status" aria-label={t('Загружаем каталог…')}>{Array.from({ length: 6 }, (_, index) => <div className="catalog-skeleton" key={index}><span/><i/><i/><i/></div>)}</div>
            : page && !page.projects.length ? <div className="mods-empty catalog-state-panel"><Search size={28}/><h3>{t('Ничего не нашлось')}</h3><p>{t('Попробуй другое название сборки.')}</p><button className="secondary" onClick={() => search('')}>{t('Показать популярное')}</button></div>
            : page ? <div className="catalog-grid">{page.projects.map(p => <button key={p.id} className={`catalog-card ${project?.id === p.id ? 'is-selected' : ''}`} disabled={working} aria-pressed={project?.id === p.id} aria-label={`${t('Выбрать сборку')} ${p.title}`} onClick={() => setProject(p)}><span className="catalog-card-top"><ProjectIcon project={p}/><span className="catalog-loader-tag">Java</span></span><span className="catalog-card-body"><strong>{p.title}</strong><span className="catalog-author">{p.author}</span><span className="catalog-description">{p.description}</span></span><span className="catalog-card-footer"><span><Download size={12}/>{number(p.downloads)}</span><span className="catalog-card-open">{t('Подробнее')}{project?.id === p.id ? <Check size={14}/> : <ArrowUpRight size={14}/>}</span></span></button>)}</div>
            : !error && <div className="mods-empty catalog-state-panel"><button className="secondary" onClick={() => setRevision(n => n + 1)}>{t('Обновить каталог')}</button></div>}
          {page && <div className="catalog-pagination"><span>{page.total ? `${offset + 1}–${Math.min(offset + page.projects.length, page.total)} / ${number(page.total)}` : '0'}</span><div><button className="secondary" disabled={!offset || working || loading} onClick={() => { setOffset(n => Math.max(0, n - 20)); results.current?.scrollIntoView({ block: 'start' }); }}><ArrowLeft size={14}/>{t('Назад')}</button><button className="secondary" disabled={!page.hasMore || working || loading} onClick={() => { setOffset(n => n + 20); results.current?.scrollIntoView({ block: 'start' }); }}>{t('Далее')}<ArrowRight size={14}/></button></div></div>}
        </div>
      </div>
      {project && <aside className="catalog-detail" ref={detail} aria-label={t('Установка сборки')}>
        <div className="catalog-detail-top"><span className="catalog-kicker">{t('НОВЫЙ ИНСТАНС')}</span><button className="icon-button" aria-label={t('Закрыть карточку сборки')} disabled={working} onClick={() => setProject(null)}><X size={17}/></button></div>
        <div className="catalog-detail-identity"><ProjectIcon key={project.id} project={project} large/><div><h3>{project.title}</h3><p className="catalog-detail-author">{project.author}</p></div></div>
        <div className="catalog-detail-content">
          <p className="catalog-detail-description">{project.description}</p>
          {versionsLoading || !versionError && releasesFor !== project.id ? <p className="catalog-loading" role="status"><LoaderCircle className="mods-spinner" size={15}/>{t('Ищем версии сборки…')}</p>
            : versionError ? <p className="catalog-version-error" role="alert">{t(versionError)}</p>
            : !releases.length ? <p className="catalog-version-error">{t('У этой сборки нет поддерживаемых версий.')}</p>
            : <label className="catalog-version-select"><span>{t('Версия сборки')}</span><Dropdown label={t('Версия сборки')} value={releaseId} onChange={value => { if (!working) setReleaseId(value); }} options={releases.map(v => ({ value: v.id, label: `${v.number || v.name} · ${v.gameVersions.slice(0, 2).join(', ')}`, disabled: working }))}/></label>}
          {selected && <dl className="catalog-file-details"><div><dt>Minecraft</dt><dd>{selected.gameVersions.join(', ')}</dd></div><div><dt>{t('Загрузчик')}</dt><dd>{selected.loaders?.map(loaderName).join(', ') || 'Minecraft Java'}</dd></div><div><dt>{t('Релиз')}</dt><dd>{t(selected.channel === 'beta' ? 'Бета' : selected.channel === 'alpha' ? 'Альфа' : 'Релиз')}</dd></div></dl>}
          <div className="modpack-option"><span>{t('Необязательные моды')}<small>{t('Добавить рекомендованные автором файлы')}</small></span><button className={`toggle ${includeOptional ? 'on' : ''}`} role="switch" aria-checked={includeOptional} aria-label={t('Необязательные моды')} disabled={working} onClick={() => setIncludeOptional(value => !value)}><span/></button></div>
        </div>
        <div className="catalog-detail-footer">
          {running && <p className="catalog-version-error">{t('Останови игру перед установкой сборки.')}</p>}
          {instanceCount >= 100 && <p className="catalog-version-error">{t('Достигнут лимит: 100 инстансов.')}</p>}
          <button className="primary catalog-install-button" disabled={!canInstall || !selected || versionsLoading} onClick={() => void installSelected()}>{working ? <LoaderCircle className="mods-spinner" size={16}/> : <Download size={16}/>} {t(working ? 'Устанавливаем…' : 'Установить сборку')}</button>
          <span className="catalog-install-note"><ShieldCheck size={14}/>{t('Создадим отдельный инстанс с модами и настройками. Minecraft и Java подготовятся при первом запуске.')}</span>
          {working && <div className="modpack-progress" role="status"><span>{t(`stage:${progress?.stage ?? 'modpack-archive'}`)}{progress?.total ? ` · ${progress.current}/${progress.total}` : ''}</span>{!!progress?.total && <progress value={progress.current} max={progress.total}/>}<button className="secondary" disabled={cancelling} onClick={() => void cancel()}>{t(cancelling ? 'Отменяем…' : 'Отменить')}</button></div>}
          <button className="catalog-text-link catalog-project-page" disabled={working} onClick={() => { void openModpackProject(project.id).catch(e => setError(messageOf(e))); }}>{t('Страница сборки')}<ExternalLink size={12}/></button>
        </div>
      </aside>}
    </div>
  </section>;
}
