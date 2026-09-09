import { Check, ChevronDown, Minus } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";

export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: ReactNode;
  description?: ReactNode;
  disabled?: boolean;
}) {
  return (
    <label className={`flex cursor-pointer items-start justify-between gap-4 py-2 ${disabled ? "opacity-50" : ""}`}>
      <span>
        <span className="block text-[13px] font-medium text-fg">{label}</span>
        {description && <span className="block text-[12px] text-fg-muted">{description}</span>}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`relative mt-0.5 h-5 w-9 shrink-0 rounded-full transition-colors ${checked ? "bg-accent" : "bg-border-strong"}`}
      >
        <span className={`absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-transform ${checked ? "translate-x-4.5" : "translate-x-0.5"}`} />
      </button>
    </label>
  );
}

export function Checkbox({
  checked,
  indeterminate,
  onChange,
  disabled,
  title,
}: {
  checked: boolean;
  indeterminate?: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
  title?: string;
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={indeterminate ? "mixed" : checked}
      title={title}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!checked);
      }}
      className={`flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
        checked || indeterminate ? "border-accent bg-accent text-accent-fg" : "border-border-strong bg-surface hover:border-accent"
      }`}
    >
      {indeterminate ? <Minus size={11} strokeWidth={3} /> : checked ? <Check size={11} strokeWidth={3} /> : null}
    </button>
  );
}

export function Select<T extends string>({
  value,
  onChange,
  options,
  className = "",
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
  className?: string;
}) {
  return (
    <div className={`relative ${className}`}>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as T)}
        className="h-8 w-full appearance-none rounded-md border border-border bg-surface pl-2.5 pr-7 text-[13px] text-fg hover:bg-surface-2"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <ChevronDown size={14} className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-fg-subtle" />
    </div>
  );
}

export function TextInput({
  value,
  onChange,
  placeholder,
  className = "",
  mono,
  type = "text",
  onKeyDown,
  id,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  className?: string;
  mono?: boolean;
  type?: string;
  onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  id?: string;
}) {
  return (
    <input
      id={id}
      type={type}
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={onKeyDown}
      className={`h-8 w-full rounded-md border border-border bg-surface px-2.5 text-[13px] text-fg placeholder:text-fg-subtle focus:border-accent ${mono ? "font-mono text-[12px]" : ""} ${className}`}
    />
  );
}

export function NumberInput({
  value,
  onChange,
  min,
  max,
  className = "",
  suffix,
}: {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  className?: string;
  suffix?: string;
}) {
  return (
    <div className={`flex items-center gap-2 ${className}`}>
      <input
        type="number"
        value={value}
        min={min}
        max={max}
        onChange={(e) => {
          const n = Number(e.target.value);
          if (!Number.isNaN(n)) onChange(n);
        }}
        className="h-8 w-20 rounded-md border border-border bg-surface px-2.5 text-[13px] tabular text-fg focus:border-accent"
      />
      {suffix && <span className="text-[12px] text-fg-muted">{suffix}</span>}
    </div>
  );
}

export function RadioGroup<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: ReactNode; description?: ReactNode; tone?: "danger" }[];
}) {
  return (
    <div className="flex flex-col gap-1.5">
      {options.map((o) => (
        <label
          key={o.value}
          className={`flex cursor-pointer items-start gap-3 rounded-md border px-3 py-2 transition-colors ${value === o.value ? "border-accent bg-accent-soft/50" : "border-border hover:bg-surface-2"}`}
        >
          <input type="radio" className="mt-1 accent-[var(--accent)]" checked={value === o.value} onChange={() => onChange(o.value)} />
          <span>
            <span className={`block text-[13px] font-medium ${o.tone === "danger" ? "text-danger" : "text-fg"}`}>{o.label}</span>
            {o.description && <span className="block text-[12px] text-fg-muted">{o.description}</span>}
          </span>
        </label>
      ))}
    </div>
  );
}

export interface MenuItem {
  label: ReactNode;
  icon?: ReactNode;
  onSelect?: () => void;
  disabled?: boolean;
  danger?: boolean;
  separator?: boolean;
  hint?: string;
}

export function Menu({
  trigger,
  items,
  align = "left",
  className = "",
}: {
  trigger: (open: boolean) => ReactNode;
  items: MenuItem[];
  align?: "left" | "right";
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const itemsEls = () => Array.from(listRef.current?.querySelectorAll<HTMLButtonElement>("[role=menuitem]:not([disabled])") ?? []);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        setOpen(false);
        return;
      }
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "Home" && e.key !== "End") return;
      const els = itemsEls();
      if (!els.length) return;
      e.preventDefault();
      const idx = els.indexOf(document.activeElement as HTMLButtonElement);
      const next =
        e.key === "Home" ? 0 : e.key === "End" ? els.length - 1 : e.key === "ArrowDown" ? (idx + 1) % els.length : (idx - 1 + els.length) % els.length;
      els[next]?.focus();
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    itemsEls()[0]?.focus();
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);
  return (
    <div ref={ref} className={`relative ${className}`}>
      <div
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" && !open) {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        {trigger(open)}
      </div>
      {open && (
        <div
          ref={listRef}
          role="menu"
          className={`fade-in absolute z-40 mt-1 min-w-52 rounded-lg border border-border bg-surface p-1 shadow-panel ${align === "right" ? "right-0" : "left-0"}`}
        >
          {items.map((item, i) =>
            item.separator ? (
              <div key={i} aria-hidden="true" className="my-1 border-t border-border" />
            ) : (
              <button
                key={i}
                type="button"
                role="menuitem"
                tabIndex={-1}
                disabled={item.disabled}
                onClick={(e) => {
                  e.stopPropagation();
                  setOpen(false);
                  item.onSelect?.();
                }}
                className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[13px] disabled:opacity-40 ${item.danger ? "text-danger hover:bg-danger-soft" : "text-fg hover:bg-surface-2"}`}
              >
                {item.icon && <span className="text-fg-muted">{item.icon}</span>}
                <span className="flex-1">{item.label}</span>
                {item.hint && <span className="text-[11px] text-fg-subtle">{item.hint}</span>}
              </button>
            ),
          )}
        </div>
      )}
    </div>
  );
}

export function ProgressBar({ value, tone = "accent", className = "" }: { value: number | null; tone?: "accent" | "safe"; className?: string }) {
  const color = tone === "safe" ? "bg-safe" : "bg-accent";
  return (
    <div className={`relative h-1.5 w-full overflow-hidden rounded-full bg-surface-3 ${value === null ? "indeterminate" : ""} ${className}`}>
      {value !== null && (
        <div className={`h-full rounded-full ${color} transition-[width] duration-300`} style={{ width: `${Math.max(0, Math.min(1, value)) * 100}%` }} />
      )}
    </div>
  );
}

export function EmptyState({ icon, title, description, action }: { icon: ReactNode; title: ReactNode; description?: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center px-6 py-16 text-center">
      <div className="mb-3 flex h-12 w-12 items-center justify-center rounded-xl bg-surface-2 text-fg-muted">{icon}</div>
      <h3 className="text-[14px] font-semibold text-fg">{title}</h3>
      {description && <p className="mt-1 max-w-md text-[12.5px] text-fg-muted">{description}</p>}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="rounded border border-border bg-surface-2 px-1.5 py-0.5 font-mono text-[10px] text-fg-muted">{children}</kbd>;
}
