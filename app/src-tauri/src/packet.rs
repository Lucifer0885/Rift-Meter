//! Pull IPv4/UDP out of a captured frame.

use std::net::Ipv4Addr;

pub struct Udp<'a> {
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
    pub sport: u16,
    pub dport: u16,
    pub payload: &'a [u8],
}

const DLT_NULL: i32 = 0;
const DLT_EN10MB: i32 = 1;

pub fn parse_udp(datalink: i32, frame: &[u8]) -> Option<Udp<'_>> {
    let ip = match datalink {
        DLT_EN10MB => {
            let mut off = 12;
            let mut ethertype = u16::from_be_bytes([*frame.get(off)?, *frame.get(off + 1)?]);
            if ethertype == 0x8100 {
                off += 4; // VLAN tag
                ethertype = u16::from_be_bytes([*frame.get(off)?, *frame.get(off + 1)?]);
            }
            if ethertype != 0x0800 {
                return None;
            }
            frame.get(off + 2..)?
        }
        DLT_NULL => frame.get(4..)?,
        _ => return None,
    };
    if ip.len() < 20 || ip[0] >> 4 != 4 || ip[9] != 17 {
        return None;
    }
    let ihl = (ip[0] & 0x0f) as usize * 4;
    let total = u16::from_be_bytes([ip[2], ip[3]]) as usize;
    let udp = ip.get(ihl..total.min(ip.len()))?;
    if udp.len() < 8 {
        return None;
    }
    let ulen = (u16::from_be_bytes([udp[4], udp[5]]) as usize).clamp(8, udp.len());
    Some(Udp {
        src: Ipv4Addr::new(ip[12], ip[13], ip[14], ip[15]),
        dst: Ipv4Addr::new(ip[16], ip[17], ip[18], ip[19]),
        sport: u16::from_be_bytes([udp[0], udp[1]]),
        dport: u16::from_be_bytes([udp[2], udp[3]]),
        payload: &udp[8..ulen],
    })
}
