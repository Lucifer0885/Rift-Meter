//! Packets in, meter updates out.

use crate::decode::{cast_skill, cast_tick, find_effect_hits, find_relayed_casts, find_hits, find_names, find_spawns, find_effect_markers, has_box_explosion, is_zone_load};
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
                    meter.on_spawn(time, s);
                }
                for p in find_names(&msg) {
                    meter.on_player(p);
                }
                // Effect auras first: a damage record can share their message.
                if has_box_explosion(&msg) {
                    meter.on_box_explosion(time);
                }
                for source in find_effect_markers(&msg) {
                    meter.on_effect_marker(time, source);
                }
                for h in find_hits(&msg) {
                    meter.on_hit(time, h);
                }
                // Area records (they name an owner) first: Explosive Support's Fire record is
                // matched by damage to the area record of the same bomb.
                let (area, tagged): (Vec<_>, Vec<_>) = find_effect_hits(&msg).into_iter().partition(|h| h.attacker != 0);
                for h in area.into_iter().chain(tagged) {
                    meter.on_hit(time, h);
                }
                for c in find_relayed_casts(&msg) {
                    meter.on_relayed_cast(time, c);
                }
            }
        }
        true
    }
}
