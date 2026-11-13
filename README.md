<p align="center">
  <img src="app/public/logo.svg" width="96" alt="Riftmeter logo">
</p>

<h1 align="center">Riftmeter</h1>

<p align="center">A passive DPS meter and overlay for Drakensang Online.</p>

Riftmeter works out your damage by reading the game's network traffic with Npcap. It never
touches the game process: it doesn't read memory, inject code or change packets. It only
listens to traffic that is already arriving on your PC.

> Riftmeter is an unofficial fan project. It is not affiliated with, endorsed by or sponsored by
> the developers or publishers of Drakensang Online. Use it at your own risk.

## Features

- **Live overlay.** An always-on-top window shows DPS, total damage, crit rate, share and the
  fight timer for you and your party. Click a row to see that player's damage per skill and per
  element. You can drag it, lock it in place, scale it, change its opacity and choose which
  columns to show.
- **Party meter.** Party members show with their name, class icon and level. Rows are coloured by
  class, and pet, summon and mercenary damage counts towards its owner.
- **Target health bar.** Shows the HP of the boss or elite you are fighting. You choose which
  ranks get a bar.
- **Fight handling.** A fight ends after a quiet spell with no hits (30 s by default), or the
  moment the boss dies, so walking and looting don't drag your DPS down. You can count only
  elites, only bosses, or everything.
- **Fight history.** Finished fights are saved to disk, so you can open them later in the History
  tab.
- **Overlay layout editor.** Move the overlay on a scaled map of your monitor, with a snap grid
  and preset positions.
- **Hotkey.** `Ctrl+Shift+O` shows and hides the overlay. You can change it in the settings.

## Install

1. Install [Npcap](https://npcap.com/#download). The default options are fine.
2. Download the installer from the [Releases page](https://github.com/Lucifer0885/Rift-Meter/releases)
   and run it. Riftmeter updates itself from then on.
3. Start Riftmeter, then the game. Capture starts by itself. If the status stays on "no game
   traffic", choose your network adapter in the **Capture** tab.

Run the game in windowed or borderless mode so the overlay stays on top of it.

## Build from source

You need:

- Windows 10 or 11 with [Npcap](https://npcap.com/#download) installed
- [Node.js](https://nodejs.org/) 20.19 or 22.12 and newer
- [Rust](https://rustup.rs/) 1.90 or newer
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for Windows (Microsoft
  C++ Build Tools and WebView2)

Npcap's `wpcap.dll` is loaded at runtime, so you don't need the Npcap SDK to build.

```bash
cd app
npm install
npm run tauri dev
```

Build the installers (NSIS and MSI), which end up in `app/src-tauri/target/release/bundle/`:

```bash
npm run tauri build
```

The version number is set in `app/src-tauri/tauri.conf.json`, and the app shows it in the header
and on the overlay. [RELEASING.md](RELEASING.md) covers signing, publishing a release and the
auto-updater.

## Project layout

| Path | What's there |
|---|---|
| `app/src/` | React + TypeScript UI for the main window and the overlay |
| `app/src-tauri/src/` | Rust backend: packet capture, RakNet parsing, decoding, the meter and fight history |
| `app/src-tauri/src/bin/` | Developer tools that run saved captures through the decoder (`replay`, `hitdump`, `msgdump`, `skillscan`) |
| `assets/` | Class and skill icons shown in the UI |

## Contributing

Bug reports and pull requests are welcome. The packet layouts live in
`app/src-tauri/src/decode.rs`, with comments on how each one was worked out.

To check a decoder change against a capture you recorded yourself:

```bash
cd app/src-tauri
cargo run --bin replay -- path/to/capture.pcapng
```

Packet captures are not in this repository, because they can contain player
names, chat, account data and IP addresses. If you share a capture in an issue, keep it short, and
look through it in Wireshark first.

## License

The code is released under the [MIT License](LICENSE). The game icons in `assets/` belong to the
owners of Drakensang Online and are not covered by it.
