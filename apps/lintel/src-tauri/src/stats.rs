//! The numbers the bar shows: CPU, memory, network speed and connection, battery. A trimmed-down
//! version of Oriel's collector, from the same sources (the counters Task Manager uses).

use crate::ffi::*;
use crate::pdh::Query;
use serde::Serialize;
use std::ptr::null_mut;
use windows::Win32::Networking::NetworkListManager::{
    INetworkListManager, NetworkListManager, NLM_CONNECTIVITY_IPV4_INTERNET, NLM_CONNECTIVITY_IPV6_INTERNET, NLM_ENUM_NETWORK_CONNECTED,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

// Virtual adapters whose traffic is also counted on a physical one.
const SKIP_NICS: &[&str] = &["hyper-v virtual", "vethernet", "isatap", "teredo", "loopback", "pseudo-interface"];
/// Ticks between looking at the connection, which changes rarely.
const LINK_EVERY: u64 = 5;

#[derive(Serialize, Clone, Default)]
pub struct Stats {
    cpu: f64,
    mem: Mem,
    net: Net,
    battery: Option<Battery>,
    link: Link,
}

#[derive(Serialize, Clone, Default)]
struct Mem {
    used: u64,
    total: u64,
    pct: f64,
}

#[derive(Serialize, Clone, Default)]
struct Net {
    down: f64,
    up: f64,
}

#[derive(Serialize, Clone)]
pub struct Battery {
    pct: u8,
    plugged: bool,
    secs_left: Option<u32>,
}

/// How this PC is online.
#[derive(Serialize, Clone, Default, PartialEq, Debug)]
pub struct Link {
    /// "wifi", "wired", or "offline". Wired means anything that isn't Wi-Fi: Ethernet, a phone's
    /// USB tethering, a VPN on its own.
    kind: &'static str,
    /// Whether that connection reaches the internet, as Windows judges it.
    internet: bool,
    /// The Wi-Fi network's name, when on Wi-Fi.
    ssid: String,
    /// Its signal, 0–100. None when Windows won't say: since Windows 11 24H2, reading it needs
    /// location permission, which a menu bar shouldn't ask for.
    signal: Option<u8>,
}

pub struct Sampler {
    q: Query,
    wifi: Option<Wifi>,
    nlm: Option<INetworkListManager>,
    link: Link,
    tick: u64,
}

// The COM object is only ever used from the sampler thread that made it.
unsafe impl Send for Sampler {}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

impl Sampler {
    /// Call on the thread that will sample: the network list is COM.
    pub fn new() -> Self {
        unsafe { CoInitializeEx(null_mut(), COINIT_MULTITHREADED) };
        let mut q = Query::new().expect("PDH unavailable");
        q.add("cpu_util", r"\Processor Information(_Total)\% Processor Utility");
        q.add("net_down", r"\Network Interface(*)\Bytes Received/sec");
        q.add("net_up", r"\Network Interface(*)\Bytes Sent/sec");
        q.collect(); // baseline for rate counters
        let nlm = unsafe { CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).ok() };
        Sampler { q, wifi: Wifi::open(), nlm, link: Link::default(), tick: 0 }
    }

    pub fn sample(&mut self) -> Stats {
        self.q.collect();
        if self.tick.is_multiple_of(LINK_EVERY) {
            self.link = self.link();
        }
        self.tick += 1;
        Stats { cpu: round1(self.q.total("cpu_util").unwrap_or(0.0).clamp(0.0, 100.0)), mem: mem(), net: self.net(), battery: battery(), link: self.link.clone() }
    }

    fn net(&self) -> Net {
        let total = |key| {
            self.q
                .read(key)
                .iter()
                .filter(|(n, _)| {
                    let n = n.to_lowercase();
                    !SKIP_NICS.iter().any(|s| n.contains(s))
                })
                .map(|(_, v)| v)
                .sum()
        };
        Net { down: total("net_down"), up: total("net_up") }
    }

    fn link(&mut self) -> Link {
        let internet = self.nlm.as_ref().and_then(|n| unsafe { n.GetConnectivity().ok() }).map(|c| {
            c.0 & (NLM_CONNECTIVITY_IPV4_INTERNET.0 | NLM_CONNECTIVITY_IPV6_INTERNET.0) != 0
        });
        let connected = self.nlm.as_ref().and_then(|n| unsafe { n.IsConnected().ok() }).map(|b| b.as_bool());
        if let Some((ssid, signal)) = self.wifi.as_ref().and_then(Wifi::current) {
            // Without location permission there's no SSID either; the network list names the
            // network after it anyway (as the Wi-Fi flyout does), and needs no permission.
            let ssid = ssid.or_else(|| self.network_name()).unwrap_or_default();
            return Link { kind: "wifi", internet: internet.unwrap_or(true), ssid, signal };
        }
        if connected.unwrap_or(false) || internet.unwrap_or(false) {
            Link { kind: "wired", internet: internet.unwrap_or(false), ..Default::default() }
        } else {
            Link { kind: "offline", ..Default::default() }
        }
    }

    /// The name of the first connected network, as Windows' network list knows it.
    fn network_name(&self) -> Option<String> {
        unsafe {
            let list = self.nlm.as_ref()?.GetNetworks(NLM_ENUM_NETWORK_CONNECTED).ok()?;
            let mut one = [None];
            let mut fetched = 0u32;
            list.Next(&mut one, Some(&mut fetched)).ok()?;
            let name = one[0].take().filter(|_| fetched == 1)?.GetName().ok()?.to_string();
            (!name.is_empty()).then_some(name)
        }
    }
}

fn mem() -> Mem {
    let mut ms = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut ms) };
    let used = ms.ullTotalPhys.saturating_sub(ms.ullAvailPhys);
    Mem { used, total: ms.ullTotalPhys, pct: round1(used as f64 / ms.ullTotalPhys.max(1) as f64 * 100.0) }
}

pub fn battery() -> Option<Battery> {
    let mut s = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut s) } == 0 || s.BatteryFlag & 128 != 0 || s.BatteryFlag == 255 || s.BatteryLifePercent > 100 {
        return None;
    }
    Some(Battery { pct: s.BatteryLifePercent, plugged: s.ACLineStatus == 1, secs_left: (s.BatteryLifeTime != u32::MAX).then_some(s.BatteryLifeTime) })
}

/// The Wi-Fi service, for the network's name and signal. None on PCs without Wi-Fi.
struct Wifi(HANDLE);

impl Wifi {
    fn open() -> Option<Self> {
        let (mut version, mut h) = (0u32, null_mut());
        (unsafe { WlanOpenHandle(2, null_mut(), &mut version, &mut h) } == 0).then_some(Wifi(h))
    }

    /// None unless a Wi-Fi adapter is connected; then its network's name and signal, each if
    /// Windows lets us read them (see `Link::signal`).
    fn current(&self) -> Option<(Option<String>, Option<u8>)> {
        unsafe {
            let mut list = null_mut();
            if WlanEnumInterfaces(self.0, null_mut(), &mut list) != 0 || list.is_null() {
                return None;
            }
            let n = (*list).dwNumberOfItems as usize;
            let items = std::slice::from_raw_parts((*list).InterfaceInfo.as_ptr(), n);
            let mut found = None;
            for i in items.iter().filter(|i| i.isState == WLAN_INTERFACE_STATE_CONNECTED) {
                found = Some((None, None));
                let (mut size, mut data) = (0u32, null_mut());
                // Refused (access denied) without location permission.
                if WlanQueryInterface(self.0, &i.InterfaceGuid, WLAN_INTF_OPCODE_CURRENT_CONNECTION, null_mut(), &mut size, &mut data, null_mut()) != 0 {
                    continue;
                }
                if size as usize >= std::mem::size_of::<WLAN_CONNECTION_ATTRIBUTES>() {
                    let a = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
                    let len = (a.uSSIDLength as usize).min(32);
                    found = Some((Some(String::from_utf8_lossy(&a.ucSSID[..len]).into_owned()), Some(a.wlanSignalQuality.min(100) as u8)));
                }
                WlanFreeMemory(data);
                break;
            }
            WlanFreeMemory(list.cast());
            found
        }
    }
}

// The handle is only used from the sampler thread.
unsafe impl Send for Wifi {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wlan_structs_match_windows() {
        // Offsets from wlanapi.h; a mismatch would read the wrong bytes as the network's name.
        assert_eq!(std::mem::size_of::<WLAN_INTERFACE_INFO>(), 532);
        assert_eq!(std::mem::offset_of!(WLAN_CONNECTION_ATTRIBUTES, ucSSID), 524);
        assert_eq!(std::mem::offset_of!(WLAN_CONNECTION_ATTRIBUTES, wlanSignalQuality), 576);
        assert_eq!(std::mem::offset_of!(WLAN_INTERFACE_INFO_LIST, InterfaceInfo), 8);
    }

    #[test]
    fn samples_this_pc() {
        let mut s = Sampler::new();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let st = s.sample();
        assert!((0.0..=100.0).contains(&st.cpu));
        assert!(st.mem.total > 0 && st.mem.used <= st.mem.total);
        println!("cpu {}% mem {}% link {:?}", st.cpu, st.mem.pct, st.link);
    }
}
