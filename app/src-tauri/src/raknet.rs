//! Turn RakNet UDP datagrams into whole game messages: split-packet reassembly and
//! dropping resent copies of reliable messages.

use std::collections::{HashMap, HashSet, VecDeque};

pub type Peer = (u32, u16, u32, u16);

const DEDUPE_WINDOW: usize = 4096;

#[derive(Default)]
pub struct Reassembler {
    splits: HashMap<(Peer, u16), HashMap<u32, Vec<u8>>>,
    seen: HashSet<(Peer, u32)>,
    seen_order: VecDeque<(Peer, u32)>,
}

fn is_reliable(r: u8) -> bool {
    matches!(r, 2 | 3 | 4 | 6 | 7)
}
fn is_sequenced(r: u8) -> bool {
    matches!(r, 1 | 4)
}
fn is_ordered(r: u8) -> bool {
    matches!(r, 1 | 3 | 4 | 7)
}

impl Reassembler {
    fn is_resend(&mut self, peer: Peer, rel: u32) -> bool {
        let key = (peer, rel);
        if !self.seen.insert(key) {
            return true;
        }
        self.seen_order.push_back(key);
        if self.seen_order.len() > DEDUPE_WINDOW {
            if let Some(old) = self.seen_order.pop_front() {
                self.seen.remove(&old);
            }
        }
        false
    }

    /// All complete game messages carried by one datagram.
    pub fn feed(&mut self, peer: Peer, data: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let Some(&flags) = data.first() else { return out };
        if flags & 0x80 == 0 || flags & 0x40 != 0 || flags & 0x20 != 0 {
            return out; // offline handshake, ACK or NAK
        }
        let n = data.len();
        let mut pos = 4; // flags + 24-bit datagram number
        while pos + 3 <= n {
            let mflags = data[pos];
            let bits = u16::from_be_bytes([data[pos + 1], data[pos + 2]]) as usize;
            pos += 3;
            let rel = mflags >> 5;
            let mut rel_no = None;
            if is_reliable(rel) {
                if pos + 3 > n {
                    break;
                }
                rel_no = Some(u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], 0]));
                pos += 3;
            }
            if is_sequenced(rel) {
                pos += 3;
            }
            if is_ordered(rel) {
                pos += 4;
            }
            let mut split = None;
            if mflags & 0x10 != 0 {
                if pos + 10 > n {
                    break;
                }
                let count = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
                let id = u16::from_be_bytes([data[pos + 4], data[pos + 5]]);
                let idx = u32::from_be_bytes(data[pos + 6..pos + 10].try_into().unwrap());
                pos += 10;
                split = Some((count, id, idx));
            }
            let size = bits.div_ceil(8);
            if pos > n {
                break;
            }
            let end = (pos + size).min(n);
            let payload = data[pos..end].to_vec();
            pos += size;
            if let Some(r) = rel_no {
                if self.is_resend(peer, r) {
                    continue;
                }
            }
            let payload = match split {
                None => payload,
                Some((count, id, idx)) => {
                    let parts = self.splits.entry((peer, id)).or_default();
                    parts.insert(idx, payload);
                    if (parts.len() as u32) < count {
                        continue;
                    }
                    let parts = self.splits.remove(&(peer, id)).unwrap();
                    let mut whole = Vec::new();
                    let mut complete = true;
                    for i in 0..count {
                        match parts.get(&i) {
                            Some(p) => whole.extend_from_slice(p),
                            None => complete = false,
                        }
                    }
                    if !complete {
                        continue;
                    }
                    whole
                }
            };
            if !payload.is_empty() {
                out.push(payload);
            }
        }
        out
    }
}
