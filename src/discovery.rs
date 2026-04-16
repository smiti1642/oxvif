//! WS-Discovery — UDP multicast device probe.
//!
//! Sends a WS-Discovery `Probe` message to the ONVIF multicast address
//! (`239.255.255.250:3702`) and collects `ProbeMatch` responses until
//! `timeout_dur` elapses.
//!
//! # Example
//!
//! ```no_run
//! use std::time::Duration;
//! use oxvif::discovery;
//!
//! #[tokio::main]
//! async fn main() {
//!     let devices = discovery::probe(Duration::from_secs(3)).await;
//!     for d in &devices {
//!         println!("{}", d.xaddrs.first().map(String::as_str).unwrap_or("(no address)"));
//!     }
//! }
//! ```

use std::collections::HashSet;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::time::{Instant, timeout};

use crate::soap::XmlNode;

// ── Constants ─────────────────────────────────────────────────────────────────

const WSD_MULTICAST: &str = "239.255.255.250:3702";
const WSD_MULTICAST_ADDR: std::net::Ipv4Addr = std::net::Ipv4Addr::new(239, 255, 255, 250);
/// Maximum UDP datagram size (IPv4 theoretical maximum).
const UDP_MAX_SIZE: usize = 65_535;

// ── DiscoveredDevice ──────────────────────────────────────────────────────────

/// A device found via WS-Discovery.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    /// Unique endpoint address (typically a `uuid:…` URN).
    pub endpoint: String,
    /// Advertised WS-Discovery types (e.g. `NetworkVideoTransmitter`).
    pub types: Vec<String>,
    /// ONVIF scopes (e.g. `onvif://www.onvif.org/name/Camera1`).
    pub scopes: Vec<String>,
    /// Device service URLs. Pass the first entry to [`OnvifClient::new`].
    ///
    /// [`OnvifClient::new`]: crate::client::OnvifClient::new
    pub xaddrs: Vec<String>,
}

impl DiscoveredDevice {
    fn from_xml(node: &XmlNode) -> Self {
        let endpoint = node
            .path(&["EndpointReference", "Address"])
            .map(|n| n.text().to_string())
            .unwrap_or_default();

        let types = node
            .child("Types")
            .map(|n| n.text().split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();

        let scopes = node
            .child("Scopes")
            .map(|n| n.text().split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();

        let xaddrs = node
            .child("XAddrs")
            .map(|n| n.text().split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();

        Self {
            endpoint,
            types,
            scopes,
            xaddrs,
        }
    }
}

// ── DiscoveryEvent ────────────────────────────────────────────────────────────

/// An unsolicited WS-Discovery announcement received while listening on the
/// multicast port.
///
/// Devices broadcast `Hello` on arrival and `Bye` on departure.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// A device has announced itself on the network.
    Hello(DiscoveredDevice),
    /// A device has left the network.
    Bye {
        /// Unique endpoint address (typically `uuid:…` URN) of the departing device.
        endpoint: String,
    },
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Send a WS-Discovery `Probe` and collect all `ProbeMatch` responses.
///
/// Binds to every non-loopback IPv4 interface (plus `0.0.0.0` as a
/// catch-all), sends a `Probe` to the ONVIF multicast group
/// (`239.255.255.250:3702`) on each, and returns every device that replies
/// within `timeout_dur`. Duplicate responses (same endpoint UUID) are
/// suppressed.
///
/// UDP multicast is lossy — a single probe can miss devices that are busy
/// or that drop the first packet. Call [`probe_rounds`] instead when you
/// need higher reliability.
///
/// Returns an empty `Vec` on any I/O error — treat failures as "no devices
/// found" rather than hard errors.
pub async fn probe(timeout_dur: Duration) -> Vec<DiscoveredDevice> {
    probe_inner(1, timeout_dur, Duration::ZERO, WSD_MULTICAST)
        .await
        .unwrap_or_default()
}

/// Send multiple `Probe` rounds and collect deduplicated `ProbeMatch`
/// responses.
///
/// Each round does what [`probe`] does — send a Probe on every non-loopback
/// IPv4 interface and listen for `timeout_per_round`. Between rounds the
/// task sleeps for `interval`. Devices found in earlier rounds are not
/// re-reported.
///
/// Multiple rounds improve reliability on busy networks or against cameras
/// that drop the first probe. A typical configuration is `3` rounds,
/// `2s` per round, `800ms` interval (≈ 6.4s total).
///
/// - `rounds = 0` returns an empty `Vec` without any I/O.
/// - `rounds = 1` is equivalent to [`probe`] (`interval` is ignored).
///
/// Returns an empty `Vec` on any I/O error — treat failures as "no devices
/// found" rather than hard errors.
///
/// # Example
///
/// ```no_run
/// use std::time::Duration;
/// use oxvif::discovery;
///
/// #[tokio::main]
/// async fn main() {
///     let devices = discovery::probe_rounds(
///         3,
///         Duration::from_secs(2),
///         Duration::from_millis(800),
///     ).await;
///     for d in &devices {
///         println!("{}", d.xaddrs.first().map(String::as_str).unwrap_or("(no address)"));
///     }
/// }
/// ```
pub async fn probe_rounds(
    rounds: usize,
    timeout_per_round: Duration,
    interval: Duration,
) -> Vec<DiscoveredDevice> {
    probe_inner(rounds, timeout_per_round, interval, WSD_MULTICAST)
        .await
        .unwrap_or_default()
}

/// Listen passively for WS-Discovery `Hello` and `Bye` multicast announcements.
///
/// Binds to UDP port 3702 (the WS-Discovery multicast port), joins the ONVIF
/// multicast group (`239.255.255.250`), and collects `Hello` / `Bye` datagrams
/// for `timeout_dur`.
///
/// Returns an empty `Vec` on any I/O error (e.g. port 3702 already in use).
///
/// # Example
///
/// ```no_run
/// use std::time::Duration;
/// use oxvif::discovery;
///
/// #[tokio::main]
/// async fn main() {
///     let events = discovery::listen(Duration::from_secs(30)).await;
///     for ev in &events {
///         println!("{ev:?}");
///     }
/// }
/// ```
pub async fn listen(timeout_dur: Duration) -> Vec<DiscoveryEvent> {
    listen_inner(timeout_dur).await.unwrap_or_default()
}

// ── Internal implementation ───────────────────────────────────────────────────

async fn probe_inner(
    rounds: usize,
    timeout_per_round: Duration,
    interval: Duration,
    target: &str,
) -> std::io::Result<Vec<DiscoveredDevice>> {
    let mut devices: Vec<DiscoveredDevice> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for round in 0..rounds {
        if round > 0 && !interval.is_zero() {
            tokio::time::sleep(interval).await;
        }

        let raw = probe_once(timeout_per_round, target).await?;
        for data in raw {
            let Ok(text) = std::str::from_utf8(&data) else {
                continue;
            };
            // Fast path: strict DOM parse. Most compliant cameras land here.
            //
            // Slow path: if the datagram contains malformed XML (unescaped
            // ampersands in scope URIs, unclosed tags, wrong-encoded CJK
            // text, etc.) `XmlNode::parse` returns `Err` and we'd otherwise
            // drop the whole ProbeMatch. Fall back to a tolerant string
            // scanner — matches how the reference Java/C# WS-Discovery
            // clients (and ODM) handle wire-level noise.
            let matches = match XmlNode::parse(text) {
                Ok(root) => collect_probe_matches(&root),
                Err(_) => collect_probe_matches_lenient(text),
            };
            for d in matches {
                if seen.insert(d.endpoint.clone()) {
                    devices.push(d);
                }
            }
        }
    }

    Ok(devices)
}

/// Send one round of Probes across every non-loopback IPv4 interface and
/// collect the raw response datagrams until `timeout_dur` elapses.
///
/// Extracted so multi-round callers can loop this while keeping a single
/// cross-round dedup set.
async fn probe_once(timeout_dur: Duration, target: &str) -> std::io::Result<Vec<Vec<u8>>> {
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::sync::{Arc, Mutex};

    // Two separate Probes per socket — one for NetworkVideoTransmitter,
    // one for Device. WS-Discovery's <Types> is an AND match, so a single
    // probe filtered on NetworkVideoTransmitter silently skips every
    // device that advertises only Device (NVRs, doorbells, some Profile T
    // encoders, etc.).  Matches ONVIF Device Manager's behaviour — without
    // the second probe a mixed company LAN can under-report by 30–40%.
    let nvt_probe = Arc::new(build_probe(
        &new_uuid(),
        ProbeTarget::NetworkVideoTransmitter,
    ));
    let device_probe = Arc::new(build_probe(&new_uuid(), ProbeTarget::Device));

    // Send a Probe from every non-loopback IPv4 interface so cameras on any
    // subnet receive it.  0.0.0.0 is always included first as a catch-all
    // (also lets loopback targets work in tests).
    let bind_ips: Vec<Ipv4Addr> = std::iter::once(Ipv4Addr::UNSPECIFIED)
        .chain(local_ipv4_addrs())
        .collect();

    // Raw datagrams collected by per-interface listener tasks.
    let received: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for ip in bind_ips {
        // Use socket2 to set IP_MULTICAST_IF before converting to tokio.
        // Neither std::net::UdpSocket nor tokio::net::UdpSocket expose this
        // option directly, but without it Windows routes the multicast probe
        // through its default multicast interface (often Hyper-V or WSL
        // virtual adapters) even when the socket is bound to a specific IP.
        use socket2::{Domain, Protocol, Socket, Type};
        let Ok(raw) = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)) else {
            continue;
        };
        let addr: std::net::SocketAddr = SocketAddrV4::new(ip, 0).into();
        if raw.bind(&addr.into()).is_err() {
            continue;
        }
        let _ = raw.set_multicast_ttl_v4(4);
        if ip != Ipv4Addr::UNSPECIFIED {
            let _ = raw.set_multicast_if_v4(&ip);
        }
        let _ = raw.set_nonblocking(true);
        let Ok(sock) = UdpSocket::from_std(raw.into()) else {
            continue;
        };
        let _ = sock.send_to(nvt_probe.as_bytes(), target).await;
        let _ = sock.send_to(device_probe.as_bytes(), target).await;

        let received = Arc::clone(&received);
        let handle = tokio::task::spawn(async move {
            let mut buf = vec![0u8; UDP_MAX_SIZE];
            let deadline = Instant::now() + timeout_dur;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match timeout(remaining, sock.recv_from(&mut buf)).await {
                    Ok(Ok((len, _))) => {
                        // Recover from mutex poison (another listener task panicked)
                        // rather than propagating the panic across all listeners.
                        received
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push(buf[..len].to_vec());
                    }
                    Ok(Err(_)) => continue, // WSAECONNRESET / transient error — keep waiting
                    Err(_) => break,        // timeout elapsed
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.await;
    }

    let raw = Arc::try_unwrap(received)
        .unwrap_or_default()
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());

    Ok(raw)
}

/// Returns all non-loopback IPv4 addresses assigned to local interfaces.
fn local_ipv4_addrs() -> Vec<std::net::Ipv4Addr> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|iface| {
            if iface.is_loopback() {
                return None;
            }
            match iface.addr {
                if_addrs::IfAddr::V4(addr) => Some(addr.ip),
                _ => None,
            }
        })
        .collect()
}

async fn listen_inner(timeout_dur: Duration) -> std::io::Result<Vec<DiscoveryEvent>> {
    use std::net::Ipv4Addr;

    let socket = UdpSocket::bind("0.0.0.0:3702").await?;
    socket.join_multicast_v4(WSD_MULTICAST_ADDR, Ipv4Addr::UNSPECIFIED)?;

    let mut buf = vec![0u8; UDP_MAX_SIZE];
    let mut events: Vec<DiscoveryEvent> = Vec::new();
    let deadline = Instant::now() + timeout_dur;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, _addr))) => {
                let Ok(text) = std::str::from_utf8(&buf[..len]) else {
                    continue;
                };
                if let Ok(root) = XmlNode::parse(text) {
                    events.extend(collect_discovery_events(&root));
                }
            }
            _ => break,
        }
    }
    Ok(events)
}

fn collect_discovery_events(root: &XmlNode) -> Vec<DiscoveryEvent> {
    // Determine message type from the WS-Addressing Action header.
    let action = root
        .path(&["Header", "Action"])
        .map(|n| n.text())
        .unwrap_or("");

    let body = root.child("Body").unwrap_or(root);

    if action.ends_with("/Hello") {
        if let Some(hello) = body.child("Hello") {
            return vec![DiscoveryEvent::Hello(DiscoveredDevice::from_xml(hello))];
        }
    } else if action.ends_with("/Bye") {
        if let Some(bye) = body.child("Bye") {
            let endpoint = bye
                .path(&["EndpointReference", "Address"])
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            return vec![DiscoveryEvent::Bye { endpoint }];
        }
    }
    vec![]
}

fn collect_probe_matches(root: &XmlNode) -> Vec<DiscoveredDevice> {
    let body = root.child("Body").unwrap_or(root);
    let matches = body.child("ProbeMatches").unwrap_or(body);
    matches
        .children_named("ProbeMatch")
        .map(DiscoveredDevice::from_xml)
        .collect()
}

// ── Lenient fallback parser ──────────────────────────────────────────────────
//
// Used only when `XmlNode::parse` rejects a datagram. Scans the raw text for
// `<...ProbeMatch>` blocks and pulls out fields by local tag name, tolerating
// the malformed XML that real-world cameras often emit:
//
// * unescaped `&` inside scope URIs (e.g. `name/A&B`)
// * unclosed or mismatched namespace prefixes
// * wrong-encoded CJK bytes in `<Scopes>`
// * stray content outside `<Body>`
//
// The scanner is intentionally structural — it matches on local tag names and
// ignores attributes, namespace prefixes, and overall validity. Same approach
// used by the reference Java `wsdiscovery` and ONVIF Device Manager.

fn collect_probe_matches_lenient(text: &str) -> Vec<DiscoveredDevice> {
    let mut devices = Vec::new();
    for block in extract_blocks(text, "ProbeMatch") {
        let endpoint = extract_first_tag(&block, "Address").unwrap_or_default();
        if endpoint.is_empty() {
            // No endpoint UUID means we can't dedup across rounds or
            // correlate with other responses — drop it.
            continue;
        }
        let types = extract_first_tag(&block, "Types")
            .map(|s| split_ws(&s))
            .unwrap_or_default();
        let scopes = extract_first_tag(&block, "Scopes")
            .map(|s| split_ws(&s))
            .unwrap_or_default();
        let xaddrs = extract_first_tag(&block, "XAddrs")
            .map(|s| split_ws(&s))
            .unwrap_or_default();
        devices.push(DiscoveredDevice {
            endpoint,
            types,
            scopes,
            xaddrs,
        });
    }
    devices
}

fn split_ws(s: &str) -> Vec<String> {
    s.split_whitespace()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Extract the inner text of every tag with the given local name.
///
/// Handles namespace prefixes (`<d:ProbeMatch>`, `<wsdd:ProbeMatch>`,
/// `<ProbeMatch>` are all matched). Distinguishes `ProbeMatch` from
/// `ProbeMatches` by requiring exact local-name match.
fn extract_blocks(xml: &str, local_name: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut search_from = 0;

    while search_from < xml.len() {
        let open = match find_open_tag(&xml[search_from..], local_name) {
            Some((start, end)) => (search_from + start, search_from + end),
            None => break,
        };
        let close = match find_close_tag(&xml[open.1..], local_name) {
            Some(pos) => open.1 + pos,
            None => break,
        };
        blocks.push(xml[open.1..close].to_string());
        search_from = close;
    }

    blocks
}

/// Find `<Name>` or `<ns:Name>` (not `<NameSuffix>` or `</Name>`).
/// Returns `(start_of_<, end_of_>)` — i.e. the slice *after* `end` is the tag's
/// inner content start.
fn find_open_tag(xml: &str, local_name: &str) -> Option<(usize, usize)> {
    let mut pos = 0;
    while pos < xml.len() {
        let rest = &xml[pos..];
        let lt = rest.find('<')?;
        let abs_lt = pos + lt;
        let after_lt = &xml[abs_lt + 1..];

        // Skip closing tags, processing instructions, comments, DOCTYPE.
        if after_lt.starts_with('/') || after_lt.starts_with('?') || after_lt.starts_with('!') {
            pos = abs_lt + 1;
            continue;
        }

        let gt = match after_lt.find('>') {
            Some(p) => p,
            None => break,
        };
        let tag_content = &after_lt[..gt]; // "d:ProbeMatch" or "ProbeMatch attr=..."
        let tag_name = tag_content.split_whitespace().next().unwrap_or("");
        let local = tag_name.rsplit(':').next().unwrap_or(tag_name);

        // Strip trailing `/` so `<Foo/>` local name matches "Foo".
        let local = local.trim_end_matches('/');

        if local == local_name {
            return Some((abs_lt, abs_lt + 1 + gt + 1));
        }

        pos = abs_lt + 1;
    }
    None
}

/// Find the `</Name>` closing tag position (start of `<`).
fn find_close_tag(xml: &str, local_name: &str) -> Option<usize> {
    let mut pos = 0;
    while pos < xml.len() {
        let rest = &xml[pos..];
        let lt = rest.find("</")?;
        let abs_lt = pos + lt;
        let after_close = &xml[abs_lt + 2..];

        let gt = after_close.find('>')?;
        let tag_name = after_close[..gt].trim();
        let local = tag_name.rsplit(':').next().unwrap_or(tag_name);

        if local == local_name {
            return Some(abs_lt);
        }

        pos = abs_lt + 2;
    }
    None
}

fn extract_first_tag(xml: &str, local_name: &str) -> Option<String> {
    extract_blocks(xml, local_name)
        .into_iter()
        .next()
        .map(|s| s.trim().to_string())
}

#[derive(Clone, Copy)]
enum ProbeTarget {
    /// `dn:NetworkVideoTransmitter` — covers most IP cameras.
    NetworkVideoTransmitter,
    /// `tds:Device` — covers NVRs, doorbells, and Profile T / S devices that
    /// advertise the Device service but not a media transmitter. Some
    /// firmware responds only to this type, so ODM and oxvif both send
    /// both probes and merge by endpoint UUID.
    Device,
}

fn build_probe(message_id: &str, target: ProbeTarget) -> String {
    let (types, onvif_ns_decl) = match target {
        ProbeTarget::NetworkVideoTransmitter => (
            "dn:NetworkVideoTransmitter",
            r#" xmlns:dn="http://www.onvif.org/ver10/network/wsdl""#,
        ),
        ProbeTarget::Device => (
            "tds:Device",
            r#" xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        ),
    };
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<s:Envelope"#,
            r#" xmlns:s="http://www.w3.org/2003/05/soap-envelope""#,
            r#" xmlns:wsa="http://www.w3.org/2005/08/addressing""#,
            r#" xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery""#,
            r#"{}>"#,
            r#"<s:Header>"#,
            r#"<wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</wsa:Action>"#,
            r#"<wsa:MessageID>uuid:{}</wsa:MessageID>"#,
            r#"<wsa:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</wsa:To>"#,
            r#"</s:Header>"#,
            r#"<s:Body>"#,
            r#"<wsd:Probe><wsd:Types>{}</wsd:Types></wsd:Probe>"#,
            r#"</s:Body>"#,
            r#"</s:Envelope>"#,
        ),
        onvif_ns_decl, message_id, types
    )
}

fn new_uuid() -> String {
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        rand::random::<u32>(),
        rand::random::<u16>(),
        rand::random::<u16>() & 0x0fff,
        (rand::random::<u16>() & 0x3fff) | 0x8000,
        rand::random::<u64>() & 0x0000_ffff_ffff_ffff,
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_match_xml(endpoint: &str, xaddrs: &str) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery"
                          xmlns:wsa="http://www.w3.org/2005/08/addressing">
               <s:Body>
                 <wsd:ProbeMatches>
                   <wsd:ProbeMatch>
                     <wsa:EndpointReference>
                       <wsa:Address>{endpoint}</wsa:Address>
                     </wsa:EndpointReference>
                     <wsd:Types>dn:NetworkVideoTransmitter</wsd:Types>
                     <wsd:Scopes>onvif://www.onvif.org/name/Camera1</wsd:Scopes>
                     <wsd:XAddrs>{xaddrs}</wsd:XAddrs>
                     <wsd:MetadataVersion>10</wsd:MetadataVersion>
                   </wsd:ProbeMatch>
                 </wsd:ProbeMatches>
               </s:Body>
             </s:Envelope>"#
        )
    }

    #[test]
    fn test_parse_probe_match_extracts_fields() {
        let xml = probe_match_xml(
            "uuid:12345678-0000-0000-0000-000000000001",
            "http://192.168.1.100/onvif/device_service",
        );
        let root = XmlNode::parse(&xml).unwrap();
        let devices = collect_probe_matches(&root);
        assert_eq!(devices.len(), 1);
        let d = &devices[0];
        assert_eq!(d.endpoint, "uuid:12345678-0000-0000-0000-000000000001");
        assert_eq!(d.xaddrs, ["http://192.168.1.100/onvif/device_service"]);
        assert_eq!(d.scopes, ["onvif://www.onvif.org/name/Camera1"]);
        assert!(
            d.types
                .iter()
                .any(|t| t.contains("NetworkVideoTransmitter"))
        );
    }

    #[test]
    fn test_parse_multiple_xaddrs() {
        let xml = probe_match_xml(
            "uuid:aabbccdd-0000-0000-0000-000000000002",
            "http://192.168.1.101/onvif/device_service http://10.0.0.1/onvif/device_service",
        );
        let root = XmlNode::parse(&xml).unwrap();
        let devices = collect_probe_matches(&root);
        assert_eq!(devices[0].xaddrs.len(), 2);
    }

    #[test]
    fn test_parse_empty_body_returns_empty() {
        let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
                       <s:Body/>
                     </s:Envelope>"#;
        let root = XmlNode::parse(xml).unwrap();
        assert!(collect_probe_matches(&root).is_empty());
    }

    #[test]
    fn test_build_nvt_probe_is_valid_xml() {
        let xml = build_probe("test-uuid-1234", ProbeTarget::NetworkVideoTransmitter);
        assert!(
            XmlNode::parse(&xml).is_ok(),
            "NVT probe output should be valid XML"
        );
        assert!(xml.contains("NetworkVideoTransmitter"));
        assert!(xml.contains("onvif.org/ver10/network/wsdl"));
        assert!(xml.contains("test-uuid-1234"));
        assert!(
            !xml.contains("Device</"),
            "NVT probe must not accidentally request Device type"
        );
    }

    #[test]
    fn test_build_device_probe_is_valid_xml() {
        let xml = build_probe("test-uuid-5678", ProbeTarget::Device);
        assert!(
            XmlNode::parse(&xml).is_ok(),
            "Device probe output should be valid XML"
        );
        assert!(xml.contains("tds:Device"));
        assert!(xml.contains("onvif.org/ver10/device/wsdl"));
        assert!(xml.contains("test-uuid-5678"));
        assert!(
            !xml.contains("NetworkVideoTransmitter"),
            "Device probe must not accidentally request NVT type"
        );
    }

    #[test]
    fn test_new_uuid_has_five_parts() {
        let uuid = new_uuid();
        let parts: Vec<&str> = uuid.split('-').collect();
        assert_eq!(parts.len(), 5, "UUID should have 5 dash-separated parts");
    }

    fn hello_xml(endpoint: &str, xaddrs: &str) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery"
                          xmlns:wsa="http://www.w3.org/2005/08/addressing">
               <s:Header>
                 <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Hello</wsa:Action>
               </s:Header>
               <s:Body>
                 <wsd:Hello>
                   <wsa:EndpointReference>
                     <wsa:Address>{endpoint}</wsa:Address>
                   </wsa:EndpointReference>
                   <wsd:Types>dn:NetworkVideoTransmitter</wsd:Types>
                   <wsd:Scopes>onvif://www.onvif.org/name/Camera1</wsd:Scopes>
                   <wsd:XAddrs>{xaddrs}</wsd:XAddrs>
                   <wsd:MetadataVersion>1</wsd:MetadataVersion>
                 </wsd:Hello>
               </s:Body>
             </s:Envelope>"#
        )
    }

    fn bye_xml(endpoint: &str) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery"
                          xmlns:wsa="http://www.w3.org/2005/08/addressing">
               <s:Header>
                 <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Bye</wsa:Action>
               </s:Header>
               <s:Body>
                 <wsd:Bye>
                   <wsa:EndpointReference>
                     <wsa:Address>{endpoint}</wsa:Address>
                   </wsa:EndpointReference>
                 </wsd:Bye>
               </s:Body>
             </s:Envelope>"#
        )
    }

    #[test]
    fn test_collect_hello_event() {
        let xml = hello_xml(
            "uuid:aaaa-0000-0000-0000-000000000001",
            "http://192.168.1.200/onvif/device_service",
        );
        let root = XmlNode::parse(&xml).unwrap();
        let events = collect_discovery_events(&root);
        assert_eq!(events.len(), 1);
        match &events[0] {
            DiscoveryEvent::Hello(d) => {
                assert_eq!(d.endpoint, "uuid:aaaa-0000-0000-0000-000000000001");
                assert_eq!(d.xaddrs, ["http://192.168.1.200/onvif/device_service"]);
            }
            DiscoveryEvent::Bye { .. } => panic!("expected Hello"),
        }
    }

    #[test]
    fn test_collect_bye_event() {
        let xml = bye_xml("uuid:bbbb-0000-0000-0000-000000000002");
        let root = XmlNode::parse(&xml).unwrap();
        let events = collect_discovery_events(&root);
        assert_eq!(events.len(), 1);
        match &events[0] {
            DiscoveryEvent::Bye { endpoint } => {
                assert_eq!(endpoint, "uuid:bbbb-0000-0000-0000-000000000002");
            }
            DiscoveryEvent::Hello(_) => panic!("expected Bye"),
        }
    }

    #[test]
    fn test_collect_unknown_action_returns_empty() {
        let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                                  xmlns:wsa="http://www.w3.org/2005/08/addressing">
               <s:Header>
                 <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</wsa:Action>
               </s:Header>
               <s:Body/>
             </s:Envelope>"#;
        let root = XmlNode::parse(xml).unwrap();
        assert!(collect_discovery_events(&root).is_empty());
    }

    // ── End-to-end UDP probe test ─────────────────────────────────────────────

    /// Spins up a local UDP mock that replies with a canned ProbeMatch,
    /// then verifies that `probe_inner` finds exactly that device.
    #[tokio::test]
    async fn test_probe_inner_receives_probe_match() {
        use std::time::Duration;
        use tokio::net::UdpSocket;

        // Bind mock on all interfaces (port 0 = OS assigns a free port).
        let mock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        let mock_addr = mock.local_addr().unwrap();
        let target = format!("127.0.0.1:{}", mock_addr.port());

        let canned = probe_match_xml(
            "uuid:mock-device-0001-0000-000000000001",
            "http://192.168.1.200/onvif/device_service",
        );

        // Responder: receive one probe, send back the canned ProbeMatch.
        tokio::spawn(async move {
            let mut buf = vec![0u8; UDP_MAX_SIZE];
            if let Ok((_, src)) = mock.recv_from(&mut buf).await {
                let _ = mock.send_to(canned.as_bytes(), src).await;
            }
        });

        let devices = probe_inner(1, Duration::from_millis(500), Duration::ZERO, &target)
            .await
            .unwrap();

        assert_eq!(devices.len(), 1, "should find exactly one device");
        assert_eq!(
            devices[0].endpoint,
            "uuid:mock-device-0001-0000-000000000001"
        );
        assert_eq!(
            devices[0].xaddrs,
            ["http://192.168.1.200/onvif/device_service"]
        );
        assert_eq!(devices[0].scopes, ["onvif://www.onvif.org/name/Camera1"]);
    }

    /// Verifies that duplicate ProbeMatch responses (same endpoint UUID)
    /// are deduplicated into a single device entry.
    #[tokio::test]
    async fn test_probe_inner_deduplicates_responses() {
        use std::time::Duration;
        use tokio::net::UdpSocket;

        let mock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        let mock_addr = mock.local_addr().unwrap();
        let target = format!("127.0.0.1:{}", mock_addr.port());

        let canned = probe_match_xml(
            "uuid:mock-device-dup-0000-000000000002",
            "http://192.168.1.201/onvif/device_service",
        );

        // Send the same ProbeMatch twice to simulate a duplicate response.
        tokio::spawn(async move {
            let mut buf = vec![0u8; UDP_MAX_SIZE];
            if let Ok((_, src)) = mock.recv_from(&mut buf).await {
                let _ = mock.send_to(canned.as_bytes(), src).await;
                let _ = mock.send_to(canned.as_bytes(), src).await;
            }
        });

        let devices = probe_inner(1, Duration::from_millis(500), Duration::ZERO, &target)
            .await
            .unwrap();

        assert_eq!(devices.len(), 1, "duplicates should be merged into one");
    }

    /// Verifies that an empty / non-ONVIF UDP response is silently ignored.
    #[tokio::test]
    async fn test_probe_inner_ignores_garbage_response() {
        use std::time::Duration;
        use tokio::net::UdpSocket;

        let mock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        let mock_addr = mock.local_addr().unwrap();
        let target = format!("127.0.0.1:{}", mock_addr.port());

        tokio::spawn(async move {
            let mut buf = vec![0u8; UDP_MAX_SIZE];
            if let Ok((_, src)) = mock.recv_from(&mut buf).await {
                let _ = mock.send_to(b"not xml at all !!!", src).await;
            }
        });

        let devices = probe_inner(1, Duration::from_millis(300), Duration::ZERO, &target)
            .await
            .unwrap();

        assert!(
            devices.is_empty(),
            "garbage response should yield no devices"
        );
    }

    /// Verifies that `probe_inner` with multiple rounds deduplicates across
    /// rounds and pauses for `interval` between them.
    #[tokio::test]
    async fn test_probe_inner_multi_round_dedups_and_sleeps() {
        use std::time::{Duration, Instant};
        use tokio::net::UdpSocket;

        let mock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        let mock_addr = mock.local_addr().unwrap();
        let target = format!("127.0.0.1:{}", mock_addr.port());

        let canned = probe_match_xml(
            "uuid:mock-device-multi-0000-000000000003",
            "http://192.168.1.202/onvif/device_service",
        );

        // Respond to every probe we receive (up to 3 rounds worth).
        tokio::spawn(async move {
            let mut buf = vec![0u8; UDP_MAX_SIZE];
            for _ in 0..3 {
                if let Ok((_, src)) = mock.recv_from(&mut buf).await {
                    let _ = mock.send_to(canned.as_bytes(), src).await;
                }
            }
        });

        let started = Instant::now();
        let devices = probe_inner(
            2,
            Duration::from_millis(200),
            Duration::from_millis(300),
            &target,
        )
        .await
        .unwrap();
        let elapsed = started.elapsed();

        assert_eq!(
            devices.len(),
            1,
            "same endpoint across rounds should dedup to one"
        );
        // 2 rounds × 200ms listen + 1 interval × 300ms = 700ms minimum.
        assert!(
            elapsed >= Duration::from_millis(700),
            "expected >= 700ms for 2 rounds + 1 interval, got {elapsed:?}"
        );
    }

    // ── Lenient parser recovery ───────────────────────────────────────────────

    /// WS-Discovery response with an unescaped `&` inside the scope URI.
    /// `XmlNode::parse` rejects this as invalid XML entity; a tolerant
    /// string scanner still extracts the fields.
    const MALFORMED_PROBE_MATCH: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery"
            xmlns:wsa="http://www.w3.org/2005/08/addressing">
  <s:Body>
    <wsd:ProbeMatches>
      <wsd:ProbeMatch>
        <wsa:EndpointReference>
          <wsa:Address>uuid:malformed-scope-0000-0000-000000000099</wsa:Address>
        </wsa:EndpointReference>
        <wsd:Types>dn:NetworkVideoTransmitter</wsd:Types>
        <wsd:Scopes>onvif://www.onvif.org/name/Alpha&Beta</wsd:Scopes>
        <wsd:XAddrs>http://192.168.1.99/onvif/device_service</wsd:XAddrs>
      </wsd:ProbeMatch>
    </wsd:ProbeMatches>
  </s:Body>
</s:Envelope>"#;

    #[test]
    fn test_strict_parser_rejects_malformed_xml() {
        // Sanity check for the premise: we want a fixture that actually
        // breaks XmlNode, otherwise the fallback test proves nothing.
        assert!(
            XmlNode::parse(MALFORMED_PROBE_MATCH).is_err(),
            "fixture should break XmlNode so the lenient fallback is exercised"
        );
    }

    #[test]
    fn test_lenient_parser_recovers_malformed_xml() {
        let devices = collect_probe_matches_lenient(MALFORMED_PROBE_MATCH);
        assert_eq!(
            devices.len(),
            1,
            "lenient scanner should extract one device"
        );
        let d = &devices[0];
        assert_eq!(d.endpoint, "uuid:malformed-scope-0000-0000-000000000099");
        assert_eq!(d.xaddrs, ["http://192.168.1.99/onvif/device_service"]);
        assert!(
            d.types
                .iter()
                .any(|t| t.contains("NetworkVideoTransmitter")),
            "types should include NetworkVideoTransmitter"
        );
    }

    #[test]
    fn test_lenient_parser_drops_missing_endpoint() {
        // No <Address> → no way to dedup → drop.
        let xml = r#"<wsd:ProbeMatch xmlns:wsd="...">
            <wsd:Types>dn:NetworkVideoTransmitter</wsd:Types>
            <wsd:XAddrs>http://10.0.0.1/onvif/device_service</wsd:XAddrs>
        </wsd:ProbeMatch>"#;
        let devices = collect_probe_matches_lenient(xml);
        assert!(devices.is_empty());
    }

    #[test]
    fn test_lenient_parser_distinguishes_probematch_from_probematches() {
        // Don't accidentally match the outer <ProbeMatches> wrapper.
        let xml = r#"<wsd:ProbeMatches xmlns:wsd="..."/>"#;
        assert!(collect_probe_matches_lenient(xml).is_empty());
    }

    /// Verifies that each round sends TWO probes per socket — one for
    /// NetworkVideoTransmitter and one for Device — by capturing the raw
    /// probe XMLs that hit the mock and asserting both types are present.
    ///
    /// Before this behaviour was added, devices that advertise only the
    /// Device type (NVRs, doorbells, Profile T encoders) were silently
    /// ignored; see ODM's NvtDiscovery.fs for the reference implementation.
    #[tokio::test]
    async fn test_probe_once_sends_nvt_and_device_probes() {
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        use tokio::net::UdpSocket;

        let mock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        let mock_addr = mock.local_addr().unwrap();
        let target = format!("127.0.0.1:{}", mock_addr.port());

        // Capture every probe the mock receives. Per NIC we expect >= 2
        // datagrams (NVT + Device); on a machine with N interfaces there
        // will be roughly (N + 1) × 2 (the extra +1 is the 0.0.0.0
        // catch-all socket).
        let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_task = Arc::clone(&captured);

        tokio::spawn(async move {
            let mut buf = vec![0u8; UDP_MAX_SIZE];
            // Drain aggressively for the test window.
            for _ in 0..16 {
                match tokio::time::timeout(Duration::from_millis(400), mock.recv_from(&mut buf))
                    .await
                {
                    Ok(Ok((len, _))) => {
                        if let Ok(text) = std::str::from_utf8(&buf[..len]) {
                            captured_task.lock().unwrap().push(text.to_string());
                        }
                    }
                    _ => break,
                }
            }
        });

        let _ = probe_inner(1, Duration::from_millis(300), Duration::ZERO, &target)
            .await
            .unwrap();

        // Give the capture task a moment to drain the socket buffer.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let probes = captured.lock().unwrap().clone();
        assert!(
            probes.len() >= 2,
            "expected at least one NVT + one Device probe, got {}",
            probes.len()
        );
        assert!(
            probes.iter().any(|p| p.contains("NetworkVideoTransmitter")),
            "no probe asked for NetworkVideoTransmitter: {probes:?}"
        );
        assert!(
            probes.iter().any(|p| p.contains("tds:Device")),
            "no probe asked for tds:Device: {probes:?}"
        );
    }

    /// `probe_rounds(0, _, _)` must return immediately with no devices and
    /// no I/O — no NIC enumeration, no socket bind.
    #[tokio::test]
    async fn test_probe_inner_zero_rounds_is_noop() {
        use std::time::{Duration, Instant};

        let started = Instant::now();
        let devices = probe_inner(0, Duration::from_secs(30), Duration::ZERO, "127.0.0.1:1")
            .await
            .unwrap();
        assert!(devices.is_empty(), "zero rounds should yield no devices");
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "zero rounds must not block on the 30s timeout"
        );
    }
}
