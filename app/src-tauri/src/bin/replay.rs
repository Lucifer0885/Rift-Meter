//! Replay a saved capture through the decoder and print every hit.
//!
//!     cargo run --bin replay -- path\to\capture.pcapng

use dso_dps_meter_lib::{engine::Engine, meter::Meter, npcap::{Next, Npcap}};

fn main() {
    let path = std::env::args().nth(1).expect("usage: replay <capture.pcapng>");
    let npcap = Npcap::load().expect("load npcap");
    let mut cap = npcap.open_file(&path).expect("open capture");
    let mut engine = Engine::default();
    let mut meter = Meter::new();
    let mut last_time = 0.0;
    let mut printed = 0u64;
    let dl = cap.datalink;
    loop {
        // A capture stopped mid-write ends in a truncated block: treat it as the end.
        let next = match cap.next() {
            Ok(n) => n,
            Err(e) => {
                eprintln!("stopped reading: {e}");
                Next::Eof
            }
        };
        match next {
            Next::Packet { time, data } => {
                engine.on_frame(&mut meter, dl, time, data);
                last_time = time;
                let new = (meter.hits_seen - printed) as usize;
                if new == 0 {
                    continue;
                }
                let snap = meter.snapshot(time);
                for h in snap.recent_hits.iter().take(new).rev() {
                    println!(
                        "{:.2} {} {:>12}  from {:08x} (owner {:08x}) -> {:08x}  {} [{}]",
                        h.time,
                        if h.crit { "CRIT" } else { "    " },
                        h.damage,
                        h.attacker,
                        h.owner,
                        h.target,
                        h.target_name,
                        h.target_rank
                    );
                }
                printed = meter.hits_seen;
            }
            Next::Timeout => {}
            Next::Eof => break,
        }
    }
    // SAVE_DIR=<folder>: save the fights like the app does, then list them back.
    if let Ok(dir) = std::env::var("SAVE_DIR") {
        let dir = std::path::PathBuf::from(dir);
        meter.end_fight();
        for f in meter.take_finished(last_time + 1e9) {
            match dso_dps_meter_lib::history::save(&dir, &f) {
                Ok(p) => println!("saved {}", p.display()),
                Err(e) => println!("save failed: {e}"),
            }
        }
        for f in dso_dps_meter_lib::history::list(&dir) {
            println!("listed {} {:.1}s total={} killed={:?} attackers={}", f.file, f.fight.duration, f.fight.total, f.fight.killed, f.fight.attackers.len());
        }
    }
    let live = meter.snapshot(last_time);
    println!("\ntracked targets at the end of the capture:");
    for t in &live.tracked_targets {
        println!(
            "  {:08x} {:<24} [{}] hp {}/{} ({:.1}%) dead={} dps={:.0} ttk={:?}",
            t.id, t.name, t.rank, t.hp, t.max_hp, t.pct * 100.0, t.dead, t.dps, t.time_to_kill.map(|s| s.round())
        );
    }
    println!("focus: {:?}", live.focus_target.as_ref().map(|t| &t.name));
    println!("player: {:?}", live.player);
    let snap = meter.snapshot(last_time + 1e9);
    println!(
        "\npackets={} game_messages={} entities={} me={:?}",
        engine.packets, engine.game_messages, snap.known_entities, snap.me.map(|m| format!("{m:08x}"))
    );
    for e in snap.encounter.iter().chain(snap.history.iter()) {
        println!("encounter {:.1}s total={} killed={:?}", e.duration, e.total, e.killed);
        for a in &e.attackers {
            println!(
                "  {:<12} dmg={:>12} dps={:>10.0} hits={} crit={:.0}% max={} class={:?} lvl={:?}",
                a.name, a.damage, a.dps, a.hits, a.crit_rate * 100.0, a.max_hit, a.class, a.level
            );
            for s in &a.sources {
                let el: Vec<String> = s.elements.iter().map(|e| format!("{}={}", e.name, e.damage)).collect();
                println!("      {:<22} casts={:>3} dmg={:>12} hits={:>4} crit={:.0}% share={:.1}% {}", s.name, s.casts, s.damage, s.hits, s.crit_rate * 100.0, s.share * 100.0, el.join(" "));
            }
        }
        for t in &e.targets {
            println!("    vs {} [{}] {}", t.name, t.rank, t.damage);
        }
    }
}
