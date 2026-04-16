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
use std::net::IpAddr;
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

/// Send a WS-Discovery `Probe` to a single known IP via unicast.
///
/// Useful for "is this device still there" checks against a known address —
/// avoids flooding the LAN with multicast and works across subnets where
/// multicast WS-Discovery cannot reach. The target only needs to listen on
/// UDP 3702 (every ONVIF device does).
///
/// Sends both `NetworkVideoTransmitter` and `Device` probes (the same dual
/// probe used by [`probe`] / [`probe_rounds`]) and deduplicates the responses
/// by endpoint UUID. Most devices reply with one `ProbeMatch`, but a few
/// reply twice (once per probe type).
///
/// Returns an empty `Vec` on timeout or any I/O error — treat failures the
/// same way as for the multicast variants.
///
/// # Example
///
/// ```no_run
/// use std::net::IpAddr;
/// use std::time::Duration;
/// use oxvif::discovery;
///
/// # async fn run() {
/// let ip: IpAddr = "192.168.1.100".parse().unwrap();
/// let devices = discovery::probe_unicast(ip, Duration::from_secs(2)).await;
/// if devices.is_empty() {
///     println!("device unreachable");
/// }
/// # }
/// ```
pub async fn probe_unicast(ip: IpAddr, timeout_dur: Duration) -> Vec<DiscoveredDevice> {
    let target = format!("{ip}:3702");
    probe_unicast_inner(timeout_dur, &target)
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
        merge_probe_responses(raw, &mut devices, &mut seen);
    }

    Ok(devices)
}

/// Parse raw ProbeMatch datagrams and append unseen devices to `out`.
///
/// Fast path: strict DOM parse via [`XmlNode::parse`]. Most compliant cameras
/// land here.
///
/// Slow path: if the datagram contains malformed XML (unescaped ampersands
/// in scope URIs, unclosed tags, wrong-encoded CJK text, etc.) the strict
/// parse fails and we fall back to a tolerant string scanner. Matches how
/// the reference Java/C# WS-Discovery clients (and ODM) handle wire-level
/// noise.
fn merge_probe_responses(
    raw: Vec<Vec<u8>>,
    out: &mut Vec<DiscoveredDevice>,
    seen: &mut HashSet<String>,
) {
    for data in raw {
        let Ok(text) = std::str::from_utf8(&data) else {
            continue;
        };
        let matches = match XmlNode::parse(text) {
            Ok(root) => collect_probe_matches(&root),
            Err(_) => collect_probe_matches_lenient(text),
        };
        for d in matches {
            if seen.insert(d.endpoint.clone()) {
                out.push(d);
            }
        }
    }
}

/// Send NVT + Device probes to a single unicast target and collect responses.
///
/// Unlike [`probe_once`] this does not enumerate NICs, set `IP_MULTICAST_IF`,
/// or join a multicast group — it binds an ephemeral socket on the matching
/// IP family and sends two datagrams to `target`. The same dedup-by-endpoint
/// rule applies because cameras often respond once per probe type.
async fn probe_unicast_inner(
    timeout_dur: Duration,
    target: &str,
) -> std::io::Result<Vec<DiscoveredDevice>> {
    // Resolve target up front so we can pick the matching bind family. If
    // resolution fails we still try the OS default (`0.0.0.0:0`) — covers
    // hostnames that resolve only at send time on some platforms.
    let bind_addr: &str = match target.parse::<std::net::SocketAddr>() {
        Ok(addr) if addr.is_ipv6() => "[::]:0",
        _ => "0.0.0.0:0",
    };

    let sock = UdpSocket::bind(bind_addr).await?;
    let nvt_probe = build_probe(&new_uuid(), ProbeTarget::NetworkVideoTransmitter);
    let device_probe = build_probe(&new_uuid(), ProbeTarget::Device);
    let _ = sock.send_to(nvt_probe.as_bytes(), target).await;
    let _ = sock.send_to(device_probe.as_bytes(), target).await;

    let mut raw: Vec<Vec<u8>> = Vec::new();
    let mut buf = vec![0u8; UDP_MAX_SIZE];
    let deadline = Instant::now() + timeout_dur;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, sock.recv_from(&mut buf)).await {
            Ok(Ok((len, _))) => raw.push(buf[..len].to_vec()),
            Ok(Err(_)) => continue, // transient — keep waiting
            Err(_) => break,        // timeout elapsed
        }
    }

    let mut devices: Vec<DiscoveredDevice> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    merge_probe_responses(raw, &mut devices, &mut seen);
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

    // JoinSet so that dropping the outer future (caller cancels mid-probe)
    // aborts every per-NIC listener task. Plain `tokio::spawn` would orphan
    // them and they'd keep holding sockets until their own timeout elapses.
    let mut join_set: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();

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
        // TTL = 32 covers the common enterprise case where the camera subnet
        // is reached through one or two IGMP-routed hops (PIM/IGMP on a core
        // switch). The original `4` was tuned for a single LAN segment and
        // silently lost devices on routed networks. ODM uses `64` as a "VPN
        // workaround" — 32 is a middle ground that still respects the spec's
        // intent that WS-Discovery stays close to the link.
        let _ = raw.set_multicast_ttl_v4(32);
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
        join_set.spawn(async move {
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
    }

    // join_all awaits every spawned task to completion. If the surrounding
    // future is dropped before this future yields Ready, JoinSet's Drop
    // aborts all in-flight tasks for us — no leaked sockets.
    join_set.join_all().await;

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
    use std::collections::HashMap;
    use std::net::Ipv4Addr;

    let socket = UdpSocket::bind("0.0.0.0:3702").await?;
    socket.join_multicast_v4(WSD_MULTICAST_ADDR, Ipv4Addr::UNSPECIFIED)?;

    let mut buf = vec![0u8; UDP_MAX_SIZE];
    let mut events: Vec<DiscoveryEvent> = Vec::new();
    // Per-endpoint AppSequence high-water mark. Used to drop reordered
    // `Bye` datagrams: if the fresh Bye carries a sequence that is older
    // than (or equal to) one we already saw, the network reordered an old
    // departure on top of a more recent presence — silently dropping it
    // avoids flapping a still-online device offline.
    let mut last_seq: HashMap<String, AppSequence> = HashMap::new();
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
                    for (ev, seq) in collect_discovery_events(&root) {
                        if !accept_event(&ev, seq.as_ref(), &mut last_seq) {
                            continue;
                        }
                        events.push(ev);
                    }
                }
            }
            _ => break,
        }
    }
    Ok(events)
}

/// WS-Discovery `<wsd:AppSequence>` header — used to detect reordered
/// announcements (a stale `Bye` arriving after a fresh `Hello`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct AppSequence {
    /// Increments on every device restart. Different `InstanceId` ⇒ the
    /// sender restarted and prior sequence numbers are no longer comparable.
    instance_id: u64,
    /// Optional `SequenceId` (xs:anyURI) — partitions a sender's messages
    /// into independent ordered streams. Different `SequenceId` (within
    /// the same `InstanceId`) is also incomparable.
    sequence_id: Option<String>,
    /// Strictly increasing within (sender, instance, sequence).
    message_number: u64,
}

impl AppSequence {
    /// Two AppSequences are comparable iff they share the same
    /// `(instance_id, sequence_id)` pair. Same endpoint UUID is enforced
    /// implicitly by the caller (we key the high-water-mark map by endpoint).
    fn comparable_to(&self, other: &Self) -> bool {
        self.instance_id == other.instance_id && self.sequence_id == other.sequence_id
    }

    fn from_node(node: &XmlNode) -> Option<Self> {
        let instance_id = node.attr("InstanceId")?.parse().ok()?;
        let message_number = node.attr("MessageNumber")?.parse().ok()?;
        let sequence_id = node.attr("SequenceId").map(str::to_string);
        Some(Self {
            instance_id,
            message_number,
            sequence_id,
        })
    }
}

/// Decide whether to surface an event, updating the per-endpoint
/// high-water mark in the process.
///
/// Rules:
/// * `Hello` always passes (a stale Hello at worst resurfaces a live
///   device — never falsely removes one). Its sequence still updates the
///   high-water mark so a subsequent stale `Bye` can be filtered.
/// * `Bye` is dropped when it carries a sequence comparable to (same
///   `InstanceId` and `SequenceId`) one we've already seen but with an
///   equal-or-lower `MessageNumber` — i.e. the network reordered an old
///   departure on top of newer presence. Incomparable Bye (different
///   `InstanceId` ⇒ device restarted between) is accepted as the device
///   genuinely left and came back.
fn accept_event(
    event: &DiscoveryEvent,
    seq: Option<&AppSequence>,
    last_seq: &mut std::collections::HashMap<String, AppSequence>,
) -> bool {
    let endpoint = match event {
        DiscoveryEvent::Hello(d) => d.endpoint.clone(),
        DiscoveryEvent::Bye { endpoint } => endpoint.clone(),
    };

    if matches!(event, DiscoveryEvent::Bye { .. })
        && let Some(new) = seq
        && let Some(prev) = last_seq.get(&endpoint)
        && new.comparable_to(prev)
        && new.message_number <= prev.message_number
    {
        return false;
    }

    if let Some(new) = seq {
        match last_seq.get(&endpoint) {
            Some(prev) if new.comparable_to(prev) && new.message_number < prev.message_number => {
                // Older than what we have; don't lower the bar.
            }
            _ => {
                last_seq.insert(endpoint, new.clone());
            }
        }
    }
    true
}

fn collect_discovery_events(root: &XmlNode) -> Vec<(DiscoveryEvent, Option<AppSequence>)> {
    // Determine message type from the WS-Addressing Action header.
    let action = root
        .path(&["Header", "Action"])
        .map(|n| n.text())
        .unwrap_or("");

    let seq = root
        .path(&["Header", "AppSequence"])
        .and_then(AppSequence::from_node);

    let body = root.child("Body").unwrap_or(root);

    if action.ends_with("/Hello") {
        if let Some(hello) = body.child("Hello") {
            return vec![(
                DiscoveryEvent::Hello(DiscoveredDevice::from_xml(hello)),
                seq,
            )];
        }
    } else if action.ends_with("/Bye") {
        if let Some(bye) = body.child("Bye") {
            let endpoint = bye
                .path(&["EndpointReference", "Address"])
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            return vec![(DiscoveryEvent::Bye { endpoint }, seq)];
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
    // Legacy WS-Addressing format. Older Chinese OEM camera firmwares
    // (Hikvision, Uniview, Dahua-family) silently drop probes that use
    // the 2005/08 wsa namespace — they only recognise 2004/08, require
    // `s:mustUnderstand="1"` on Action/To headers, and expect an explicit
    // `<wsa:ReplyTo>`. This is also the exact wire format ODM sends, and
    // the format the pre-oxvif oxdm used when it could discover ~195 of
    // 195 cameras on a real heterogeneous LAN. Modernising to 2005/08
    // (the WS-Discovery 1.1 wsa) cost ~80 of those devices.
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<s:Envelope"#,
            r#" xmlns:s="http://www.w3.org/2003/05/soap-envelope""#,
            r#" xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing""#,
            r#" xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery""#,
            r#"{}>"#,
            r#"<s:Header>"#,
            r#"<wsa:Action s:mustUnderstand="1">http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</wsa:Action>"#,
            r#"<wsa:MessageID>uuid:{}</wsa:MessageID>"#,
            r#"<wsa:ReplyTo><wsa:Address>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:Address></wsa:ReplyTo>"#,
            r#"<wsa:To s:mustUnderstand="1">urn:schemas-xmlsoap-org:ws:2005:04:discovery</wsa:To>"#,
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
        match &events[0].0 {
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
        match &events[0].0 {
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

    // ── AppSequence reorder filter ───────────────────────────────────────────

    fn hello_with_seq_xml(endpoint: &str, instance_id: u64, msg_num: u64) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery"
                          xmlns:wsa="http://www.w3.org/2005/08/addressing">
               <s:Header>
                 <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Hello</wsa:Action>
                 <wsd:AppSequence InstanceId="{instance_id}" MessageNumber="{msg_num}"/>
               </s:Header>
               <s:Body>
                 <wsd:Hello>
                   <wsa:EndpointReference><wsa:Address>{endpoint}</wsa:Address></wsa:EndpointReference>
                   <wsd:Types>dn:NetworkVideoTransmitter</wsd:Types>
                   <wsd:XAddrs>http://192.168.1.50/onvif/device_service</wsd:XAddrs>
                 </wsd:Hello>
               </s:Body>
             </s:Envelope>"#
        )
    }

    fn bye_with_seq_xml(endpoint: &str, instance_id: u64, msg_num: u64) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery"
                          xmlns:wsa="http://www.w3.org/2005/08/addressing">
               <s:Header>
                 <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Bye</wsa:Action>
                 <wsd:AppSequence InstanceId="{instance_id}" MessageNumber="{msg_num}"/>
               </s:Header>
               <s:Body>
                 <wsd:Bye>
                   <wsa:EndpointReference><wsa:Address>{endpoint}</wsa:Address></wsa:EndpointReference>
                 </wsd:Bye>
               </s:Body>
             </s:Envelope>"#
        )
    }

    /// Drives `accept_event` over a sequence of XML datagrams and returns
    /// the events that survived the reorder filter — exactly the same
    /// pipeline as `listen_inner`, just minus the socket I/O.
    fn replay(xmls: &[String]) -> Vec<DiscoveryEvent> {
        use std::collections::HashMap;
        let mut last_seq: HashMap<String, AppSequence> = HashMap::new();
        let mut out = Vec::new();
        for xml in xmls {
            let root = XmlNode::parse(xml).unwrap();
            for (ev, seq) in collect_discovery_events(&root) {
                if accept_event(&ev, seq.as_ref(), &mut last_seq) {
                    out.push(ev);
                }
            }
        }
        out
    }

    #[test]
    fn test_reorder_filter_in_order_passes() {
        let ep = "uuid:device-aa-0000-0000-000000000001";
        let stream = vec![hello_with_seq_xml(ep, 100, 1), bye_with_seq_xml(ep, 100, 2)];
        let events = replay(&stream);
        assert_eq!(events.len(), 2, "in-order Hello/Bye should both pass");
        assert!(matches!(events[0], DiscoveryEvent::Hello(_)));
        assert!(matches!(events[1], DiscoveryEvent::Bye { .. }));
    }

    #[test]
    fn test_reorder_filter_drops_stale_bye() {
        // Hello @ msg 5 then a stale Bye @ msg 3 (same InstanceId, lower
        // MessageNumber) — the Bye is reordered network noise, drop it.
        let ep = "uuid:device-bb-0000-0000-000000000002";
        let stream = vec![hello_with_seq_xml(ep, 100, 5), bye_with_seq_xml(ep, 100, 3)];
        let events = replay(&stream);
        assert_eq!(events.len(), 1, "stale Bye must be dropped");
        assert!(matches!(events[0], DiscoveryEvent::Hello(_)));
    }

    #[test]
    fn test_reorder_filter_drops_equal_message_number_bye() {
        // Same MessageNumber as last seen Hello — also a duplicate, drop.
        let ep = "uuid:device-cc-0000-0000-000000000003";
        let stream = vec![hello_with_seq_xml(ep, 100, 7), bye_with_seq_xml(ep, 100, 7)];
        let events = replay(&stream);
        assert_eq!(events.len(), 1, "Bye with same msg# is a duplicate");
        assert!(matches!(events[0], DiscoveryEvent::Hello(_)));
    }

    #[test]
    fn test_reorder_filter_accepts_bye_after_restart() {
        // Hello @ instance 100 then Bye @ instance 200 (device rebooted in
        // between). Different InstanceId ⇒ incomparable ⇒ accept the Bye —
        // it really did go away (and came back) so removing the old
        // entry is correct.
        let ep = "uuid:device-dd-0000-0000-000000000004";
        let stream = vec![hello_with_seq_xml(ep, 100, 9), bye_with_seq_xml(ep, 200, 1)];
        let events = replay(&stream);
        assert_eq!(events.len(), 2, "Bye after device restart is real");
        assert!(matches!(events[1], DiscoveryEvent::Bye { .. }));
    }

    #[test]
    fn test_reorder_filter_per_endpoint_isolation() {
        // Sequences are tracked per-endpoint — a low Bye for device B does
        // not get filtered by a high Hello for device A.
        let ep_a = "uuid:device-ee-0000-0000-00000000000a";
        let ep_b = "uuid:device-ee-0000-0000-00000000000b";
        let stream = vec![
            hello_with_seq_xml(ep_a, 100, 50),
            bye_with_seq_xml(ep_b, 100, 1),
        ];
        let events = replay(&stream);
        assert_eq!(events.len(), 2);
        assert!(matches!(events[1], DiscoveryEvent::Bye { ref endpoint } if endpoint == ep_b));
    }

    #[test]
    fn test_reorder_filter_accepts_bye_when_no_appsequence() {
        // A device that omits AppSequence entirely — we have no basis to
        // filter, so events pass through (matches pre-filter behaviour).
        let ep = "uuid:device-ff-0000-0000-000000000006";
        let stream = vec![hello_xml(ep, "http://x"), bye_xml(ep)];
        let events = replay(&stream);
        assert_eq!(events.len(), 2);
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

    // ── Unicast probe ─────────────────────────────────────────────────────────

    /// Unicast probe must reach a known IP, deduplicate dual-probe responses,
    /// and return the parsed device.
    #[tokio::test]
    async fn test_probe_unicast_finds_device() {
        use std::time::Duration;
        use tokio::net::UdpSocket;

        let mock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mock_addr = mock.local_addr().unwrap();

        let canned = probe_match_xml(
            "uuid:unicast-mock-0000-0000-000000000010",
            "http://127.0.0.1/onvif/device_service",
        );

        // Reply to both NVT and Device probes (camera responds twice).
        tokio::spawn(async move {
            let mut buf = vec![0u8; UDP_MAX_SIZE];
            for _ in 0..2 {
                if let Ok((_, src)) = mock.recv_from(&mut buf).await {
                    let _ = mock.send_to(canned.as_bytes(), src).await;
                }
            }
        });

        let devices = probe_unicast_inner(Duration::from_millis(500), &mock_addr.to_string())
            .await
            .unwrap();

        assert_eq!(devices.len(), 1, "duplicate responses should dedup");
        assert_eq!(
            devices[0].endpoint,
            "uuid:unicast-mock-0000-0000-000000000010"
        );
    }

    /// Dropping the `probe_inner` future via `tokio::select!` must return
    /// control quickly — `JoinSet`'s drop aborts every per-NIC listener so
    /// they don't keep listening for the full configured timeout.
    #[tokio::test]
    async fn test_probe_inner_drop_returns_promptly() {
        use std::time::{Duration, Instant};

        let started = Instant::now();
        // 30s probe timeout, but we cancel after 50ms via select.
        tokio::select! {
            _ = probe_inner(1, Duration::from_secs(30), Duration::ZERO, "127.0.0.1:1") => {}
            _ = tokio::time::sleep(Duration::from_millis(50)) => {}
        }
        // If we got here within a reasonable margin of 50ms, drop propagated.
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "select cancellation should release the probe future, took {:?}",
            started.elapsed()
        );
    }

    /// Unicast probe against a silent target returns empty after the timeout
    /// (does not error).
    #[tokio::test]
    async fn test_probe_unicast_silent_target_returns_empty() {
        use std::time::{Duration, Instant};

        // Bind a socket to grab a port, then drop it — port is now free but
        // the OS may briefly reject sends. Either way, no replies.
        let probe_target = {
            let s = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let addr = s.local_addr().unwrap();
            drop(s);
            addr.to_string()
        };

        let started = Instant::now();
        let devices = probe_unicast_inner(Duration::from_millis(150), &probe_target)
            .await
            .unwrap();

        assert!(devices.is_empty());
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "should return shortly after the configured timeout"
        );
    }
}
