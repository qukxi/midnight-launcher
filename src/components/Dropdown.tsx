import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown } from 'lucide-react';
export interface DropdownOption { value: string; label: string; disabled?: boolean }
interface Props { id?: string; label: string; value: string; options: DropdownOption[]; onChange: (value: string) => void; editable?: boolean; required?: boolean; placeholder?: string; emptyLabel?: string }
export function Dropdown({ id, label, value, options, onChange, editable = false, required, placeholder, emptyLabel = '—' }: Props) {
  const generatedId = useId(); const controlId = id ?? generatedId;
  const listId = `${controlId}-list`;
  const root = useRef<HTMLDivElement>(null); const menu = useRef<HTMLDivElement>(null);
  const control = useRef<HTMLInputElement & HTMLButtonElement>(null);
  const [open, setOpen] = useState(false); const [active, setActive] = useState(-1);
  const [filter, setFilter] = useState(false); const [style, setStyle] = useState<CSSProperties>({position: 'fixed', visibility: 'hidden'});
  const visible = editable && filter ? options.filter(o => o.label.toLowerCase().includes(value.toLowerCase())) : options;
  const selected = options.find(o => o.value === value);
  const choose = (index: number) => { const option = visible[index]; if (!option || option.disabled) return; onChange(option.value); setOpen(false); setFilter(false); control.current?.focus(); };
  function show() { setFilter(false); setActive(options.findIndex(o => o.value === value && !o.disabled)); setOpen(true); }
  useLayoutEffect(() => {
    if (!open) return;
    const position = () => {
      const rect = root.current!.getBoundingClientRect(); const tokens = getComputedStyle(root.current!);
      const below = window.innerHeight - rect.bottom - 12; const above = rect.top - 12;
      const height = Math.min(280, Math.max(below, above)); const down = below >= 180 || below >= above;
      const width = Math.min(Math.max(rect.width, 220), window.innerWidth - 24);
      const next: Record<string, string | number> = { position: 'fixed', left: Math.max(12, Math.min(rect.left, window.innerWidth - width - 12)), width, maxHeight: Math.max(60, height) };
      if (down) next.top = rect.bottom + 6; else next.bottom = window.innerHeight - rect.top + 6;
      for (const token of ['--surface-deep', '--surface', '--ink', '--rose', '--muted', '--line']) next[token] = tokens.getPropertyValue(token);
      setStyle(next as CSSProperties);
    };
    position(); const observer = new ResizeObserver(position); observer.observe(root.current!); const modal = root.current?.closest('dialog'); if (modal) observer.observe(modal); window.addEventListener('resize', position); window.addEventListener('scroll', position, true);
    const outside = (e: PointerEvent) => { if (!root.current?.contains(e.target as Node) && !menu.current?.contains(e.target as Node)) setOpen(false); };
    document.addEventListener('pointerdown', outside);
    return () => { observer.disconnect(); window.removeEventListener('resize', position); window.removeEventListener('scroll', position, true); document.removeEventListener('pointerdown', outside); };
  }, [open]);
  useEffect(() => { if (open && active >= 0) menu.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' }); }, [active, open]);
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') { if (open) { e.preventDefault(); e.stopPropagation(); setOpen(false); } return; }
    if (e.key === 'Tab') { setOpen(false); return; }
    if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key) && (!editable || !['Home', 'End'].includes(e.key))) {
      e.preventDefault(); if (!open) { show(); return; }
      const indices = visible.flatMap((o, i) => o.disabled ? [] : [i]);
      const index = indices.indexOf(active);
      setActive(e.key === 'Home' ? indices[0] ?? -1 : e.key === 'End' ? indices.at(-1) ?? -1 : indices[index < 0 ? (e.key === 'ArrowUp' ? indices.length - 1 : 0) : (index + (e.key === 'ArrowUp' ? -1 : 1) + indices.length) % indices.length] ?? -1);
    } else if (e.key === 'Enter' || (!editable && e.key === ' ')) {
      if (open) { e.preventDefault(); choose(active); } else if (!editable) { e.preventDefault(); show(); }
    } else if (!editable && e.key.length === 1) {
      e.preventDefault(); if (!open) show();
      setActive(visible.findIndex(o => !o.disabled && o.label.toLowerCase().startsWith(e.key.toLowerCase())));
    }
  }
  const aria = { id: controlId, role: 'combobox', 'aria-label': label, 'aria-expanded': open, 'aria-controls': open ? listId : undefined, 'aria-activedescendant': open && active >= 0 && visible[active] ? `${listId}-${active}` : undefined, 'aria-haspopup': 'listbox' as const };
  return <div className={`midnight-dropdown ${editable ? 'is-editable' : ''}`} ref={root}>
    {editable ? <input {...aria} ref={control} value={value} required={required} placeholder={placeholder} autoComplete="off" aria-autocomplete="list" onFocus={show} onClick={() => { if (!open) show(); }} onBlur={e => { if (!menu.current?.contains(e.relatedTarget)) setOpen(false); }} onChange={e => { onChange(e.target.value); setFilter(true); setActive(-1); setOpen(true); }} onKeyDown={key}/> : <button {...aria} type="button" ref={control} className="dropdown-trigger" onClick={() => open ? setOpen(false) : show()} onKeyDown={key} onBlur={() => setOpen(false)}><span>{selected?.label ?? value}</span></button>}
    <ChevronDown size={16} className={`dropdown-chevron ${open ? 'is-open' : ''}`}/>
    {open && createPortal(<div ref={menu} id={listId} role="listbox" aria-label={label} className="midnight-dropdown-menu" style={style} onMouseDown={e => e.preventDefault()}>
      {visible.length ? visible.map((option, i) => <div key={option.value} id={`${listId}-${i}`} data-index={i} role="option" aria-selected={option.value === value} aria-disabled={option.disabled || undefined} className={`midnight-dropdown-option ${i === active ? 'is-active' : ''}`} onPointerMove={() => { if (!option.disabled) setActive(i); }} onClick={() => choose(i)}><span>{option.label}</span>{option.value === value && <Check size={15}/>}</div>) : <div className="midnight-dropdown-empty">{emptyLabel}</div>}
    </div>, root.current?.closest('dialog') ?? document.body)}
  </div>;
}
