import { useEffect, useState } from 'react'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { api, useSavedFights, useSettings, useUpdate } from './api'
import { CLASSES, DEFAULT_CLASS_COLORS, clock, fmt, fmtFull, fmtTime, pct } from './format'
import { RANKS, type CaptureStatus, type DeviceInfo, type EncounterView, type Settings, type Update } from './types'
import { AttackerTable } from './AttackerTable'
import { TitleBar } from './TitleBar'
import { HotkeyField } from './HotkeyField'
import { HpBar } from './HpBar'
import Layout from './Layout'
import { classIcon } from './skillIcons'
import { Button, Card, Slider, Stat, Toggle } from './ui'

type Tab = 'live' | 'history' | 'filters' | 'overlay' | 'layout' | 'capture'

const TABS: { id: Tab; label: string }[] = [
  { id: 'live', label: 'Live' },
  { id: 'history', label: 'History' },
  { id: 'filters', label: 'Filters' },
  { id: 'overlay', label: 'Overlay' },
  { id: 'layout', label: 'Layout' },
  { id: 'capture', label: 'Capture' },
]

export default function MainApp() {
  const update = useUpdate()
  const [settings, saveSettings] = useSettings()
  const [tab, setTab] = useState<Tab>('live')

  return (
    <div className="flex h-full flex-col">
      <TitleBar update={update} />
      <Header update={update} />
      <div className="flex min-h-0 flex-1">
        <nav className="w-40 shrink-0 border-r border-white/5 p-3">
          {TABS.map((t) => (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={`mb-1 block w-full rounded-md px-3 py-2 text-left text-sm transition-colors ${
                tab === t.id ? 'bg-white/10 text-white' : 'text-zinc-400 hover:bg-white/5 hover:text-zinc-200'
              }`}
            >
              {t.label}
            </button>
          ))}
        </nav>
        <main className="min-w-0 flex-1 overflow-y-auto p-5">
          {tab === 'live' && <LiveTab update={update} settings={settings} />}
          {tab === 'history' && <HistoryTab settings={settings} />}
          {settings && tab === 'filters' && <FiltersTab settings={settings} save={saveSettings} />}
          {settings && tab === 'overlay' && <OverlayTab settings={settings} save={saveSettings} />}
          {settings && tab === 'layout' && <Layout settings={settings} save={saveSettings} />}
          {settings && tab === 'capture' && (
            <CaptureTab settings={settings} save={saveSettings} status={update?.status} />
          )}
        </main>
      </div>
    </div>
  )
}

function StatusPill({ status }: { status?: CaptureStatus }) {
  if (!status) return null
  let color = 'bg-zinc-500'
  let text = 'Stopped'
  if (status.error) {
    color = 'bg-red-500'
    text = 'Error'
  } else if (status.running) {
    const fresh = status.lastGameTraffic !== null && status.lastGameTraffic < 5
    color = fresh ? 'bg-emerald-500' : 'bg-amber-500'
    text = fresh ? 'Receiving game traffic' : 'Capturing, no game traffic yet'
  }
  return (
    <span className="flex items-center gap-2 text-sm text-zinc-300">
      <span className={`h-2.5 w-2.5 rounded-full ${color}`} />
      {text}
    </span>
  )
}

function Header({ update }: { update: Update | null }) {
  const status = update?.status
  return (
    <header className="flex items-center gap-4 border-b border-white/5 px-5 py-3">
      <StatusPill status={status} />
      {status?.running && status.deviceLabel && (
        <span className="truncate text-xs text-zinc-500">on {status.deviceLabel}</span>
      )}
      <div className="ml-auto flex gap-2">
        <Button onClick={() => api.resetMeter()}>Reset fight</Button>
        {status?.running ? (
          <Button onClick={() => api.stopCapture()}>Stop</Button>
        ) : (
          <Button variant="primary" onClick={() => api.startCapture().catch(() => {})}>
            Start
          </Button>
        )}
      </div>
    </header>
  )
}

function rankBadge(rank: string) {
  const colors: Record<string, string> = {
    monster: 'bg-zinc-700 text-zinc-300',
    minion: 'bg-zinc-800 text-zinc-400',
    add: 'bg-rose-950 text-rose-300',
    guardian: 'bg-sky-900 text-sky-200',
    champion: 'bg-violet-900 text-violet-200',
    leader: 'bg-orange-900 text-orange-200',
    boss: 'bg-red-900 text-red-200',
    worldboss: 'bg-red-800 text-red-100',
  }
  return (
    <span className={`rounded px-1.5 py-0.5 text-[11px] font-medium ${colors[rank] ?? 'bg-zinc-800 text-zinc-500'}`}>
      {rank}
    </span>
  )
}

function LiveTab({ update, settings }: { update: Update | null; settings: Settings | null }) {
  const enc = update?.snapshot.encounter
  const me = enc?.attackers.find((a) => a.isMe) ?? enc?.attackers[0]
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-5 gap-3">
        <Stat label="DPS" value={me ? fmt(me.damage / enc!.duration) : '—'} />
        <Stat label="Total" value={me ? fmt(me.damage) : '—'} sub={me ? fmtFull(me.damage) : undefined} />
        <Stat
          label="Crit"
          value={me ? pct(me.critRate) : '—'}
          sub={me ? `${me.crits} / ${me.hits} hits` : undefined}
          accent="text-crit"
        />
        <Stat label="Max hit" value={me ? fmt(me.maxHit) : '—'} sub={me ? fmtFull(me.maxHit) : undefined} />
        <Stat
          label={enc?.over ? 'Fight over' : 'Fight time'}
          value={enc ? fmtTime(enc.duration) : '—'}
          sub={enc?.over ? 'next hit starts a new fight' : undefined}
        />
      </div>

      {!enc && (
        <Card>
          <p className="text-sm text-zinc-400">
            Waiting for combat. Hit something in game and the numbers show up here and in the overlay.
          </p>
        </Card>
      )}

      <TargetHpCard update={update} />

      {enc && (
        <div className="grid grid-cols-1 gap-4 2xl:grid-cols-[minmax(0,1fr)_20rem]">
          <Card title="Damage">
            <AttackerTable enc={enc} classColors={settings?.overlay.classColors} />
          </Card>
          <Card title="Targets">
            <ul className="space-y-2 text-sm">
              {enc.targets.map((t) => (
                <li key={t.name + t.rank} className="flex items-center gap-2">
                  {rankBadge(t.rank)}
                  <span className="truncate">{t.name}</span>
                  <span className="tabular ml-auto text-zinc-400">{fmt(t.damage)}</span>
                </li>
              ))}
            </ul>
          </Card>
        </div>
      )}

      <Card title="Recent hits">
        <div className="max-h-72 overflow-y-auto">
          <table className="tabular w-full text-sm">
            <tbody>
              {(update?.snapshot.recentHits ?? []).map((h, i) => (
                <tr key={i} className={`border-t border-white/5 ${h.counted ? '' : 'opacity-40'}`}>
                  <td className="py-1.5 text-xs text-zinc-500">{clock(h.time)}</td>
                  <td className={`py-1.5 text-right font-medium ${h.crit ? 'text-crit' : ''}`}>
                    {fmtFull(h.damage)}
                    {h.crit ? '!' : ''}
                  </td>
                  <td className="py-1.5 pl-4 text-zinc-400">{h.isMe ? 'You' : `Player ${(h.owner & 0xffff).toString(16)}`}</td>
                  <td className="py-1.5">→</td>
                  <td className="py-1.5">
                    <span className="mr-2">{h.targetName}</span>
                    {rankBadge(h.targetRank)}
                  </td>
                  <td className="py-1.5 text-right text-xs text-zinc-500">{h.counted ? '' : 'filtered out'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
    </div>
  )
}

function TargetHpCard({ update }: { update: Update | null }) {
  const focus = update?.snapshot.focusTarget
  const others = (update?.snapshot.trackedTargets ?? []).filter((t) => t.id !== focus?.id).slice(0, 6)
  if (!focus && others.length === 0) return null
  return (
    <Card title="Target HP">
      {focus && (
        <div className="mb-4">
          <HpBar t={focus} />
          <div className="tabular mt-1 text-xs text-zinc-500">
            Your damage on it: {fmt(focus.myDamage)}
            {focus.maxHp > 0 && ` (${pct(focus.myDamage / focus.maxHp)} of its HP)`}
          </div>
        </div>
      )}
      {others.length > 0 && (
        <div className="grid grid-cols-2 gap-x-6 gap-y-3 border-t border-white/5 pt-3">
          {others.map((t) => (
            <HpBar key={t.id} t={t} compact />
          ))}
        </div>
      )}
    </Card>
  )
}

function HistoryTab({ settings }: { settings: Settings | null }) {
  const { fights, dir } = useSavedFights()
  const [open, setOpen] = useState<string | null>(null)
  const [confirm, setConfirm] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const remove = (file: string) => {
    setConfirm(null)
    api.deleteFight(file).catch((e) => setError(String(e)))
  }
  return (
    <Card
      title="Saved fights"
      right={
        <div className="flex items-center gap-3">
          {dir && (
            <span className="max-w-80 truncate text-xs text-zinc-500" title={dir.path}>
              {dir.path}
            </span>
          )}
          <Button onClick={() => api.openHistoryDir().catch((e) => setError(String(e)))}>Open folder</Button>
        </div>
      }
    >
      {settings && !settings.saveFights && (
        <p className="mb-2 text-xs text-crit">Saving is off (Filters → Saved fights). New fights won't appear here.</p>
      )}
      {error && <p className="mb-2 text-xs text-red-400">{error}</p>}
      {fights?.length === 0 && <p className="text-sm text-zinc-400">No saved fights in this folder yet.</p>}
      <ul className="divide-y divide-white/5">
        {(fights ?? []).map(({ file, fight: e }) => {
          const me = e.attackers.find((a) => a.isMe) ?? e.attackers[0]
          return (
            <li key={file} className="py-2">
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setOpen(open === file ? null : file)}
                  className="tabular flex min-w-0 flex-1 items-center gap-4 text-left text-sm"
                >
                  <span className="w-32 shrink-0 text-zinc-500">{dateTime(e.start)}</span>
                  <span className="w-14 shrink-0">{fmtTime(e.duration)}</span>
                  <span className="w-28 shrink-0 font-medium">{me ? fmt(me.damage / e.duration) : '—'} DPS</span>
                  <span className="w-24 shrink-0 text-zinc-400">{fmt(e.total)} total</span>
                  <span className="w-20 shrink-0 text-crit">{me ? pct(me.critRate) : ''} crit</span>
                  <span className="truncate text-zinc-400 capitalize">{mainTarget(e)?.name ?? e.killed}</span>
                  {mainTarget(e) && rankBadge(mainTarget(e)!.rank)}
                  {e.killed && <span className="shrink-0 text-xs text-emerald-400">killed</span>}
                </button>
                {confirm === file ? (
                  <span className="flex shrink-0 gap-1">
                    <Button variant="danger" onClick={() => remove(file)}>
                      Delete
                    </Button>
                    <Button onClick={() => setConfirm(null)}>Cancel</Button>
                  </span>
                ) : (
                  <button
                    type="button"
                    onClick={() => setConfirm(file)}
                    title="Delete this fight"
                    className="shrink-0 rounded p-1.5 text-zinc-500 hover:bg-white/10 hover:text-red-400"
                  >
                    <TrashIcon />
                  </button>
                )}
              </div>
              {open === file && (
                <div className="mt-3">
                  <AttackerTable enc={e} classColors={settings?.overlay.classColors} />
                </div>
              )}
            </li>
          )
        })}
      </ul>
    </Card>
  )
}

/** The fight's named target: the killed boss, else the most-damaged target with a known name. */
const mainTarget = (e: EncounterView) =>
  e.targets.find((t) => t.name === e.killed) ?? e.targets.find((t) => t.name !== 'unknown') ?? e.targets[0]

function TrashIcon() {
  return (
    <svg viewBox="0 0 16 16" className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth="1.5">
      <path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.5h5.8l.6-8.5M7 7v4M9 7v4" />
    </svg>
  )
}

/** "11 Nov, 21:04" */
const dateTime = (epochSec: number) =>
  new Date(epochSec * 1000).toLocaleString(undefined, {
    day: 'numeric',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  })

function SavedFightsCard({ settings, save }: { settings: Settings; save: (s: Settings) => void }) {
  const { dir } = useSavedFights()
  const set = (patch: Partial<Settings>) => save({ ...settings, ...patch })
  const pick = async () => {
    const chosen = await openDialog({ directory: true, defaultPath: dir?.path, title: 'Folder for saved fights' })
    if (typeof chosen === 'string') set({ historyDir: chosen })
  }
  return (
    <Card title="Saved fights">
      <Toggle
        label="Save fights to disk"
        hint="Each finished fight is written as a JSON file and listed in History."
        checked={settings.saveFights}
        onChange={(v) => set({ saveFights: v })}
      />
      <div className="py-1.5">
        <div className="text-sm text-zinc-200">Folder</div>
        <div className="mt-1 flex items-center gap-2">
          <code
            className="min-w-0 flex-1 truncate rounded bg-black/30 px-2 py-1.5 text-xs text-zinc-300"
            title={dir?.path}
          >
            {dir?.path ?? '…'}
          </code>
          <Button onClick={pick}>Change…</Button>
          <Button onClick={() => api.openHistoryDir()}>Open</Button>
          {dir && !dir.isDefault && <Button onClick={() => set({ historyDir: null })}>Use default</Button>}
        </div>
        <div className="mt-1 text-xs text-zinc-500">
          Fights already saved stay in the old folder. History lists the current folder only.
        </div>
      </div>
      <Slider
        label="Only save fights longer than"
        value={settings.minSavedFight}
        min={0}
        max={60}
        step={5}
        display={settings.minSavedFight === 0 ? 'save all' : `${settings.minSavedFight}s`}
        onChange={(v) => set({ minSavedFight: v })}
      />
    </Card>
  )
}

function FiltersTab({ settings, save }: { settings: Settings; save: (s: Settings) => void }) {
  const set = (patch: Partial<Settings>) => save({ ...settings, ...patch })
  const toggleRank = (id: string, on: boolean) =>
    set({ ranks: on ? [...settings.ranks, id] : settings.ranks.filter((r) => r !== id) })
  const preset = (ranks: string[]) => set({ ranks })
  return (
    <div className="max-w-xl space-y-4">
      <Card title="Count damage against">
        <p className="mb-3 text-xs text-zinc-500">
          Nothing selected counts every target. Filtered hits still show in the hit log, greyed out.
        </p>
        <div className="mb-3 flex flex-wrap gap-2">
          <Button onClick={() => preset([])}>Everything</Button>
          <Button onClick={() => preset(['guardian', 'champion', 'leader', 'boss', 'worldboss'])}>Elites and up</Button>
          <Button onClick={() => preset(['leader', 'boss', 'worldboss'])}>Bosses only</Button>
        </div>
        {RANKS.map((r) => (
          <Toggle
            key={r.id}
            label={r.label}
            hint={r.hint}
            checked={settings.ranks.includes(r.id)}
            onChange={(v) => toggleRank(r.id, v)}
          />
        ))}
        <div className="mt-2 border-t border-white/5 pt-2">
          <Toggle
            label="Also count unknown targets"
            hint="Mobs that were already nearby when capture started have no known rank."
            checked={settings.includeUnknown}
            onChange={(v) => set({ includeUnknown: v })}
          />
        </div>
      </Card>
      <Card title="Fights">
        <Slider
          label="End a fight after no hits for"
          value={settings.idleTimeout}
          min={5}
          max={120}
          step={5}
          display={`${settings.idleTimeout}s`}
          onChange={(v) => set({ idleTimeout: v })}
        />
        <Toggle
          label="Stop the timer when the boss dies"
          hint="The fight ends at the boss kill, so DPS isn't diluted by looting and walking. The next hit starts a new fight."
          checked={settings.stopOnBossKill}
          onChange={(v) => set({ stopOnBossKill: v })}
        />
      </Card>
      <SavedFightsCard settings={settings} save={save} />
    </div>
  )
}

function OverlayTab({ settings, save }: { settings: Settings; save: (s: Settings) => void }) {
  const o = settings.overlay
  const set = (patch: Partial<Settings['overlay']>) => save({ ...settings, overlay: { ...o, ...patch } })
  const toggleHpRank = (id: string, on: boolean) =>
    set({ hpRanks: on ? [...o.hpRanks, id] : o.hpRanks.filter((r) => r !== id) })
  return (
    <div className="grid max-w-4xl grid-cols-2 gap-4">
      <Card title="Whose damage">
        <div className="mb-2 grid grid-cols-2 gap-2">
          {[
            { party: false, label: 'Only me', hint: 'Your numbers, split by skill' },
            { party: true, label: 'Whole party', hint: 'One row per character; click a row for its skills' },
          ].map((m) => (
            <button
              key={m.label}
              type="button"
              onClick={() => set({ partyMode: m.party })}
              className={`rounded-md border px-3 py-2 text-left transition-colors ${
                o.partyMode === m.party ? 'border-me bg-me/15' : 'border-white/10 hover:bg-white/5'
              }`}
            >
              <div className="text-sm font-medium text-zinc-100">{m.label}</div>
              <div className="text-xs text-zinc-500">{m.hint}</div>
            </button>
          ))}
        </div>
        <Toggle
          label="Skill breakdown"
          hint="One row per skill under the character's numbers. Summons count under the skill that makes them."
          checked={o.showSkills}
          onChange={(v) => set({ showSkills: v })}
        />
        <Toggle
          label="Times used"
          hint="How often you used each skill, including ones that deal no damage (yours only)."
          checked={o.showCasts}
          onChange={(v) => set({ showCasts: v })}
        />
        <Toggle
          label="Elemental damage"
          hint="Split damage by element: Fire, Ice, Lightning. Includes burns and explosions from talents (for example Master of Ice after Frost Wind)."
          checked={o.showElements}
          onChange={(v) => set({ showElements: v })}
        />
        <Slider
          label="Max rows"
          value={o.maxRows}
          min={1}
          max={10}
          step={1}
          display={String(o.maxRows)}
          onChange={(v) => set({ maxRows: v })}
        />
      </Card>

      <Card title="Numbers">
        <Toggle label="DPS" checked={o.showDps} onChange={(v) => set({ showDps: v })} />
        <Toggle label="Damage" checked={o.showTotal} onChange={(v) => set({ showTotal: v })} />
        <Toggle label="Crit rate" checked={o.showCrit} onChange={(v) => set({ showCrit: v })} />
        <Toggle label="Max hit" hint="Character view only." checked={o.showMaxHit} onChange={(v) => set({ showMaxHit: v })} />
        <Toggle label="Party share %" hint="Party view only." checked={o.showShare} onChange={(v) => set({ showShare: v })} />
        <Toggle label="Combat timer" checked={o.showTimer} onChange={(v) => set({ showTimer: v })} />
      </Card>

      <Card title="Health bar">
        <Toggle
          label="Show mob health bar"
          hint="Above the rows, for the most important mob you're fighting."
          checked={o.showTargetHp}
          onChange={(v) => set({ showTargetHp: v })}
        />
        {o.showTargetHp && (
          <>
            <div className="mt-2 mb-1 text-xs text-zinc-500">For these mob types</div>
            <div className="mb-2 flex flex-wrap gap-1.5">
              {[...RANKS, { id: 'unknown', label: 'Unknown', hint: '' }].map((r) => {
                const on = o.hpRanks.includes(r.id)
                return (
                  <button
                    key={r.id}
                    type="button"
                    onClick={() => toggleHpRank(r.id, !on)}
                    className={`rounded-full border px-2.5 py-1 text-xs transition-colors ${
                      on ? 'border-me bg-me/20 text-zinc-100' : 'border-white/10 text-zinc-400 hover:bg-white/5'
                    }`}
                  >
                    {r.label}
                  </button>
                )
              })}
            </div>
            <Toggle label="Mob name" checked={o.showHpName} onChange={(v) => set({ showHpName: v })} />
            <Toggle
              label="HP numbers"
              hint="e.g. 14,546,156 / 38,564,199"
              checked={o.showHpNumbers}
              onChange={(v) => set({ showHpNumbers: v })}
            />
            <Toggle label="HP %" checked={o.showHpPct} onChange={(v) => set({ showHpPct: v })} />
          </>
        )}
      </Card>

      <RowColorsCard o={o} set={set} />

      <Card title="Window">
        <Toggle label="Show overlay" checked={o.visible} onChange={(v) => set({ visible: v })} />
        <HotkeyField settings={settings} />
        <Toggle
          label="Lock in place"
          hint="Stops dragging or resetting by clicking the overlay. Move it from the Layout page."
          checked={o.locked}
          onChange={(v) => set({ locked: v })}
        />
        <Toggle
          label="Click-through"
          hint="Clicks go to the game (rows and the lock icon can't be clicked either)."
          checked={o.clickThrough}
          onChange={(v) => set({ clickThrough: v })}
        />
        <Slider
          label="Background opacity"
          value={o.opacity}
          min={0}
          max={1}
          step={0.05}
          display={pct(o.opacity)}
          onChange={(v) => set({ opacity: v })}
        />
        <Slider
          label="Size"
          value={o.scale}
          min={0.7}
          max={1.6}
          step={0.05}
          display={`${Math.round(o.scale * 100)}%`}
          onChange={(v) => set({ scale: v })}
        />
        <Slider
          label="Width"
          value={o.width}
          min={260}
          max={720}
          step={20}
          display={`${o.width}px`}
          onChange={(v) => set({ width: v })}
        />
      </Card>
    </div>
  )
}

function CaptureTab({
  settings,
  save,
  status,
}: {
  settings: Settings
  save: (s: Settings) => void
  status?: CaptureStatus
}) {
  const [devices, setDevices] = useState<DeviceInfo[]>([])
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    api.listInterfaces().then(setDevices).catch((e) => setError(String(e)))
  }, [])
  const auto = devices.find((d) => d.isDefault)
  return (
    <div className="max-w-xl space-y-4">
      <Card title="Network adapter">
        <select
          value={settings.interface ?? ''}
          onChange={(e) => save({ ...settings, interface: e.target.value || null })}
          className="w-full rounded-md border border-white/10 bg-zinc-900 px-3 py-2 text-sm"
        >
          <option value="">Automatic{auto ? ` (${auto.description || auto.name})` : ''}</option>
          {devices.map((d) => (
            <option key={d.name} value={d.name}>
              {d.description || d.name}
              {d.ipv4.length ? ` — ${d.ipv4.join(', ')}` : ''}
            </option>
          ))}
        </select>
        <p className="mt-2 text-xs text-zinc-500">
          Automatic picks the adapter Windows uses for the internet. Capture is passive: it only reads
          traffic and never touches the game.
        </p>
        <div className="mt-2">
          <Toggle
            label="Start capturing when the app opens"
            checked={settings.autoStart}
            onChange={(v) => save({ ...settings, autoStart: v })}
          />
        </div>
      </Card>
      <Card title="Status">
        <dl className="tabular grid grid-cols-2 gap-y-1 text-sm">
          <dt className="text-zinc-500">Capturing</dt>
          <dd>{status?.running ? 'yes' : 'no'}</dd>
          <dt className="text-zinc-500">UDP packets</dt>
          <dd>{status?.packets ?? 0}</dd>
          <dt className="text-zinc-500">Game messages</dt>
          <dd>{status?.gameMessages ?? 0}</dd>
          <dt className="text-zinc-500">Last game traffic</dt>
          <dd>{status?.lastGameTraffic != null ? `${status.lastGameTraffic.toFixed(1)}s ago` : 'none yet'}</dd>
        </dl>
        {(status?.error || error) && (
          <p className="mt-3 rounded-md bg-red-500/10 p-2 text-sm text-red-300">{status?.error || error}</p>
        )}
      </Card>
    </div>
  )
}

/** Party row colors: one color per class (editable), or a random color per player each fight. */
function RowColorsCard({
  o,
  set,
}: {
  o: Settings['overlay']
  set: (patch: Partial<Settings['overlay']>) => void
}) {
  const random = o.rowColors === 'random'
  const setClassColor = (cls: string, color: string | null) => {
    const next = { ...o.classColors }
    if (color === null) delete next[cls]
    else next[cls] = color
    set({ classColors: next })
  }
  return (
    <Card title="Party row colors">
      <div className="mb-3 grid grid-cols-2 gap-2">
        {[
          { mode: 'class' as const, label: 'By class', hint: 'Each class has its own color' },
          { mode: 'random' as const, label: 'Random', hint: 'A new color per player every fight' },
        ].map((m) => (
          <button
            key={m.mode}
            type="button"
            onClick={() => set({ rowColors: m.mode })}
            className={`rounded-md border px-3 py-2 text-left transition-colors ${
              o.rowColors === m.mode ? 'border-me bg-me/15' : 'border-white/10 hover:bg-white/5'
            }`}
          >
            <div className="text-sm font-medium text-zinc-100">{m.label}</div>
            <div className="text-xs text-zinc-500">{m.hint}</div>
          </button>
        ))}
      </div>
      <div className={`space-y-1.5 ${random ? 'pointer-events-none opacity-40' : ''}`}>
        {CLASSES.map((cls) => {
          const value = o.classColors[cls] ?? DEFAULT_CLASS_COLORS[cls]
          const changed = o.classColors[cls] !== undefined
          return (
            <label key={cls} className="flex items-center gap-2.5 text-sm text-zinc-200">
              {classIcon(cls) ? (
                <img src={classIcon(cls)} alt="" className="h-5 w-5 rounded-sm" draggable={false} />
              ) : (
                <span className="h-5 w-5" />
              )}
              <span className="flex-1">{cls}</span>
              {changed && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.preventDefault()
                    setClassColor(cls, null)
                  }}
                  className="text-xs text-zinc-500 hover:text-zinc-200"
                >
                  Reset
                </button>
              )}
              <input
                type="color"
                value={value}
                onChange={(e) => setClassColor(cls, e.target.value)}
                className="h-7 w-12 cursor-pointer rounded border border-white/10 bg-transparent"
                title={`Change the ${cls} color`}
              />
            </label>
          )
        })}
      </div>
      {random && <p className="mt-2 text-xs text-zinc-500">Class colors are kept for when you switch back.</p>}
    </Card>
  )
}
