//! Minimal Npcap bindings, loaded at runtime from `wpcap.dll`.
//!
//! Loading the DLL at runtime means no Npcap SDK is needed to build, and the app shows a
//! clear error instead of failing to start when Npcap is missing.

use libloading::Library;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::net::{Ipv4Addr, UdpSocket};
use std::sync::Arc;

#[repr(C)]
struct PcapIf {
    next: *mut PcapIf,
    name: *mut c_char,
    description: *mut c_char,
    addresses: *mut PcapAddr,
    flags: u32,
}

#[repr(C)]
struct PcapAddr {
    next: *mut PcapAddr,
    addr: *mut SockAddr,
    netmask: *mut SockAddr,
    broadaddr: *mut SockAddr,
    dstaddr: *mut SockAddr,
}

#[repr(C)]
struct SockAddr {
    family: u16,
    data: [u8; 14],
}

#[repr(C)]
struct BpfProgram {
    len: u32,
    insns: *mut c_void,
}

/// `struct pcap_pkthdr` on Windows (`long` is 32-bit there).
#[repr(C)]
struct PktHdr {
    ts_sec: i32,
    ts_usec: i32,
    caplen: u32,
    len: u32,
}

const AF_INET: u16 = 2;
const ERRBUF: usize = 256;

type FindAllDevs = unsafe extern "C" fn(*mut *mut PcapIf, *mut c_char) -> c_int;
type FreeAllDevs = unsafe extern "C" fn(*mut PcapIf);
type OpenLive = unsafe extern "C" fn(*const c_char, c_int, c_int, c_int, *mut c_char) -> *mut c_void;
type OpenOffline = unsafe extern "C" fn(*const c_char, *mut c_char) -> *mut c_void;
type Compile = unsafe extern "C" fn(*mut c_void, *mut BpfProgram, *const c_char, c_int, u32) -> c_int;
type SetFilter = unsafe extern "C" fn(*mut c_void, *mut BpfProgram) -> c_int;
type FreeCode = unsafe extern "C" fn(*mut BpfProgram);
type NextEx = unsafe extern "C" fn(*mut c_void, *mut *mut PktHdr, *mut *const u8) -> c_int;
type Close = unsafe extern "C" fn(*mut c_void);
type Datalink = unsafe extern "C" fn(*mut c_void) -> c_int;
type GetErr = unsafe extern "C" fn(*mut c_void) -> *mut c_char;

pub struct Npcap {
    _lib: Library,
    findalldevs: FindAllDevs,
    freealldevs: FreeAllDevs,
    open_live: OpenLive,
    open_offline: OpenOffline,
    compile: Compile,
    setfilter: SetFilter,
    freecode: FreeCode,
    next_ex: NextEx,
    close: Close,
    datalink: Datalink,
    geterr: GetErr,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Device {
    pub name: String,
    pub description: String,
    pub ipv4: Vec<String>,
}

pub enum Next<'a> {
    Packet { time: f64, data: &'a [u8] },
    Timeout,
    Eof,
}

pub struct Capture {
    npcap: Arc<Npcap>,
    handle: *mut c_void,
    pub datalink: i32,
}

// The handle is only ever used from the thread that owns the Capture.
unsafe impl Send for Capture {}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

impl Npcap {
    pub fn load() -> Result<Arc<Npcap>, String> {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let path = format!("{root}\\System32\\Npcap\\wpcap.dll");
        // LOAD_WITH_ALTERED_SEARCH_PATH so wpcap.dll finds Packet.dll next to it.
        let lib: Library = unsafe {
            libloading::os::windows::Library::load_with_flags(
                &path,
                libloading::os::windows::LOAD_WITH_ALTERED_SEARCH_PATH,
            )
        }
        .map_err(|e| format!("Npcap not found ({path}): {e}. Install Npcap from https://npcap.com"))?
        .into();
        unsafe {
            macro_rules! sym {
                ($name:literal) => {
                    *lib.get($name).map_err(|e| format!("wpcap.dll is missing a function: {e}"))?
                };
            }
            Ok(Arc::new(Npcap {
                findalldevs: sym!(b"pcap_findalldevs\0"),
                freealldevs: sym!(b"pcap_freealldevs\0"),
                open_live: sym!(b"pcap_open_live\0"),
                open_offline: sym!(b"pcap_open_offline\0"),
                compile: sym!(b"pcap_compile\0"),
                setfilter: sym!(b"pcap_setfilter\0"),
                freecode: sym!(b"pcap_freecode\0"),
                next_ex: sym!(b"pcap_next_ex\0"),
                close: sym!(b"pcap_close\0"),
                datalink: sym!(b"pcap_datalink\0"),
                geterr: sym!(b"pcap_geterr\0"),
                _lib: lib,
            }))
        }
    }

    pub fn devices(&self) -> Result<Vec<Device>, String> {
        let mut err = [0 as c_char; ERRBUF];
        let mut head: *mut PcapIf = std::ptr::null_mut();
        if unsafe { (self.findalldevs)(&mut head, err.as_mut_ptr()) } != 0 {
            return Err(cstr(err.as_ptr()));
        }
        let mut out = Vec::new();
        let mut cur = head;
        while !cur.is_null() {
            let dev = unsafe { &*cur };
            let mut ipv4 = Vec::new();
            let mut a = dev.addresses;
            while !a.is_null() {
                let addr = unsafe { &*a };
                if !addr.addr.is_null() {
                    let sa = unsafe { &*addr.addr };
                    if sa.family == AF_INET {
                        ipv4.push(Ipv4Addr::new(sa.data[2], sa.data[3], sa.data[4], sa.data[5]).to_string());
                    }
                }
                a = addr.next;
            }
            out.push(Device { name: cstr(dev.name), description: cstr(dev.description), ipv4 });
            cur = dev.next;
        }
        unsafe { (self.freealldevs)(head) };
        Ok(out)
    }

    /// The adapter Windows would use to reach the internet (matched by local IPv4).
    pub fn default_device(&self) -> Option<Device> {
        let local = UdpSocket::bind("0.0.0.0:0")
            .and_then(|s| s.connect("8.8.8.8:53").map(|_| s))
            .and_then(|s| s.local_addr())
            .ok()?
            .ip()
            .to_string();
        self.devices().ok()?.into_iter().find(|d| d.ipv4.contains(&local))
    }

    pub fn open_live(self: &Arc<Self>, device: &str, filter: &str) -> Result<Capture, String> {
        let mut err = [0 as c_char; ERRBUF];
        let name = CString::new(device).map_err(|e| e.to_string())?;
        // snaplen 65535, not promiscuous, 100 ms read timeout so stop requests are noticed
        let h = unsafe { (self.open_live)(name.as_ptr(), 65535, 0, 100, err.as_mut_ptr()) };
        if h.is_null() {
            return Err(cstr(err.as_ptr()));
        }
        let cap = Capture { npcap: self.clone(), handle: h, datalink: unsafe { (self.datalink)(h) } };
        cap.set_filter(filter)?;
        Ok(cap)
    }

    pub fn open_file(self: &Arc<Self>, path: &str) -> Result<Capture, String> {
        let mut err = [0 as c_char; ERRBUF];
        let p = CString::new(path).map_err(|e| e.to_string())?;
        let h = unsafe { (self.open_offline)(p.as_ptr(), err.as_mut_ptr()) };
        if h.is_null() {
            return Err(cstr(err.as_ptr()));
        }
        Ok(Capture { npcap: self.clone(), handle: h, datalink: unsafe { (self.datalink)(h) } })
    }
}

impl Capture {
    fn set_filter(&self, filter: &str) -> Result<(), String> {
        let f = CString::new(filter).map_err(|e| e.to_string())?;
        let mut prog = BpfProgram { len: 0, insns: std::ptr::null_mut() };
        let n = &self.npcap;
        unsafe {
            if (n.compile)(self.handle, &mut prog, f.as_ptr(), 1, 0xffff_ffff) != 0 {
                return Err(format!("bad capture filter: {}", cstr((n.geterr)(self.handle))));
            }
            let r = (n.setfilter)(self.handle, &mut prog);
            (n.freecode)(&mut prog);
            if r != 0 {
                return Err(format!("could not set filter: {}", cstr((n.geterr)(self.handle))));
            }
        }
        Ok(())
    }

    pub fn next(&mut self) -> Result<Next<'_>, String> {
        let mut hdr: *mut PktHdr = std::ptr::null_mut();
        let mut data: *const u8 = std::ptr::null();
        let r = unsafe { (self.npcap.next_ex)(self.handle, &mut hdr, &mut data) };
        match r {
            1 => {
                let h = unsafe { &*hdr };
                let bytes = unsafe { std::slice::from_raw_parts(data, h.caplen as usize) };
                let time = h.ts_sec as u32 as f64 + h.ts_usec as f64 / 1e6;
                Ok(Next::Packet { time, data: bytes })
            }
            0 => Ok(Next::Timeout),
            -2 => Ok(Next::Eof),
            _ => Err(cstr(unsafe { (self.npcap.geterr)(self.handle) })),
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe { (self.npcap.close)(self.handle) };
    }
}
