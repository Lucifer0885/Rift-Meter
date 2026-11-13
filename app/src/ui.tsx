import type { ReactNode } from 'react'

export function Card({ title, children, right }: { title?: string; children: ReactNode; right?: ReactNode }) {
  return (
    <section className="rounded-lg border border-white/5 bg-white/[0.03] p-4">
      {(title || right) && (
        <div className="mb-3 flex items-center justify-between">
          {title && <h2 className="text-xs font-semibold tracking-wide text-zinc-400 uppercase">{title}</h2>}
          {right}
        </div>
      )}
      {children}
    </section>
  )
}

export function Stat({ label, value, sub, accent }: { label: string; value: string; sub?: string; accent?: string }) {
  return (
    <div className="rounded-lg border border-white/5 bg-white/3 px-4 py-3">
      <div className="text-xs text-zinc-400">{label}</div>
      <div className={`tabular mt-1 text-2xl font-semibold ${accent ?? 'text-zinc-100'}`}>{value}</div>
      {sub && <div className="tabular mt-0.5 text-xs text-zinc-500">{sub}</div>}
    </div>
  )
}

export function Toggle({
  checked,
  onChange,
  label,
  hint,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  label: string
  hint?: string
}) {
  return (
    <label className="flex cursor-pointer items-start gap-3 py-1.5">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative mt-0.5 h-5 w-9 shrink-0 rounded-full transition-colors ${
          checked ? 'bg-me' : 'bg-zinc-700'
        }`}
      >
        <span
          className={`absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all ${checked ? 'left-4.5' : 'left-0.5'}`}
        />
      </button>
      <span>
        <span className="text-sm text-zinc-200">{label}</span>
        {hint && <span className="block text-xs text-zinc-500">{hint}</span>}
      </span>
    </label>
  )
}

export function Slider({
  label,
  value,
  min,
  max,
  step,
  onChange,
  display,
}: {
  label: string
  value: number
  min: number
  max: number
  step: number
  onChange: (v: number) => void
  display: string
}) {
  return (
    <label className="block py-1.5">
      <div className="flex justify-between text-sm">
        <span className="text-zinc-200">{label}</span>
        <span className="tabular text-zinc-400">{display}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="mt-1 w-full accent-me"
      />
    </label>
  )
}

export function Button({
  children,
  onClick,
  variant = 'default',
  disabled,
}: {
  children: ReactNode
  onClick: () => void
  variant?: 'default' | 'primary' | 'danger'
  disabled?: boolean
}) {
  const styles = {
    default: 'bg-white/5 hover:bg-white/10 text-zinc-200',
    primary: 'bg-me/90 hover:bg-me text-white',
    danger: 'bg-red-500/80 hover:bg-red-500 text-white',
  }[variant]
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={`rounded-md px-3 py-1.5 text-sm font-medium transition-colors disabled:opacity-40 ${styles}`}
    >
      {children}
    </button>
  )
}
