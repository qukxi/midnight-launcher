import { useEffect, useId, useRef, useState, type CSSProperties } from 'react';
import { AppWindow, ArrowUpRight, Check, ChevronRight, Cpu, Info, Languages, Layers3, Minimize2, Monitor, Moon, Palette, Play, RotateCcw, Sparkles, Terminal, Wind, type LucideIcon } from 'lucide-react';
import { translate } from '../i18n';
import { themes } from '../themes';
import type { Instance, Settings } from '../model';

interface Props {
  settings: Settings;
  current: Instance;
  storageError: boolean;
  onChange: (patch: Partial<Settings>) => void;
  onEditInstance: () => void;
  onOpenLogs: () => void;
}

const sections = [
  { id: 'appearance', label: 'Оформление', detail: 'Цвета и атмосфера', icon: Palette },
  { id: 'game', label: 'Игра и Java', detail: 'Всё для запуска', icon: Cpu },
  { id: 'launcher', label: 'Лаунчер', detail: 'Привычные мелочи', icon: AppWindow },
] as const;
type Section = typeof sections[number]['id'];

function ToggleRow({ icon: Icon, label, description, checked, disabled, onChange }: {
  icon: LucideIcon; label: string; description: string; checked: boolean;
  disabled?: boolean; onChange: () => void;
}) {
  const descriptionId = useId();
  return <div className={`settings-toggle-row${disabled ? ' is-disabled' : ''}`}>
    <span className="settings-row-icon"><Icon size={18} aria-hidden="true"/></span>
    <div className="settings-row-copy"><h4>{label}</h4><p id={descriptionId}>{description}</p></div>
    <button type="button" role="switch" aria-label={label} aria-describedby={descriptionId} aria-checked={checked} disabled={disabled}
      className={`settings-switch${checked ? ' is-on' : ''}`} onClick={onChange}><span>{checked && <Check size={10} aria-hidden="true"/>}</span></button>
  </div>;
}

export function SettingsPage({ settings, current, storageError, onChange, onEditInstance, onOpenLogs }: Props) {
  const [section, setSection] = useState<Section>('appearance');
  const root = useRef<HTMLElement>(null);
  const t = (text: string) => translate(settings.language, text);
  const activeTheme = themes.find(theme => theme.id === settings.theme) ?? themes[0];
  const memory = (settings.memory / 1024).toLocaleString(settings.language === 'ru' ? 'ru-RU' : 'en-US', { maximumFractionDigits: 1 });
  useEffect(() => { root.current?.closest('main')?.scrollTo({ top: 0 }); }, [section]);

  return <section ref={root} className="page content-page settings-page">
    <div className="settings-page-heading">
      <div><div className="eyebrow">{t('ТВОЙ MIDNIGHT')}</div><h2>{t('Настройки')}<span>.</span></h2>
        <p>{t('Устройся поудобнее. Здесь всё по-твоему.')}</p></div>
      <span className={`settings-save-state${storageError ? ' has-error' : ''}`} role="status">
        {storageError ? <Info size={14} aria-hidden="true"/> : <Check size={14} aria-hidden="true"/>}
        {t(storageError ? 'Не сохранено' : 'Всё сохранено')}
      </span>
    </div>

    <div className="settings-workspace">
      <aside className="settings-sidebar">
        <div className="settings-nav-caption">{t('ПРЕДПОЧТЕНИЯ')}</div>
        <nav className="settings-sections" aria-label={t('Разделы настроек')}>
          {sections.map(item => <button type="button" key={item.id} aria-current={section === item.id ? 'page' : undefined}
            aria-controls="settings-section-content" onClick={() => setSection(item.id)}>
            <span className="settings-nav-icon"><item.icon size={18} aria-hidden="true"/></span>
            <span className="settings-nav-copy"><strong>{t(item.label)}</strong><small>{t(item.detail)}</small></span>
            <ChevronRight size={14} aria-hidden="true"/>
          </button>)}
        </nav>
        <div className="settings-sidebar-note"><span className="settings-note-icon"><Check size={13} aria-hidden="true"/></span>
          <p>{t('Без лишних кнопок.')}<br/><span>{t('Изменения сохраняются автоматически.')}</span></p>
        </div>
      </aside>

      <div className="settings-content" id="settings-section-content">
        {section === 'appearance' && <>
          <section className="settings-card settings-palette-card" aria-labelledby="settings-palette-title">
            <div className="settings-card-heading">
              <div><h3 id="settings-palette-title">{t('Тема оформления')}</h3><p>{t('Твой лаунчер. Твоя атмосфера.')}</p></div>
              <span className="settings-count"><Palette size={12} aria-hidden="true"/>{themes.length} {t('тем')}</span>
            </div>

            <div className="settings-theme-showcase">
              <div className="settings-showcase-copy">
                <span className="settings-showcase-label"><span/>{t('СЕЙЧАС ВЫБРАНА')}</span>
                <h4>{t(activeTheme.name)}</h4>
                <p>{t(activeTheme.description)}</p>
                <span className="settings-showcase-colors" aria-hidden="true"><i/><i/><i/><i/></span>
              </div>
              <div className="settings-launcher-preview" role="img" aria-label={t('Предпросмотр выбранной темы')}>
                <div className="settings-mini-topbar" aria-hidden="true"><Moon size={12}/><span>MIDNIGHT</span><i/><i/><i/></div>
                <div className="settings-mini-world" aria-hidden="true"><span/><span/><span/><i/><i/><i/></div>
                <span className="settings-mini-title" aria-hidden="true">MIDNIGHT</span>
                <span className="settings-mini-play" aria-hidden="true"><Play size={10} fill="currentColor"/>{t('Играть')}</span>
                <div className="settings-mini-dock" aria-hidden="true"><span/><span/><span/><span/></div>
              </div>
            </div>

            <div className="settings-themes" role="group" aria-labelledby="settings-palette-title">
              {themes.map(theme => <button type="button" key={theme.id} className={`settings-theme${theme.id === settings.theme ? ' is-selected' : ''}`}
                aria-label={t(theme.name)} aria-pressed={theme.id === settings.theme} onClick={() => onChange({ theme: theme.id })}>
                <span className="settings-theme-preview" data-preview-theme={theme.id} aria-hidden="true"><i/><i/><i/></span>
                <span className="settings-theme-name">{t(theme.name)}</span>
                <span className="settings-theme-check" aria-hidden="true">{theme.id === settings.theme && <Check size={11}/>}</span>
              </button>)}
            </div>
          </section>

          <section className="settings-card settings-comfort-card" aria-labelledby="settings-comfort-title">
            <div className="settings-card-heading"><h3 id="settings-comfort-title">{t('Комфорт в деталях')}</h3><Sparkles size={17} aria-hidden="true"/></div>
            <ToggleRow icon={Layers3} label={t('Градиент фона')} description={t('Мягкие переливы цветов выбранной темы.')}
              checked={settings.gradient} onChange={() => onChange({ gradient: !settings.gradient })}/>
            <ToggleRow icon={Wind} label={t('Меньше движения')} description={t('Спокойный интерфейс без декоративной анимации.')}
              checked={settings.reduceMotion} onChange={() => onChange({ reduceMotion: !settings.reduceMotion })}/>
            <div className="settings-language-row">
              <span className="settings-row-icon"><Languages size={18} aria-hidden="true"/></span>
              <div className="settings-row-copy"><h4 id="settings-language-title">{t('Язык интерфейса')}</h4><p>{t('Перевод применяется сразу.')}</p></div>
              <div className="settings-language-options" role="group" aria-labelledby="settings-language-title">
                {([{ value: 'ru', label: 'Русский' }, { value: 'en', label: 'English' }] as const).map(language => <button type="button" key={language.value}
                  lang={language.value} aria-pressed={settings.language === language.value} onClick={() => onChange({ language: language.value })}>{language.label}</button>)}
              </div>
            </div>
          </section>
        </>}

        {section === 'game' && <>
          <div className="settings-section-intro"><span className="settings-section-emblem"><Cpu size={22} aria-hidden="true"/></span><div><h3>{t('Место для больших миров')}</h3><p>{t('Параметры по умолчанию для твоих Java-сборок.')}</p></div></div>
          <section className="settings-card settings-ram-card" aria-labelledby="settings-memory-title">
            <div className="settings-card-heading"><div><h3 id="settings-memory-title">{t('Оперативная память')}</h3><p>{t('Максимальный объём памяти для Minecraft Java.')}</p></div><span className="settings-memory-value">{memory}<small>{t('ГБ')}</small></span></div>
            <div className="settings-memory-presets" role="group" aria-label={t('Быстрый выбор памяти')}>
              {[2048, 4096, 6144, 8192].map(value => <button type="button" key={value} aria-pressed={settings.memory === value} onClick={() => onChange({ memory: value })}>
                <strong>{value / 1024}<small>{t('ГБ')}</small></strong><span className="settings-preset-dot" aria-hidden="true"/>
              </button>)}
            </div>
            <div className="settings-memory-control">
              <input className="settings-memory-slider" aria-labelledby="settings-memory-title" aria-describedby="settings-memory-note" aria-valuetext={`${memory} ${t('ГБ')}`} type="range" min="1024" max="16384" step="512"
                style={{ '--memory-progress': `${(settings.memory - 1024) / 15360 * 100}%` } as CSSProperties}
                value={settings.memory} onChange={event => onChange({ memory: Number(event.target.value) })}/>
              <div className="settings-memory-scale" aria-hidden="true"><span>1 {t('ГБ')}</span><span>{t('Шаг — 512 МБ')}</span><span>16 {t('ГБ')}</span></div>
            </div>
            <p className="settings-card-footnote" id="settings-memory-note"><Info size={14} aria-hidden="true"/>{t('Лимит памяти Java не применяется к Horizon.')}</p>
          </section>
          <section className="settings-card settings-runtime-card" aria-labelledby="settings-java-title">
            <div className="settings-card-heading"><div><h3 id="settings-java-title">{t('Среда Java')}</h3><p>{t('Нужная версия для каждого мира.')}</p></div><span className="settings-auto-badge"><span/>{t(settings.javaPath.trim() ? 'Свой путь' : 'Автоматически')}</span></div>
            <div className="settings-java-input"><label className="settings-field-label" htmlFor="settings-java-path">{t('Исполняемый файл Java')}</label><div>
              <Terminal size={16} aria-hidden="true"/><input id="settings-java-path" value={settings.javaPath} onChange={event => onChange({ javaPath: event.target.value })}
                placeholder={t('Автоматически (Java по версии Minecraft)')} aria-describedby="settings-java-note" spellCheck={false} autoComplete="off"/>
              {settings.javaPath && <button type="button" className="icon-button" onClick={() => onChange({ javaPath: '' })} aria-label={t('Использовать автоматическую Java')} title={t('Использовать автоматическую Java')}><RotateCcw size={15}/></button>}
            </div></div>
            <p id="settings-java-note">{t('Оставь поле пустым — Midnight установит нужную версию.')}</p>
            <p className="settings-card-footnote"><Info size={14} aria-hidden="true"/>{t('Свой путь проверяется по версии и архитектуре. Для Horizon Java нужна только при подготовке ресурсов.')}</p>
          </section>
          <div className="settings-instance-note"><Layers3 size={18} aria-hidden="true"/><div><span>{t('Каждой сборке — свои настройки')}</span><p>{t('У отдельных сборок могут быть свои параметры.')}</p></div>
            <button type="button" onClick={onEditInstance}>{t('Настроить сборку')}<ArrowUpRight size={14} aria-hidden="true"/></button></div>
        </>}

        {section === 'launcher' && <>
          <div className="settings-section-intro"><span className="settings-section-emblem"><AppWindow size={22} aria-hidden="true"/></span><div><h3>{t('В твоём ритме')}</h3><p>{t('Маленькие привычки для удобной игры.')}</p></div></div>
          <section className="settings-card" aria-labelledby="settings-window-title">
            <div className="settings-card-heading"><div><h3 id="settings-window-title">{t('После запуска игры')}</h3><p>{t('Выбери, как будет вести себя окно Midnight.')}</p></div></div>
            <div className="settings-window-options" role="group" aria-labelledby="settings-window-title">
              {[{ minimize: false, label: 'Оставлять открытым', description: 'Лаунчер всегда под рукой.', icon: Monitor }, { minimize: true, label: 'Сворачивать лаунчер', description: 'Всё внимание — игре.', icon: Minimize2 }].map(option => <button type="button" key={String(option.minimize)}
                aria-pressed={(settings.minimizeOnLaunch ?? false) === option.minimize} onClick={() => onChange({ minimizeOnLaunch: option.minimize })}>
                <span className="settings-window-illustration" aria-hidden="true"><span><option.icon size={24}/><i/><i/></span></span>
                <strong>{t(option.label)}</strong><span className="settings-window-description">{t(option.description)}</span>
                <span className="settings-option-check" aria-hidden="true">{(settings.minimizeOnLaunch ?? false) === option.minimize && <Check size={11}/>}</span>
              </button>)}
            </div>
            <div className="settings-card-rows"><ToggleRow icon={AppWindow} label={t('Возвращать окно после выхода из игры')}
              description={t(settings.minimizeOnLaunch ? 'Midnight снова появится после закрытия Minecraft.' : 'Доступно при сворачивании лаунчера.')}
              checked={settings.restoreOnExit ?? true} disabled={!settings.minimizeOnLaunch} onChange={() => onChange({ restoreOnExit: !(settings.restoreOnExit ?? true) })}/></div>
          </section>
          <section className="settings-card settings-console-card" aria-labelledby="settings-console-title">
            <div className="settings-console-heading"><span className="settings-row-icon"><Terminal size={19} aria-hidden="true"/></span><div><h3 id="settings-console-title">{t('Консоль игры')}</h3><p>{t('Вывод запуска и ошибки Minecraft')}</p></div></div>
            <button type="button" className="settings-console-open" onClick={onOpenLogs}>
              <span className="settings-console-prompt" aria-hidden="true">&gt;_</span><span><strong>{current.name}</strong><small>{current.engine === 'horizon' ? 'Horizon · Rust' : 'Minecraft Java'} · {current.version}</small></span><span>{t('Открыть консоль')}<ArrowUpRight size={15} aria-hidden="true"/></span>
            </button>
          </section>
        </>}
        <div className="settings-content-footer"><Moon size={12} aria-hidden="true"/><span>{t('Настроено под тебя. Готово к приключениям.')}</span></div>
      </div>
    </div>
  </section>;
}
