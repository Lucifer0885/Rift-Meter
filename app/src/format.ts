export function fmt(n: number): string {
  const a = Math.abs(n)
  if (a >= 1e9) return (n / 1e9).toFixed(2) + 'B'
  if (a >= 1e6) return (n / 1e6).toFixed(2) + 'M'
  if (a >= 1e3) return (n / 1e3).toFixed(1) + 'K'
  return Math.round(n).toString()
}

export function fmtFull(n: number): string {
  return Math.round(n).toLocaleString('en-US')
}

export function fmtTime(sec: number): string {
  const s = Math.floor(sec)
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

export function pct(x: number): string {
  return `${Math.round(x * 100)}%`
}

export function clock(epochSec: number): string {
  return new Date(epochSec * 1000).toLocaleTimeString()
}

/** Element colors: ice blue, fire red, lightning light yellow, poison green, andermagic purple, physical white. */
const ELEMENT_COLORS: Record<string, string> = {
  Ice: "#4ea3f2",
  Fire: "#e5484d",
  Lightning: "#f5e27a",
  Poison: "#5fc95a",
  Andermagic: "#a77bf3",
  Physical: "#f4f4f5",
};
export const elementColor = (name: string) => ELEMENT_COLORS[name] ?? "#a1a1aa";

/** Default class colors: Ranger green, Spellweaver (mage) blue, Steam Mechanicus (dwarf) gray, Dragonknight red-orange. */
export const DEFAULT_CLASS_COLORS: Record<string, string> = {
  Ranger: "#3fae55",
  Spellweaver: "#3b7ff0",
  "Steam Mechanicus": "#8b9099",
  Dragonknight: "#e8562a",
};
export const CLASSES = Object.keys(DEFAULT_CLASS_COLORS);

/** A class's color, with the user's overrides from settings. */
export const classColor = (
  cls: string | null | undefined,
  overrides?: Record<string, string>,
): string | undefined => (cls ? (overrides?.[cls] ?? DEFAULT_CLASS_COLORS[cls]) : undefined);

/** A random-looking but steady color for a player in one fight (same id + fight start = same color). */
export function randomRowColor(id: number, fightStart: number): string {
  let h = (id ^ Math.floor(fightStart * 1000)) >>> 0;
  h = Math.imul(h ^ (h >>> 16), 0x45d9f3b) >>> 0;
  h = Math.imul(h ^ (h >>> 16), 0x45d9f3b) >>> 0;
  return `hsl(${h % 360} 65% 55%)`;
}
