import { classColor, elementColor, fmt, fmtFull, pct } from './format'
import { classIcon, skillIcon } from './skillIcons'
import type { AttackerView, ElementView, EncounterView, SourceView } from './types'

/** Number columns after the name, shared by the header and every row so they line up. */
const COLS = 'grid grid-cols-[minmax(10rem,1fr)_5.5rem_5rem_3.5rem_3.5rem_3.5rem_5rem] items-center gap-x-3'

/** Damage per character, one block each: totals, an element split, then a row per skill. */
export function AttackerTable({ enc, classColors }: { enc: EncounterView; classColors?: Record<string, string> }) {
  const top = enc.attackers[0]?.damage || 1
  return (
    <div className="tabular text-sm">
      <div className={`${COLS} px-3 pb-2 text-[11px] font-medium tracking-wide text-zinc-500 uppercase`}>
        <span>Source</span>
        <span className="text-right">Damage</span>
        <span className="text-right">DPS</span>
        <span className="text-right">Share</span>
        <span className="text-right">Crit</span>
        <span className="text-right">Hits</span>
        <span className="text-right">Max hit</span>
      </div>
      <div className="space-y-3">
        {enc.attackers.map((a) => (
          <AttackerBlock key={a.id} a={a} top={top} single={enc.attackers.length === 1} classColors={classColors} />
        ))}
      </div>
    </div>
  )
}

function AttackerBlock({
  a,
  top,
  single,
  classColors,
}: {
  a: AttackerView
  top: number
  single: boolean
  classColors?: Record<string, string>
}) {
  const dealt = a.sources.filter((s) => s.damage > 0)
  const idle = a.sources.filter((s) => s.damage === 0 && s.casts > 0)
  const topSource = dealt[0]?.damage || 1
  const uses = a.sources.reduce((n, s) => n + s.casts, 0)
  return (
    <section className="overflow-hidden rounded-lg border border-white/5 bg-white/2">
      {/* Character row: its share of the party fills the background. */}
      <div className={`${COLS} relative px-3 py-2.5`}>
        {!single && (
          <div
            className={`absolute inset-y-0 left-0`}
            style={{ width: `${(a.damage / top) * 100}%` }}
          />
        )}
        {/* Name gets the whole first line; class and casts go underneath so it isn't clipped. */}
        <div className="relative min-w-0">
          <div className={`truncate font-semibold ${a.isMe ? 'text-me' : 'text-zinc-100'}`} title={a.name}>
            {a.name}
          </div>
          {(a.class || uses > 0) && (
            <div className="mt-1 flex items-center gap-2">
              {a.class && <ClassBadge cls={a.class} level={a.level} color={classColor(a.class, classColors)} />}
              {uses > 0 && <span className="shrink-0 text-xs text-zinc-500">{uses} casts</span>}
            </div>
          )}
        </div>
        <span className="relative text-right font-semibold text-zinc-50" title={fmtFull(a.damage)}>
          {fmt(a.damage)}
        </span>
        <span className="relative text-right font-semibold text-zinc-200">{fmt(a.dps)}</span>
        <span className="relative text-right text-zinc-400">{pct(a.share)}</span>
        <span className="relative text-right text-crit">{pct(a.critRate)}</span>
        <span className="relative text-right text-zinc-300">{a.hits}</span>
        <span className="relative text-right text-zinc-300" title={fmtFull(a.maxHit)}>
          {fmt(a.maxHit)}
        </span>
      </div>

      {a.elements.length > 0 && (
        <div className="border-t border-white/5 px-3 py-2">
          <ElementBar els={a.elements} total={a.damage} className="h-1.5" />
          <ElementLegend els={a.elements} total={a.damage} />
        </div>
      )}

      {a.sources.length > 1 && dealt.length > 0 && (
        <div className="border-t border-white/5 py-1">
          {dealt.map((s) => (
            <SourceRow key={s.name} s={s} topSource={topSource} />
          ))}
        </div>
      )}

      {idle.length > 0 && (
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 border-t border-white/5 px-3 py-2 text-xs text-zinc-500">
          <span>No damage:</span>
          {idle.map((s) => (
            <span key={s.name} className="flex items-center gap-1.5 text-zinc-400" title={`${s.name}: used ${s.casts}×`}>
              <SkillIcon name={s.name} size="h-4 w-4" />
              {s.name}
              <span className="text-zinc-600">×{s.casts}</span>
            </span>
          ))}
        </div>
      )}
    </section>
  )
}

function SourceRow({ s, topSource }: { s: SourceView; topSource: number }) {
  return (
    <div className={`${COLS} group px-3 py-1.5 hover:bg-white/3`}>
      <div className="flex min-w-0 items-center gap-2.5">
        <SkillIcon name={s.name} size="h-7 w-7" />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-1.5">
            <span className="truncate text-zinc-200" title={s.name}>
              {s.name}
            </span>
            {s.casts > 0 && <span className="shrink-0 text-xs text-zinc-500">×{s.casts}</span>}
          </div>
          {/* Bar length = damage vs the top skill; colors = its element split (hover for numbers). */}
          <div className="mt-1 h-1 w-full rounded-full bg-white/5">
            <ElementBar
              els={s.elements}
              total={s.damage}
              className="h-1"
              style={{ width: `${Math.max((s.damage / topSource) * 100, 1.5)}%` }}
            />
          </div>
        </div>
      </div>
      <span className="text-right text-zinc-100" title={fmtFull(s.damage)}>
        {fmt(s.damage)}
      </span>
      <span className="text-right text-zinc-400">{fmt(s.dps)}</span>
      <span className="text-right text-zinc-400">{pct(s.share)}</span>
      <span className="text-right text-crit/80">{pct(s.critRate)}</span>
      <span className="text-right text-zinc-400">{s.hits}</span>
      <span className="text-right text-zinc-400" title={fmtFull(s.maxHit)}>
        {fmt(s.maxHit)}
      </span>
    </div>
  )
}

function SkillIcon({ name, size }: { name: string; size: string }) {
  const icon = skillIcon(name)
  return icon ? (
    <img src={icon} alt="" className={`${size} shrink-0 rounded`} draggable={false} />
  ) : (
    <span className={`${size} shrink-0 rounded bg-white/5`} />
  )
}

/** Stacked bar of element shares. Falls back to a neutral bar when no element is known. */
function ElementBar({
  els,
  total,
  className,
  style,
}: {
  els: ElementView[]
  total: number
  className: string
  style?: React.CSSProperties
}) {
  const sum = els.reduce((n, e) => n + e.damage, 0) || total || 1
  const title = els.map((e) => `${e.name} ${fmt(e.damage)} (${pct(e.damage / sum)})`).join('\n')
  return (
    <div className={`flex overflow-hidden rounded-full ${className}`} style={style} title={title || undefined}>
      {els.length === 0 ? (
        <div className="h-full w-full bg-zinc-500/60" />
      ) : (
        els.map((e) => (
          <div
            key={e.name}
            className="h-full"
            style={{ width: `${(e.damage / sum) * 100}%`, background: elementColor(e.name), opacity: 0.85 }}
          />
        ))
      )}
    </div>
  )
}

function ElementLegend({ els, total }: { els: ElementView[]; total: number }) {
  const sum = els.reduce((n, e) => n + e.damage, 0) || total || 1
  return (
    <div className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1 text-xs">
      {els.map((e) => (
        <span key={e.name} className="flex items-center gap-1.5">
          <span className="h-2 w-2 rounded-full" style={{ background: elementColor(e.name) }} />
          <span className="text-zinc-300">{e.name}</span>
          <span className="text-zinc-500">
            {fmt(e.damage)} · {pct(e.damage / sum)}
          </span>
        </span>
      ))}
    </div>
  )
}

/** "Ranger · 100" in the class's color. */
export function ClassBadge({ cls, level, color: c }: { cls: string; level?: number | null; color?: string }) {
  const color = c ?? classColor(cls) ?? '#a1a1aa'
  return (
    <span
      className="flex shrink-0 items-center gap-1 self-center rounded px-1.5 py-px text-[11px] font-medium"
      style={{ color, background: `${color}1f`, border: `1px solid ${color}40` }}
    >
      {classIcon(cls) && <img src={classIcon(cls)} alt="" className="h-3.5 w-3.5 rounded-sm" draggable={false} />}
      {cls}
      {level ? <span className="opacity-70"> · {level}</span> : null}
    </span>
  )
}
