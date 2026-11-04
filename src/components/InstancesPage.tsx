import { useEffect, useId, useRef, useState } from 'react';
import { ArrowRight, Check, Compass, Cpu, Ellipsis, Layers3, Play, Plus, Puzzle, Search, Settings2, Trash2, X } from 'lucide-react';
import { translate } from '../i18n';
import { filterInstances, type InstanceFilter } from '../instanceLibrary';
import { loaderName, type Instance, type Settings } from '../model';

interface Props {
  instances: Instance[];
  selected: string;
  settings: Settings;
  busy: boolean;
  running: boolean;
  onSelect: (id: string) => void;
  onPlay: (id: string) => void;
  onCreate: () => void;
  onEdit: (id: string) => void;
  onMods: (id: string) => void;
  onRemove: (id: string) => void;
  onBrowse: () => void;
  onHorizon: () => void;
}

function WorldScene({ accent = 'purple' }: { accent?: Instance['accent'] }) {
  return <div className="library-scene" data-accent={accent} aria-hidden="true">
    <span className="library-scene-sun"/><span className="library-scene-stars"/>
    <span className="library-terrain library-terrain-back"/><span className="library-terrain library-terrain-front"/>
    <span className="library-scene-grid"/>
  </div>;
}

export function InstancesPage({ instances, selected, settings, busy, running, onSelect, onPlay, onCreate, onEdit, onMods, onRemove, onBrowse, onHorizon }: Props) {
  const t = (text: string) => translate(settings.language, text);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<InstanceFilter>('all');
  const [removing, setRemoving] = useState<Instance | null>(null);
  const root = useRef<HTMLElement>(null);
  const removeDialog = useRef<HTMLDialogElement>(null);
  const removeTrigger = useRef<HTMLElement | null>(null);
  const createButton = useRef<HTMLButtonElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const dialogId = useId();
  const current = instances.find(instance => instance.id === selected) ?? instances[0];
  const locked = busy || running;
  const atLimit = instances.length >= 100;
  const visible = filterInstances(instances, query, filter);
  const counts = { all: instances.length, java: instances.filter(instance => instance.engine !== 'horizon').length, horizon: instances.filter(instance => instance.engine === 'horizon').length };
  const engineName = (instance: Instance) => instance.engine === 'horizon' ? 'Horizon · Rust' : instance.loader ? loaderName(instance.loader.kind) : 'Vanilla';

  useEffect(() => {
    if (removing) removeDialog.current?.showModal(); else removeDialog.current?.close();
  }, [removing]);
  useEffect(() => {
    const closeMenus = (event: PointerEvent | KeyboardEvent) => {
      if (event instanceof KeyboardEvent && event.key !== 'Escape') return;
      root.current?.querySelectorAll<HTMLDetailsElement>('.library-card-menu[open]').forEach(menu => {
        if (event instanceof PointerEvent && menu.contains(event.target as Node)) return;
        menu.open = false;
        if (event instanceof KeyboardEvent) menu.querySelector('summary')?.focus();
      });
    };
    document.addEventListener('pointerdown', closeMenus);
    document.addEventListener('keydown', closeMenus);
    return () => { document.removeEventListener('pointerdown', closeMenus); document.removeEventListener('keydown', closeMenus); };
  }, []);

  return <section ref={root} className="page content-page library-page">
    <header className="library-heading">
      <div><div className="eyebrow">{t('ТВОЯ БИБЛИОТЕКА')}</div><h2>{t('Сборки')}<span>.</span></h2><p>{t('Все твои миры — под рукой.')}</p></div>
      <div className="library-heading-actions">
        <button type="button" className="library-button library-button-secondary" disabled={busy} onClick={onBrowse}><Compass size={17}/>{t('Найти сборку')}</button>
        <button ref={createButton} type="button" className="library-button library-button-primary" disabled={locked || atLimit} title={atLimit ? t('Достигнут лимит: 100 инстансов.') : undefined} onClick={onCreate}><Plus size={18}/>{t('Создать сборку')}</button>
      </div>
    </header>

    <section className="library-featured" aria-labelledby="library-current-name">
      <div className="library-featured-art"><WorldScene accent={current.accent}/></div>
      <div className="library-featured-copy">
        <span className="library-current-label"><span/>{t(running ? 'ИГРА ЗАПУЩЕНА' : 'ВЫБРАНА ДЛЯ ЗАПУСКА')}</span>
        <h3 id="library-current-name">{current.name}</h3>
        <div className="library-featured-meta"><span><Layers3 size={14}/>{engineName(current)}</span><span>{current.version}</span>
          {current.engine !== 'horizon' && <span>{((current.memory ?? settings.memory) / 1024).toLocaleString(settings.language, { maximumFractionDigits: 1 })} {t('ГБ')} RAM</span>}
        </div>
        <div className="library-featured-actions">
          <button type="button" className="library-button library-button-primary library-play" disabled={locked} onClick={() => onPlay(current.id)}><Play size={16} fill="currentColor"/>{t(running ? 'Игра запущена' : busy ? 'Подождите…' : 'Играть')}</button>
          <button type="button" className="library-button library-featured-settings" disabled={locked} onClick={() => onEdit(current.id)}><Settings2 size={16}/>{t('Настроить')}</button>
          {current.engine !== 'horizon' && <button type="button" className="library-button library-featured-settings" disabled={locked} onClick={() => onMods(current.id)}><Puzzle size={16}/>{t('Моды')}</button>}
        </div>
      </div>
    </section>

    <div className="library-toolbar">
      <div className="library-filters" role="group" aria-label={t('Тип сборки')}>
        {([{ id: 'all', label: 'Все сборки' }, { id: 'java', label: 'Java' }, { id: 'horizon', label: 'Horizon' }] as const).map(item => <button type="button" key={item.id} aria-pressed={filter === item.id} onClick={() => setFilter(item.id)}>{t(item.label)}<span>{counts[item.id]}</span></button>)}
      </div>
      <div className="library-search"><Search size={17} aria-hidden="true"/><input ref={searchInput} type="search" aria-label={t('Поиск сборок')} placeholder={t('Название, версия или загрузчик…')} value={query} onChange={event => setQuery(event.target.value)}/>
        {query && <button type="button" aria-label={t('Очистить поиск')} onClick={() => { setQuery(''); searchInput.current?.focus(); }}><X size={15}/></button>}
      </div>
    </div>

    <div className="library-results-heading"><h3>{t('Моя коллекция')}<span>{instances.length}</span></h3><span role="status">{query.trim() || filter !== 'all' ? t('Найдено: {count}').replace('{count}', String(visible.length)) : t('Отдельные версии, моды и настройки')}</span></div>
    {visible.length ? <div className="library-grid">
      {visible.map(instance => <article className={`library-card${instance.id === selected ? ' is-selected' : ''}`} key={instance.id}>
        <div className="library-card-art"><WorldScene accent={instance.accent}/><span className="library-version-badge">{instance.engine === 'horizon' ? 'Horizon' : 'Minecraft'} {instance.version}</span>
          {instance.id === selected && <span className="library-card-check" aria-label={t('Выбрана')}><Check size={14}/></span>}
        </div>
        <div className="library-card-copy"><h4 title={instance.name}>{instance.name}</h4><p><span className={`library-engine-dot${instance.engine === 'horizon' ? ' is-native' : ''}`}/>{engineName(instance)}{instance.loader && <span>{instance.loader.version}</span>}{instance.modpack && <span className="library-modpack-label">Modrinth</span>}</p></div>
        <footer className="library-card-actions">
          <button type="button" className="library-select" aria-pressed={instance.id === selected} disabled={locked} onClick={() => onSelect(instance.id)}>{t(instance.id === selected ? 'Выбрана' : 'Выбрать')}{instance.id === selected ? <Check size={15}/> : <ArrowRight size={15}/>}</button>
          <button type="button" className="library-icon-button" aria-label={t('Настроить сборку {name}').replace('{name}', instance.name)} title={t('Настроить сборку')} disabled={locked} onClick={() => onEdit(instance.id)}><Settings2 size={17}/></button>
          <details className="library-card-menu" onToggle={event => { if (event.currentTarget.open) root.current?.querySelectorAll<HTMLDetailsElement>('.library-card-menu[open]').forEach(menu => { if (menu !== event.currentTarget) menu.open = false; }); }}>
            <summary className="library-icon-button" aria-label={t('Действия со сборкой {name}').replace('{name}', instance.name)} title={t('Другие действия')}><Ellipsis size={18}/></summary>
            <div className="library-menu-actions">
              {instance.engine !== 'horizon' && <button type="button" disabled={locked} onClick={() => onMods(instance.id)}><Puzzle size={16}/>{t('Моды сборки')}</button>}
              <button type="button" className="library-remove-action" disabled={locked || instances.length <= 1} onClick={event => {
                const menu = event.currentTarget.closest('details');
                removeTrigger.current = menu?.querySelector('summary') ?? null;
                if (menu) menu.open = false;
                setRemoving(instance);
              }}><Trash2 size={16}/>{t('Убрать из списка')}</button>
              {instances.length <= 1 && <p>{t('В библиотеке должна остаться хотя бы одна сборка.')}</p>}
            </div>
          </details>
        </footer>
      </article>)}
      {!query.trim() && filter === 'all' && instances.length < 3 && !atLimit && <button type="button" className="library-new-card" disabled={locked} onClick={onCreate}><span><Plus size={25}/></span><strong>{t('Ещё один мир')}</strong><small>{t('Создай сборку под своё настроение.')}</small><span className="library-new-link">{t('Создать сборку')}<ArrowRight size={14}/></span></button>}
    </div> : <div className="library-empty"><span><Search size={26}/></span><h3>{t('Такой сборки пока нет')}</h3><p>{t('Попробуй другое название, версию или тип сборки.')}</p><button type="button" className="library-button library-button-secondary" onClick={() => { setQuery(''); setFilter('all'); searchInput.current?.focus(); }}>{t('Сбросить поиск и фильтр')}</button></div>}

    {!counts.horizon && <section className="library-horizon"><span className="library-horizon-icon"><Cpu size={23}/></span><div><h3>{t('Знакомый Minecraft. Другой движок.')}</h3><p>{t('Попробуй Horizon — нативный клиент на Rust.')}</p></div><button type="button" className="library-button library-button-secondary" disabled={locked || atLimit} onClick={onHorizon}>{t('Добавить Horizon')}<ArrowRight size={15}/></button></section>}

    <dialog ref={removeDialog} className="library-remove-dialog" aria-labelledby={`${dialogId}-title`} aria-describedby={`${dialogId}-description`} onCancel={() => setRemoving(null)}
      onClose={() => { setRemoving(null); if (removeTrigger.current?.isConnected) removeTrigger.current.focus(); else createButton.current?.focus(); }}>
      <span className="library-remove-icon"><Trash2 size={23}/></span><h2 id={`${dialogId}-title`}>{t('Убрать сборку из списка?')}</h2>
      <p className="library-remove-name">{removing?.name}</p><p id={`${dialogId}-description`}>{t('Файлы игры останутся на диске. Сборка исчезнет только из библиотеки лаунчера.')}</p>
      <div className="library-dialog-actions"><button type="button" className="library-button library-button-secondary" autoFocus onClick={() => setRemoving(null)}>{t('Отменить')}</button><button type="button" className="library-button library-button-danger" disabled={locked || instances.length <= 1 || !removing} onClick={() => { if (removing && !locked && instances.length > 1) { onRemove(removing.id); setRemoving(null); } }}>{t('Убрать из списка')}</button></div>
    </dialog>
  </section>;
}
