// Writes latest.json, the file the in-app updater downloads to see if there's a new version.
// Run after `npm run tauri build` (or use `npm run release`, which does both).
// Upload latest.json together with the installers and their .sig files to the GitHub release.
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const REPO = 'Lucifer0885/Rift-Meter'
const root = new URL('../', import.meta.url)
const conf = JSON.parse(readFileSync(new URL('src-tauri/tauri.conf.json', root), 'utf8'))
const version = conf.version
const name = conf.productName
const bundle = new URL('src-tauri/target/release/bundle/', root)
const base = `https://github.com/${REPO}/releases/download/v${version}`

const installers = {
  nsis: `nsis/${name}_${version}_x64-setup.exe`,
  msi: `msi/${name}_${version}_x64_en-US.msi`,
}

const platforms = {}
for (const [kind, path] of Object.entries(installers)) {
  const sig = new URL(`${path}.sig`, bundle)
  if (!existsSync(sig)) {
    console.error(`Missing ${fileURLToPath(sig)}. Build with the signing key set (see RELEASING.md).`)
    process.exit(1)
  }
  const file = path.split('/').pop()
  platforms[`windows-x86_64-${kind}`] = { signature: readFileSync(sig, 'utf8').trim(), url: `${base}/${file}` }
}
// Fallback for clients that don't say which installer they came from.
platforms['windows-x86_64'] = platforms['windows-x86_64-nsis']

const notes = process.argv[2] ?? `Riftmeter v${version}`
const out = { version, notes, pub_date: new Date().toISOString(), platforms }
const dest = new URL('latest.json', bundle)
writeFileSync(dest, JSON.stringify(out, null, 2) + '\n')
console.log(`Wrote ${fileURLToPath(dest)}`)
