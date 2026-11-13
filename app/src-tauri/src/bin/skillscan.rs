//! Debug: find skill IDs in server messages near a player's entity ID.
//!     cargo run --bin skillscan -- capture.pcapng <entity-hex> <lo-hex> <hi-hex>
//! Prints every S2C spot where a u16 in lo..=hi sits within 24 bytes of the entity ID.

use dso_dps_meter_lib::{decode, npcap::{Next, Npcap}, packet::parse_udp, raknet::Reassembler};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ent = u32::from_str_radix(&a[2], 16).unwrap().to_le_bytes();
    let lo = u16::from_str_radix(&a[3], 16).unwrap();
    let hi = u16::from_str_radix(&a[4], 16).unwrap();
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
                    if outgoing {
                        if decode::is_cast(&msg) {
                            println!("{time:.3} MYCAST {}", hex(&msg));
                        }
                        continue;
                    }
                    if msg.len() > 20000 {
                        continue;
                    }
                    if std::env::var("CASTREC").is_ok() {
                        let re = regex::bytes::Regex::new(r"(?s-u)(..\x01\x00)\xff([\x40-\x4f])\x00\x00\x00(..)(.{4})(..)").unwrap();
                        for k in 0..8 {
                            let x = decode::shifted(&msg, k);
                            for c in re.captures_iter(&x) {
                                let who = u32::from_le_bytes(c[1].try_into().unwrap());
                                let id = u16::from_le_bytes(c[3].try_into().unwrap());
                                let id2 = u16::from_le_bytes(c[5].try_into().unwrap());
                                let s0 = c.get(0).unwrap().start(); let pre = &x[s0.saturating_sub(40)..s0]; println!("{time:.3} REC sub={:02x} len={} k={k} who={who:08x} t={:02x} id={id:04x} then={} id2={id2:04x}\n   {} |", msg[1], msg.len(), c[2][0], hex(&c[4]), hex(pre)); if x.len() < 200 { println!("   FULL {}", hex(&x)); }
                            }
                        }
                        continue;
                    }
                    for k in 0..8 {
                        let x = decode::shifted(&msg, k);
                        for i in 0..x.len().saturating_sub(4) {
                            if x[i..i + 4] != ent {
                                continue;
                            }
                            let s = i.saturating_sub(24);
                            let e = (i + 28).min(x.len().saturating_sub(1));
                            for j in s..e {
                                let v = u16::from_le_bytes([x[j], x[j + 1]]);
                                if (lo..=hi).contains(&v) {
                                    let ps = j.saturating_sub(32);
                                    let pe = (j + 32).min(x.len());
                                    println!(
                                        "{time:.3} S2C {:02x} {:02x} len={} k={k} ent@{i} id={v:04x}@{j}\n   {} | {}",
                                        msg[0], msg[1], msg.len(), hex(&x[ps..j]), hex(&x[j..pe])
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Next::Timeout => {}
            Next::Eof => break,
        }
    }
}
