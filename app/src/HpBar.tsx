import { fmt, fmtTime } from './format'
import type { TargetHpView } from './types'

function barColor(pct: number) {
  if (pct > 0.5) return 'bg-emerald-500'
  if (pct > 0.2) return 'bg-amber-500'
  return 'bg-red-500'
}

/** Target HP bar with name, percent, and time-to-kill. */
export function HpBar({ t, compact }: { t: TargetHpView; compact?: boolean }) {
  return (
    <div>
      <div className={`flex items-baseline justify-between gap-2 ${compact ? 'text-[11px]' : 'text-sm'}`}>
        <span className="truncate font-semibold capitalize">
          {t.name} <span className="font-normal text-zinc-400">({t.rank})</span>
        </span>
        <span className="tabular shrink-0 text-zinc-300">
          {t.dead ? 'Dead' : `${(t.pct * 100).toFixed(1)}%`}
        </span>
      </div>
      <div className={`mt-1 overflow-hidden rounded bg-zinc-800 ${compact ? 'h-2' : 'h-3'}`}>
        <div
          className={`h-full transition-[width] duration-200 ${t.dead ? 'bg-zinc-600' : barColor(t.pct)}`}
          style={{ width: `${t.pct * 100}%` }}
        />
      </div>
      <div className={`tabular mt-1 flex justify-between text-zinc-400 ${compact ? 'text-[10px]' : 'text-xs'}`}>
        <span>
          {fmt(t.hp)} / {fmt(t.maxHp)}
        </span>
        <span>
          {t.dead
            ? `killed in ${fmtTime(t.fightTime)}`
            : t.timeToKill != null
              ? `~${fmtTime(t.timeToKill)} left · ${fmt(t.dps)}/s`
              : `${fmt(t.dps)}/s`}
        </span>
      </div>
    </div>
  )
}
