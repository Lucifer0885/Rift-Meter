// Mirrors the Rust structs in src-tauri/src/{meter,settings,lib}.rs (camelCase).

export interface OverlaySettings {
  visible: boolean
  clickThrough: boolean
  locked: boolean
  opacity: number
  scale: number
  width: number
  /** Party row colors: by class, or a random color per player each fight. */
  rowColors: 'class' | 'random'
  /** Class name -> '#rrggbb' overrides; missing classes use the defaults. */
  classColors: Record<string, string>
  showDps: boolean
  showTotal: boolean
  showCrit: boolean
  showMaxHit: boolean
  showTimer: boolean
  showShare: boolean
  partyMode: boolean
  showSkills: boolean
  showCasts: boolean
  showElements: boolean
  maxRows: number
  showTargetHp: boolean
  hpRanks: string[]
  showHpName: boolean
  showHpNumbers: boolean
  showHpPct: boolean
}

export interface Settings {
  interface: string | null
  autoStart: boolean
  idleTimeout: number
  stopOnBossKill: boolean
  saveFights: boolean
  historyDir: string | null
  minSavedFight: number
  overlayHotkey: string | null
  ranks: string[]
  includeUnknown: boolean
  overlay: OverlaySettings
}

export interface SourceView {
  name: string
  damage: number
  dps: number
  hits: number
  crits: number
  critRate: number
  maxHit: number
  share: number
  casts: number
  elements: ElementView[]
}

export interface ElementView {
  name: string
  damage: number
}

export interface AttackerView {
  id: number
  name: string
  isMe: boolean
  damage: number
  dps: number
  hits: number
  crits: number
  critRate: number
  maxHit: number
  share: number
  sources: SourceView[]
  elements: ElementView[]
  /** "Ranger", "Spellweaver", "Dragonknight", "Steam Mechanicus"; null if unknown. */
  class?: string | null
  level?: number | null
}

export interface TargetView {
  name: string
  rank: string
  damage: number
}

export interface SavedFight {
  file: string
  fight: EncounterView
}

export interface HistoryDirInfo {
  path: string
  isDefault: boolean
  defaultPath: string
}

export interface EncounterView {
  start: number
  duration: number
  over: boolean
  killed: string | null
  total: number
  attackers: AttackerView[]
  targets: TargetView[]
}

export interface HitView {
  time: number
  damage: number
  crit: boolean
  attacker: number
  owner: number
  isMe: boolean
  target: number
  targetName: string
  targetRank: string
  counted: boolean
}

export interface TargetHpView {
  id: number
  name: string
  rank: string
  hp: number
  maxHp: number
  pct: number
  dead: boolean
  dps: number
  myDamage: number
  timeToKill: number | null
  fightTime: number
  sinceLastHit: number
}

export const ELITE_RANKS = ['guardian', 'champion', 'leader', 'boss', 'worldboss']

/** Your character, known outside fights too (zone load or your player record). */
export interface PlayerView {
  name: string | null
  class: string | null
  level: number | null
}

export interface Snapshot {
  me: number | null
  player: PlayerView | null
  encounter: EncounterView | null
  focusTarget: TargetHpView | null
  trackedTargets: TargetHpView[]
  recentHits: HitView[]
  history: EncounterView[]
  knownEntities: number
}

export interface CaptureStatus {
  running: boolean
  device: string | null
  deviceLabel: string | null
  error: string | null
  packets: number
  gameMessages: number
  lastGameTraffic: number | null
}

export interface Update {
  status: CaptureStatus
  snapshot: Snapshot
}

/** A display, in physical pixels on the virtual desktop. */
export interface MonitorInfo {
  name: string
  x: number
  y: number
  width: number
  height: number
  scale: number
  primary: boolean
}

/** Overlay window position and size, physical pixels. */
export interface OverlayGeometry {
  x: number
  y: number
  width: number
  height: number
}

export interface DeviceInfo {
  name: string
  description: string
  ipv4: string[]
  isDefault: boolean
}

/** Rank categories the game client uses (see rank_category in decode.rs). */
export const RANKS = [
  { id: 'monster', label: 'Monster', hint: 'normal trash mobs' },
  { id: 'guardian', label: 'Guardian', hint: 'tier 1 marker' },
  { id: 'champion', label: 'Champion', hint: 'tier 2 marker' },
  { id: 'leader', label: 'Leader', hint: 'tier 3 marker' },
  { id: 'boss', label: 'Boss', hint: 'tier 3 marker' },
  { id: 'worldboss', label: 'World boss', hint: 'tier 3 marker' },
  { id: 'add', label: 'Boss adds', hint: 'mobs a boss summons during the fight' },
] as const
