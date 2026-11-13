import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { APP_NAME, APP_VERSION, Logo } from "./brand";
import { useEffect, useRef, useState } from "react";
import { api, useSettings, useUpdate } from "./api";
import { classColor, elementColor, randomRowColor, fmt, fmtFull, fmtTime, pct } from "./format";
import { classIcon, skillIcon } from "./skillIcons";
import type {
  AttackerView,
  ElementView,
  OverlaySettings,
  TargetHpView,
  Update,
} from "./types";

const WIDTH = 320;
/** Row colors: you are always blue, party members get the rest in order. */
const ME_COLOR = "#4e8ff2";
const PARTY_COLORS = [
  "#3fb6a8",
  "#a77bf3",
  "#e0a43a",
  "#e2647a",
  "#8bc34a",
  "#ef8a3c",
  "#5bc0eb",
];
const SKILL_COLORS = [
  "#4e8ff2",
  "#3fb6a8",
  "#a77bf3",
  "#e0a43a",
  "#e2647a",
  "#8bc34a",
  "#ef8a3c",
  "#5bc0eb",
];


/** "Fire 12.3M · Ice 400k" */
function ElementChips({ elements }: { elements: ElementView[] }) {
  if (elements.length === 0) return null;
  return (
    <div className="tabular mt-1.5 flex flex-wrap gap-x-3 gap-y-0.5 text-[11px]">
      {elements.map((e) => (
        <span key={e.name} className="flex items-center gap-1">
          <span
            className="inline-block h-2 w-2 rounded-full"
            style={{ background: elementColor(e.name) }}
          />
          <span style={{ color: elementColor(e.name) }}>{e.name}</span>
          <span className="font-semibold">{fmt(e.damage)}</span>
        </span>
      ))}
    </div>
  );
}

/** Thin stacked bar of a row's damage by element. */
function ElementBar({
  elements,
  total,
}: {
  elements: ElementView[];
  total: number;
}) {
  if (elements.length === 0 || total <= 0) return null;
  return (
    <div className="absolute inset-x-0 bottom-0 flex h-0.75">
      {elements.map((e) => (
        <div
          key={e.name}
          style={{
            width: `${(e.damage / total) * 100}%`,
            background: elementColor(e.name),
          }}
        />
      ))}
    </div>
  );
}

function LockIcon({ locked }: { locked: boolean }) {
  return (
    <svg
      viewBox="0 0 16 16"
      className="h-3.5 w-3.5"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
    >
      <rect
        x="3"
        y="7"
        width="10"
        height="7"
        rx="1.5"
        fill={locked ? "currentColor" : "none"}
      />
      {locked ? (
        <path d="M5 7V5a3 3 0 0 1 6 0v2" />
      ) : (
        <path d="M5 7V5a3 3 0 0 1 5.8-1" />
      )}
    </svg>
  );
}

/** Clicks on these don't start a window drag. */
const noDrag = (e: React.MouseEvent) => e.stopPropagation();

/** The HP bar target: the most important recent target whose rank is enabled. */
function hpTarget(
  update: Update | null,
  o: OverlaySettings,
  idle: number,
): TargetHpView | undefined {
  if (!o.showTargetHp || !update) return undefined;
  return update.snapshot.trackedTargets.find(
    (t) => o.hpRanks.includes(t.rank) && t.sinceLastHit < idle,
  );
}

function HpBlock({ t, o }: { t: TargetHpView; o: OverlaySettings }) {
  const color = t.dead
    ? "#52525b"
    : t.pct > 0.5
      ? "#3ecf6e"
      : t.pct > 0.2
        ? "#e0a43a"
        : "#e2544a";
  return (
    <div className="mt-1.5">
      {(o.showHpName || o.showHpNumbers || o.showHpPct) && (
        <div className="flex items-baseline gap-2 text-[11px]">
          {o.showHpName && (
            <span className="truncate font-semibold capitalize text-zinc-100">
              {t.name}
            </span>
          )}
          <span className="tabular ml-auto shrink-0 text-zinc-400">
            {t.dead
              ? "Dead"
              : o.showHpNumbers && `${fmtFull(t.hp)} / ${fmtFull(t.maxHp)}`}
          </span>
          {o.showHpPct && !t.dead && (
            <span className="tabular w-11 shrink-0 text-right font-semibold text-zinc-100">
              {(t.pct * 100).toFixed(1)}%
            </span>
          )}
        </div>
      )}
      <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-white/10">
        <div
          className="h-full transition-[width] duration-200"
          style={{ width: `${t.pct * 100}%`, background: color }}
        />
      </div>
    </div>
  );
}

/** A bar row like the in-game meters: fill = share of the top row. */
function BarRow({
  color,
  fill,
  left,
  right,
  onClick,
  children,
  highlight,
}: {
  color: string;
  fill: number;
  /** Your own row: outlined, since rows are colored by class. */
  highlight?: boolean;
  left: React.ReactNode;
  right: React.ReactNode;
  onClick?: () => void;
  children?: React.ReactNode;
}) {
  return (
    <div
      onMouseDown={onClick ? noDrag : undefined}
      onClick={onClick}
      className={`relative h-6 overflow-hidden rounded bg-white/4 text-xs ${
        onClick ? "cursor-pointer hover:bg-white/10" : ""
      } ${highlight ? "ring-1 ring-white/45 ring-inset" : ""}`}
    >
      <div
        className="absolute inset-y-0 left-0 transition-[width] duration-200"
        style={{
          width: `${Math.max(fill, 0.02) * 100}%`,
          background: color,
          opacity: 0.55,
        }}
      />
      <div className="tabular relative flex h-full items-center gap-2 px-2">
        <span className="truncate font-semibold">{left}</span>
        <span className="ml-auto flex shrink-0 items-center gap-2">
          {right}
        </span>
      </div>
      {children}
    </div>
  );
}

/** One character: headline numbers, then one row per damage source. */
function CharacterView({
  a,
  duration,
  o,
}: {
  a: AttackerView;
  duration: number;
  o: OverlaySettings;
}) {
  const top = a.sources[0]?.damage || 1;
  return (
    <>
      <div className="mt-1.5 flex items-end justify-between gap-3">
        {o.showDps && (
          <div>
            <div className="tabular text-3xl leading-none font-bold">
              {fmt(a.damage / duration)}
            </div>
            <div className="mt-1 text-[10px] text-zinc-400">DPS</div>
          </div>
        )}
        <dl className="tabular grid grid-cols-[auto_auto] gap-x-3 text-right text-xs">
          {o.showTotal && (
            <>
              <dt className="text-left text-zinc-400">Damage</dt>
              <dd className="font-semibold">{fmt(a.damage)}</dd>
            </>
          )}
          {o.showCrit && (
            <>
              <dt className="text-left text-zinc-400">Crit</dt>
              <dd className="text-crit font-semibold">{pct(a.critRate)}</dd>
            </>
          )}
          {o.showMaxHit && (
            <>
              <dt className="text-left text-zinc-400">Max hit</dt>
              <dd className="font-semibold">{fmt(a.maxHit)}</dd>
            </>
          )}
        </dl>
      </div>
      {o.showElements && <ElementChips elements={a.elements} />}
      {o.showSkills && a.sources.length > 0 && (
        <div className="mt-2 space-y-1">
          {a.sources
            .filter((s) => s.damage > 0 || (o.showCasts && s.casts > 0))
            .slice(0, o.maxRows)
            .map((s, i) => (
              <BarRow
                key={s.name}
                color={SKILL_COLORS[i % SKILL_COLORS.length]}
                fill={s.damage / top}
                left={
                  <span className="flex items-center" title={s.name}>
                    {!skillIcon(s.name) && o.showElements && s.elements[0] && (
                      <span
                        className="mr-1.5 inline-block h-2 w-2 rounded-full"
                        style={{ background: elementColor(s.elements[0].name) }}
                        title={s.elements
                          .map((e) => `${e.name} ${fmt(e.damage)}`)
                          .join(", ")}
                      />
                    )}
                    {skillIcon(s.name) ? (
                      <img
                        src={skillIcon(s.name)}
                        alt={s.name}
                        className="h-5 w-5 shrink-0 rounded-sm ring-1 ring-black/40"
                        draggable={false}
                      />
                    ) : (
                      <span className="truncate">{s.name}</span>
                    )}
                    {o.showCasts && s.casts > 0 && (
                      <span className="ml-1.5 font-normal text-zinc-400">
                        ×{s.casts}
                      </span>
                    )}
                  </span>
                }
                right={
                  s.damage === 0 ? (
                    <span className="text-zinc-500">no damage</span>
                  ) : (
                    <>
                      {o.showCrit && (
                        <span className="text-crit">{pct(s.critRate)}</span>
                      )}
                      {o.showTotal && (
                        <span className="text-zinc-300">{fmt(s.damage)}</span>
                      )}
                      {o.showDps && <span>{fmt(s.dps)}/s</span>}
                      <span className="w-9 text-right text-zinc-400">
                        {pct(s.share)}
                      </span>
                    </>
                  )
                }
              >
                {o.showElements && (
                  <ElementBar elements={s.elements} total={s.damage} />
                )}
              </BarRow>
            ))}
        </div>
      )}
    </>
  );
}

export default function Overlay() {
  const update = useUpdate();
  const [settings, saveSettings] = useSettings();
  const [selected, setSelected] = useState<number | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const o = settings?.overlay;
  const scale = o?.scale ?? 1;

  // Size the window to the card so the transparent area around it never blocks clicks.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const win = getCurrentWindow();
    const ro = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      win
        .setSize(new LogicalSize(Math.ceil(r.width), Math.ceil(r.height)))
        .catch(() => {});
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const locked = !!o?.locked;
  const onMouseDown = (e: React.MouseEvent) => {
    if (!locked && e.button === 0 && e.detail === 1)
      getCurrentWindow()
        .startDragging()
        .catch(() => {});
  };
  const toggleLock = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (settings)
      saveSettings({
        ...settings,
        overlay: { ...settings.overlay, locked: !locked },
      });
  };

  const enc = update?.snapshot.encounter;
  const attackers = enc?.attackers ?? [];
  const me = attackers.find((a) => a.isMe) ?? attackers[0];
  const party = !!o?.partyMode;
  // Party view drills into one character when its row is clicked.
  const drilled =
    party && selected !== null
      ? attackers.find((a) => a.id === selected)
      : undefined;
  const focus =
    o && settings ? hpTarget(update, o, settings.idleTimeout) : undefined;
  const bg = `rgba(15, 17, 21, ${o?.opacity ?? 0.85})`;
  // Rows take their class's color (or a random one per fight, per settings); without a known
  // class, the old you/party colors.
  const color = (a: AttackerView, i: number) =>
    o?.rowColors === "random" && enc
      ? randomRowColor(a.id, enc.start)
      : (classColor(a.class, o?.classColors) ??
        (a.isMe ? ME_COLOR : PARTY_COLORS[i % PARTY_COLORS.length]));

  return (
    <div
      ref={ref}
      onMouseDown={onMouseDown}
      style={{ width: o?.width || WIDTH, background: bg, zoom: scale }}
      className="cursor-default rounded-lg px-3 py-2.5 text-zinc-100"
    >
      <div
        onDoubleClick={() => !locked && api.resetMeter()}
        className="flex items-center gap-2 text-[11px] font-semibold tracking-wide text-zinc-400"
      >
        {drilled ? (
          <button
            type="button"
            onMouseDown={noDrag}
            onClick={() => setSelected(null)}
            className="-ml-1 rounded px-1 text-zinc-300 hover:bg-white/10 hover:text-white"
            title="Back to the party"
          >
            ‹ {drilled.name}
          </button>
        ) : (
          <span className="flex items-center gap-1.5">
            <Logo className="h-3.5 w-3.5" />
            {o?.showTimer && enc ? "Combat" : APP_NAME}
          </span>
        )}
        {o?.showTimer && enc && (
          <span
            className={`tabular font-normal ${enc.over ? "text-zinc-500" : "text-zinc-200"}`}
          >
            {fmtTime(enc.duration)}
            {enc.killed ? " · boss down" : enc.over && " · over"}
          </span>
        )}
        <span className="tabular ml-auto text-[9px] font-normal text-zinc-500" title={`${APP_NAME} version`}>
          v{APP_VERSION}
        </span>
        <button
          type="button"
          onMouseDown={noDrag}
          onDoubleClick={noDrag}
          onClick={toggleLock}
          title={
            locked ? "Locked: click to allow moving" : "Click to lock in place"
          }
          className={`-mr-1 rounded p-0.5 transition-colors ${
            locked
              ? "text-crit hover:text-amber-300"
              : "text-zinc-500 hover:text-zinc-200"
          }`}
        >
          <LockIcon locked={locked} />
        </button>
      </div>

      {o && focus && <HpBlock t={focus} o={o} />}

      {!o || !enc || !me ? (
        <div className="py-2 text-sm text-zinc-300">
          {update?.status.error
            ? "Capture error — see the app"
            : update?.status.running
              ? "Waiting for combat…"
              : "Capture stopped"}
        </div>
      ) : party && !drilled ? (
        <div className="mt-2 space-y-1">
          {/* Column titles, same widths as the numbers in each row. */}
          <div className="tabular flex items-center gap-2 px-2 text-[10px] font-medium tracking-wide text-zinc-500 uppercase">
            <span className="truncate">Player</span>
            <span className="ml-auto flex shrink-0 items-center gap-2">
              {o.showCrit && <span className={`${PARTY_COLS.crit} text-right`}>Crit</span>}
              {o.showTotal && <span className={`${PARTY_COLS.total} text-right`}>Damage</span>}
              {o.showDps && <span className={`${PARTY_COLS.dps} text-right`}>DPS</span>}
              {o.showShare && <span className={`${PARTY_COLS.share} text-right`}>Share</span>}
            </span>
          </div>
          {attackers.slice(0, o.maxRows).map((a, i) => (
            <BarRow
              key={a.id}
              color={color(a, i)}
              highlight={a.isMe && attackers.length > 1}
              fill={a.damage / (attackers[0].damage || 1)}
              left={
                <span className="flex items-center gap-1.5">
                  {a.class && <ClassIcon cls={a.class} level={a.level} />}
                  <span className="truncate">{a.name}</span>
                </span>
              }
              onClick={() => setSelected(a.id)}
              right={
                <>
                  {o.showCrit && (
                    <span className={`${PARTY_COLS.crit} text-right text-crit`}>{pct(a.critRate)}</span>
                  )}
                  {o.showTotal && (
                    <span className={`${PARTY_COLS.total} text-right text-zinc-300`}>{fmt(a.damage)}</span>
                  )}
                  {o.showDps && (
                    <span className={`${PARTY_COLS.dps} text-right font-semibold`}>{fmt(a.dps)}/s</span>
                  )}
                  {o.showShare && (
                    <span className={`${PARTY_COLS.share} text-right text-zinc-400`}>{pct(a.share)}</span>
                  )}
                </>
              }
            >
              {o.showElements && (
                <ElementBar elements={a.elements} total={a.damage} />
              )}
            </BarRow>
          ))}
        </div>
      ) : (
        <CharacterView a={drilled ?? me} duration={enc.duration} o={o} />
      )}
    </div>
  );
}

/** Fixed widths of the party view's number columns, so the titles line up with them. */
const PARTY_COLS = { crit: "w-9", total: "w-14", dps: "w-16", share: "w-9" };

/** Class icon for party rows (assets/class); a short colored tag if the icon is missing. */
function ClassIcon({ cls, level }: { cls: string; level?: number | null }) {
  const title = level ? `${cls} · level ${level}` : cls;
  const icon = classIcon(cls);
  if (icon) {
    return <img src={icon} alt={cls} title={title} className="h-4 w-4 shrink-0 rounded-sm" draggable={false} />;
  }
  const tag = cls.split(" ").map((w) => w[0]).join("").slice(0, 2).toUpperCase();
  return (
    <span title={title} className="shrink-0 rounded bg-white/10 px-1 text-[9px] leading-3.5 font-bold text-zinc-300">
      {tag}
    </span>
  );
}
