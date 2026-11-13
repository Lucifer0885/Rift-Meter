// Icons from <repo>/assets, matched by name. Drop a new PNG in and it is picked up.
// Skills: assets/skills/<class>/<skill-name>.png ("Guardian of Flames" -> guardian-of-flames.png).
// Mercenaries and jewels: assets/mercenary/*.png and assets/jewel/*.png, mapped from their in-game names below.
const files = import.meta.glob(['../../assets/skills/*/*.png', '../../assets/mercenary/*.png', '../../assets/jewel/*.png', '../../assets/class/*.png', '../../assets/general/*.png'], {
  eager: true,
  query: '?url',
  import: 'default',
}) as Record<string, string>

const bySlug: Record<string, string> = {}
for (const [path, url] of Object.entries(files)) {
  const slug = path.split('/').pop()!.replace(/\.png$/, '')
  bySlug[slug] = url
}

/** In-game mercenary/jewel name (as the meter reports it) -> icon file name. */
const NAMED_ICONS: Record<string, string> = {
  'Viridis the Sylvan Treant': 'viridis',
  'Ember Knight Kain': 'ember-knight-kain',
  'Sacred Expedition': 'divine-wanderer',
  'Blade Sentinel': 'ancient-watcher',
  'Twilight Wrath': 'shadow-stalker',
  'Jewel of Glacial Fang': 'jewel-buff-snow-wolf',
  'Jewel of Eternal Scorn': 'jewel-eternal-scorn',
  "Agathon's Guard": 'agathon-guard',
  'Shrapnel Shot': 'sharpnel-shot',
  // Elemental masteries: same icons for every class.
  'Master of Fire': 'fire-mastery',
  'Master of Ice': 'ice-mastery',
  'Master of Lightning': 'lightning-mastery',
  'Master of Poison': 'poison-mastery',
  Shieldwall: 'dwarf_skill_shieldwall',
}

const slugify = (name: string) =>
  name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')

/** Icon URL for a damage row (skill or mercenary), or undefined ("Other hits", "Fire effects"...). */
export const skillIcon = (name: string): string | undefined =>
  bySlug[NAMED_ICONS[name] ?? slugify(name)]

/** Class (as the meter reports it) -> assets/class/<file>.png. */
const CLASS_ICONS: Record<string, string> = {
  Dragonknight: 'class-dragonknight',
  Spellweaver: 'class-spellweaver',
  Ranger: 'class-ranger',
  'Steam Mechanicus': 'class-dwarf',
}

export const classIcon = (cls: string): string | undefined => bySlug[CLASS_ICONS[cls]]
