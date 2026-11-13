//! Debug: print every server message in a time window.
//!     msgdump <capture> <from-unix-time> <to-unix-time> [max-bytes]
//! One line per message: time, sub-message type, length, then the bytes (bit shift 0).

use dso_dps_meter_lib::{npcap::{Next, Npcap}, packet::parse_udp, raknet::Reassembler};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let from: f64 = a[2].parse().unwrap();
    let to: f64 = a[3].parse().unwrap();
    let max: usize = a.get(4).map(|s| s.parse().unwrap()).unwrap_or(160);
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
                    if time < from || time > to || msg.len() < 2 {
                        continue;
                    }
                    // Optional 5th arg: an entity id (hex); print where it appears at every bit shift.
                    if let Some(ent) = a.get(5) {
                        let ent = u32::from_str_radix(ent, 16).unwrap().to_le_bytes();
                        for k in 0..8 {
                            let x = dso_dps_meter_lib::decode::shifted(&msg, k);
                            for i in 0..x.len().saturating_sub(4) {
                                if x[i..i + 4] == ent {
                                    let s = i.saturating_sub(48);
                                    let e = (i + 24).min(x.len());
                                    let h: Vec<String> = x[s..e].iter().map(|b| format!("{b:02x}")).collect();
                                    println!("{time:.3} {:02x} len={} k={k} off={i}  {}", msg[1], msg.len(), h.join(" "));
                                }
                            }
                        }
                        continue;
                    }
                    let dir = if outgoing { "C2S" } else { "S2C" };
                    let hex: Vec<String> = msg[..msg.len().min(max)].iter().map(|b| format!("{b:02x}")).collect();
                    println!("{time:.3} {dir} {:02x} {:02x} len={} {}", msg[0], msg[1], msg.len(), hex.join(" "));
                }
            }
            Next::Timeout => continue,
            Next::Eof => break,
        }
    }
}
