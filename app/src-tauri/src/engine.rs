//! Packets in, meter updates out.

use crate::decode::{cast_skill, cast_tick, find_effect_hits, find_relayed_casts, find_hits, find_names, find_spawns, has_box_explosion, is_zone_load};
use crate::meter::Meter;
use crate::packet::parse_udp;
use crate::raknet::Reassembler;

#[derive(Default)]
pub struct Engine {
    rak: Reassembler,
    pub packets: u64,
    pub game_messages: u64,
}

impl Engine {
    /// Feed one captured frame. Returns true if it carried game traffic.
    pub fn on_frame(&mut self, meter: &mut Meter, datalink: i32, time: f64, frame: &[u8]) -> bool {
        let Some(udp) = parse_udp(datalink, frame) else { return false };
        self.packets += 1;
        let outgoing = udp.src.is_private() && !udp.dst.is_private();
        let peer = (u32::from(udp.src), udp.sport, u32::from(udp.dst), udp.dport);
        let msgs = self.rak.feed(peer, udp.payload);
        if msgs.is_empty() {
            return false;
        }
        for msg in msgs {
            self.game_messages += 1;
            if outgoing {
                if let (Some(skill), Some(tick)) = (cast_skill(&msg), cast_tick(&msg)) {
                    meter.on_cast(time, skill, tick);
                }
            } else {
                if is_zone_load(&msg) {
                    meter.on_zone_change();
                }
                for s in find_spawns(&msg) {
                    meter.on_spawn(s);
                }
                for p in find_names(&msg) {
                    meter.on_player(p);
                }
                for h in find_hits(&msg) {
                    meter.on_hit(time, h);
                }
                for h in find_effect_hits(&msg) {
                    meter.on_hit(time, h);
                }
                if has_box_explosion(&msg) {
                    meter.on_box_explosion(time);
                }
                for c in find_relayed_casts(&msg) {
                    meter.on_relayed_cast(time, c);
                }
            }
        }
        true
    }
}
