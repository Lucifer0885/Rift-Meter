//! Debug: where the local player shows up outside combat.
//!     whoami <capture> [name]
//! Prints zone loads, every player-info record, and (with a name) every message holding that name.

use dso_dps_meter_lib::{decode, npcap::{Next, Npcap}, packet::parse_udp, raknet::Reassembler};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let needle = a.get(2).map(|s| s.as_bytes().to_vec());
    let npcap = Npcap::load().unwrap();
    let mut cap = npcap.open_file(&a[1]).unwrap();
    let dl = cap.datalink;
    let mut rak = Reassembler::default();
    loop {
        match cap.next().unwrap() {
            Next::Packet { time, data } => {
                let Some(udp) = parse_udp(dl, data) else { continue };
                let outgoing = udp.src.is_private() && !udp.dst.is_private();
                let peer = (u32::from(udp.src), udp.sport, u32::from(udp.dst), udp.dport);
                for msg in rak.feed(peer, udp.payload) {
                    if msg.len() < 2 {
                        continue;
                    }
                    let dir = if outgoing { "C2S" } else { "S2C" };
                    if decode::is_zone_load(&msg) {
                        println!("{time:.3} ZONE_LOAD len={} player={:?}", msg.len(), decode::zone_load_player(&msg).map(|(n, c, id)| (n, c, id.map(|i| format!("{i:08x}")))));
                    }
                    for p in decode::find_names(&msg) {
                        println!("{time:.3} NAME {:08x} {} {:?} {:?} sub={:02x} len={}", p.id, p.name, p.class, p.level, msg[1], msg.len());
                    }
                    if let Some(n) = &needle {
                        for k in 0..8 {
                            let x = decode::shifted(&msg, k);
                            let mut i = 0;
                            while i + n.len() <= x.len() {
                                if &x[i..i + n.len()] == n.as_slice() {
                                    let s = i.saturating_sub(24);
                                    let e = (i + n.len() + 24).min(x.len());
                                    let h: Vec<String> = x[s..e].iter().map(|b| format!("{b:02x}")).collect();
                                    println!("{time:.3} {dir} HAS sub={:02x} len={} k={k} off={i} {}", msg[1], msg.len(), h.join(" "));
                                }
                                i += 1;
                            }
                        }
                    }
                }
            }
            Next::Timeout => continue,
            Next::Eof => break,
        }
    }
}
