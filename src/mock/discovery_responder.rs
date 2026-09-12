//! WS-Discovery **responder** — the server side of `src/discovery.rs`
//! (metamorph M3, `feature = "mock-server"`).
//!
//! `discovery.rs` sends a `Probe` and parses `ProbeMatch`; this answers a
//! `Probe` with a `ProbeMatch`, so a mock / clone device is found by a normal
//! WS-Discovery client (oxdm, ONVIF Device Manager, Frigate). It advertises a
//! [`DiscoveredDevice`](crate::DiscoveredDevice) — the same shape a client
//! receives — so what a device announces and what a client discovers are one
//! type.
//!
//! UDP multicast is unreliable in some CI / containers. [`DiscoveryResponder::spawn_many`]
//! shares one listener across a bounded group. The core is split into pure
//! functions ([`build_probe_match`], [`probe_response`]) plus a spawnable
//! listener; tests drive the pure path and a loopback **unicast** round-trip,
//! never multicast.
//!
//! Simplification: only the `Probe`'s `<Types>` filter is honoured (an AND
//! match by local name, empty = match all) in the legacy single-device path,
//! which ignores `<Scopes>`. The new shared path validates known namespaces/types
//! and declines scoped probes instead of returning false matches. Neither path
//! implements complete WS-Discovery matching, Hello/Bye or Resolve.

use std::net::Ipv4Addr;
use std::{
    collections::VecDeque,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::net::UdpSocket;
use tokio::sync::oneshot;

use crate::discovery::{DiscoveredDevice, new_uuid};
use crate::soap::XmlNode;
use crate::types::xml_escape;

/// ONVIF WS-Discovery multicast group.
const WSD_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const MAX_DATAGRAM: usize = 16 * 1024;
const DISCOVERY_NS: &str = "http://schemas.xmlsoap.org/ws/2005/04/discovery";
const ADDRESSING_NS: &str = "http://schemas.xmlsoap.org/ws/2004/08/addressing";
const SOAP_NS: &str = "http://www.w3.org/2003/05/soap-envelope";
const NVT_NS: &str = "http://www.onvif.org/ver10/network/wsdl";
const DEVICE_NS: &str = "http://www.onvif.org/ver10/device/wsdl";

/// Build a `ProbeMatches` envelope advertising `dev`, correlated to the probe's
/// `MessageID` via `<wsa:RelatesTo>`.
///
/// Uses the same legacy WS-Addressing (2004/08) + WS-Discovery (2005/04)
/// namespaces `discovery::build_probe` sends and ONVIF Device Manager uses —
/// the wire format the widest range of camera firmware accepts.
pub(crate) fn build_probe_match(relates_to: &str, dev: &DiscoveredDevice) -> String {
    build_sequenced_match(relates_to, dev, 1, 1, None)
}

fn build_sequenced_match(
    relates_to: &str,
    dev: &DiscoveredDevice,
    instance: u32,
    number: u32,
    sequence: Option<&str>,
) -> String {
    let types = join_escaped(&dev.types);
    let scopes = join_escaped(&dev.scopes);
    let xaddrs = join_escaped(&dev.xaddrs);
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<s:Envelope"#,
            r#" xmlns:s="http://www.w3.org/2003/05/soap-envelope""#,
            r#" xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing""#,
            r#" xmlns:dn="http://www.onvif.org/ver10/network/wsdl""#,
            r#" xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
            r#" xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery">"#,
            r#"<s:Header>"#,
            r#"<wsa:MessageID>urn:uuid:{msg_id}</wsa:MessageID>"#,
            r#"<wsa:RelatesTo>{relates_to}</wsa:RelatesTo>"#,
            r#"<wsa:To>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:To>"#,
            r#"<wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/ProbeMatches</wsa:Action>"#,
            r#"<wsd:AppSequence InstanceId="{instance}" MessageNumber="{number}"{sequence}/>"#,
            r#"</s:Header>"#,
            r#"<s:Body>"#,
            r#"<wsd:ProbeMatches><wsd:ProbeMatch>"#,
            r#"<wsa:EndpointReference><wsa:Address>{endpoint}</wsa:Address></wsa:EndpointReference>"#,
            r#"<wsd:Types>{types}</wsd:Types>"#,
            r#"<wsd:Scopes>{scopes}</wsd:Scopes>"#,
            r#"<wsd:XAddrs>{xaddrs}</wsd:XAddrs>"#,
            r#"<wsd:MetadataVersion>1</wsd:MetadataVersion>"#,
            r#"</wsd:ProbeMatch></wsd:ProbeMatches>"#,
            r#"</s:Body>"#,
            r#"</s:Envelope>"#,
        ),
        msg_id = new_uuid(),
        relates_to = xml_escape(relates_to),
        endpoint = xml_escape(&dev.endpoint),
        types = types,
        scopes = scopes,
        xaddrs = xaddrs,
        instance = instance,
        number = number,
        sequence = sequence
            .map(|value| format!(" SequenceId=\"{}\"", xml_escape(value)))
            .unwrap_or_default(),
    )
}

/// Decide how to answer a raw incoming datagram advertising `dev`.
///
/// Returns `Some(probe_match_xml)` when `datagram` is a WS-Discovery `Probe`
/// whose `<Types>` filter `dev` satisfies, else `None` (not a probe, malformed,
/// or a type mismatch).
pub(crate) fn probe_response(datagram: &str, dev: &DiscoveredDevice) -> Option<String> {
    let root = XmlNode::parse(datagram).ok()?;

    // Must be a Probe (WS-Addressing Action header ending in `/Probe`).
    let action = root.path(&["Header", "Action"]).map(|n| n.text());
    if !action.is_some_and(|a| a.ends_with("/Probe")) {
        return None;
    }

    let body = root.child("Body").unwrap_or(&root);
    let probe = body.child("Probe")?;

    // Types filter: AND match by local name; empty filter matches every device.
    let probe_types = probe
        .child("Types")
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    if !types_match(&probe_types, &dev.types) {
        return None;
    }

    let relates_to = root
        .path(&["Header", "MessageID"])
        .map(|n| n.text())
        .unwrap_or("");
    Some(build_probe_match(relates_to, dev))
}

/// A probe matches when every requested type (by local name) is advertised by
/// the device. An empty request matches all.
fn types_match(probe_types: &str, dev_types: &[String]) -> bool {
    probe_types.split_whitespace().all(|pt| {
        let want = local_name(pt);
        dev_types.iter().any(|dt| local_name(dt) == want)
    })
}

/// Strip a namespace prefix: `dn:NetworkVideoTransmitter` → `NetworkVideoTransmitter`.
fn local_name(qname: &str) -> &str {
    qname.rsplit(':').next().unwrap_or(qname)
}

fn join_escaped(items: &[String]) -> String {
    items
        .iter()
        .map(|s| xml_escape(s).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A running discovery responder. Answering stops when this is dropped.
pub struct DiscoveryResponder {
    addr: std::net::SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl DiscoveryResponder {
    /// Bind a UDP socket at `bind_addr`, optionally join the ONVIF multicast
    /// group, and answer probes advertising `dev` on a background task.
    ///
    /// Pass `join_multicast = true` with `bind_addr = "0.0.0.0:3702"` for real
    /// multicast listening; `false` with an ephemeral port (`"127.0.0.1:0"`) for
    /// a unicast round-trip in tests. The caller must supply reachable XAddrs;
    /// this responder does not create or expose the advertised HTTP service.
    pub async fn spawn(
        bind_addr: &str,
        join_multicast: bool,
        dev: DiscoveredDevice,
    ) -> std::io::Result<Self> {
        let sock = UdpSocket::bind(bind_addr).await?;
        if join_multicast {
            sock.join_multicast_v4(WSD_MULTICAST_ADDR, Ipv4Addr::UNSPECIFIED)?;
        }
        let addr = sock.local_addr()?;
        let (tx, rx) = oneshot::channel::<()>();
        let task = tokio::spawn(serve(sock, dev, rx));
        Ok(Self {
            addr,
            shutdown: Some(tx),
            task: Some(task),
        })
    }

    /// Serve 1..256 ONVIF device announcements using one UDP listener.
    /// `Some(interface)` explicitly joins multicast on a concrete local IPv4
    /// interface; `None` is unicast-only and supports loopback tests.
    /// Each match is a separate bounded datagram. Scoped probes and unsupported
    /// namespaces/types/reply endpoints are declined, not treated as matches.
    /// This is a basic test responder, not complete ONVIF discovery conformance.
    /// No Hello/Bye or Resolve is emitted. Boot time and a fresh sequence UUID
    /// distinguish runs; persisted boot counters are not implemented.
    ///
    /// # Errors
    /// Fails before binding for empty/oversized groups, duplicate/invalid endpoint
    /// metadata or oversized responses, and on bind/interface/multicast failure.
    pub async fn spawn_many(
        bind_addr: &str,
        interface: Option<Ipv4Addr>,
        devices: Vec<DiscoveredDevice>,
    ) -> std::io::Result<Self> {
        validate_members(&devices)?;
        if let Some(ip) = interface
            && (ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_broadcast()
                || !if_addrs::get_if_addrs()?
                    .iter()
                    .any(|nic| nic.ip() == std::net::IpAddr::V4(ip)))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "discovery interface must be a concrete local IPv4 address",
            ));
        }
        let socket = UdpSocket::bind(bind_addr).await?;
        if let Some(ip) = interface {
            socket.join_multicast_v4(WSD_MULTICAST_ADDR, ip)?;
        }
        let addr = socket.local_addr()?;
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(serve_many(socket, devices, rx));
        Ok(Self {
            addr,
            shutdown: Some(tx),
            task: Some(task),
        })
    }

    /// Stop responding and await listener release before reusing its UDP port.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }

    /// The address the responder is bound to (useful when the port was `0`).
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.addr
    }
}

pub(crate) fn validate_members(devices: &[DiscoveredDevice]) -> std::io::Result<()> {
    let mut endpoints = std::collections::HashSet::new();
    let valid_uri = |value: &str| {
        !value.chars().any(|c| c.is_control() || c.is_whitespace())
            && reqwest::Url::parse(value).is_ok()
    };
    if devices.is_empty() || devices.len() > 256 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "shared discovery requires 1..256 devices",
        ));
    }
    for device in devices {
        if device.endpoint.len() > 256
            || !valid_uri(&device.endpoint)
            || !endpoints.insert(&device.endpoint)
            || device.types.is_empty()
            || device.types.len() > 2
            || device
                .types
                .iter()
                .any(|t| !matches!(t.as_str(), "dn:NetworkVideoTransmitter" | "tds:Device"))
            || device.xaddrs.is_empty()
            || device.xaddrs.len() > 4
            || device
                .xaddrs
                .iter()
                .any(|s| !valid_uri(s) || !s.starts_with("http://") || s.len() > 1024)
            || device.scopes.len() > 17
            || device
                .scopes
                .iter()
                .any(|s| s.len() > 1024 || !valid_uri(s))
            || build_sequenced_match(
                &"x".repeat(512),
                device,
                u32::MAX,
                u32::MAX,
                Some("urn:uuid:00000000-0000-0000-0000-000000000000"),
            )
            .len()
                > MAX_DATAGRAM
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid, duplicate or oversized shared discovery metadata",
            ));
        }
    }
    Ok(())
}

// Keep this narrow: validate the supported probe shape without expanding the
// namespace-stripped DOM's compatibility behavior for unrelated SOAP callers.
fn supported_probe_xml(xml: &str, types: &str) -> bool {
    use quick_xml::{
        NsReader,
        events::Event,
        name::{QName, ResolveResult},
    };
    let mut reader = NsReader::from_str(xml);
    let mut path = Vec::new();
    let mut roots = 0;
    loop {
        let Ok((namespace, event)) = reader.read_resolved_event() else {
            return false;
        };
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                let name = element.local_name().as_ref().to_owned();
                let expected = match name.as_str() {
                    "Envelope" | "Header" | "Body" => SOAP_NS,
                    "Action" | "MessageID" | "To" | "ReplyTo" | "Address" => ADDRESSING_NS,
                    "Probe" | "Types" | "Scopes" => DISCOVERY_NS,
                    _ => return false,
                };
                if !matches!(namespace, ResolveResult::Bound(ns) if ns.as_ref() == expected) {
                    return false;
                }
                if path.is_empty() {
                    roots += 1;
                }
                path.push(name.clone());
                if !matches!(
                    path.join("/").as_str(),
                    "Envelope"
                        | "Envelope/Header"
                        | "Envelope/Body"
                        | "Envelope/Header/Action"
                        | "Envelope/Header/MessageID"
                        | "Envelope/Header/To"
                        | "Envelope/Header/ReplyTo"
                        | "Envelope/Header/ReplyTo/Address"
                        | "Envelope/Body/Probe"
                        | "Envelope/Body/Probe/Types"
                        | "Envelope/Body/Probe/Scopes"
                ) {
                    return false;
                }
                if name == "Types" {
                    for ty in types.split_whitespace() {
                        let (ns, local) = reader.resolver().resolve_element(QName(ty));
                        let expected = match local.as_ref() {
                            "NetworkVideoTransmitter" => NVT_NS,
                            "Device" => DEVICE_NS,
                            _ => return false,
                        };
                        if !matches!(ns, ResolveResult::Bound(value) if value.as_ref() == expected)
                        {
                            return false;
                        }
                    }
                }
                if matches!(event, Event::Empty(_)) {
                    path.pop();
                }
            }
            Event::End(_) => {
                if path.pop().is_none() {
                    return false;
                }
            }
            Event::DocType(_) => return false,
            Event::Text(value) if path.is_empty() && !value.xml10_content().trim().is_empty() => {
                return false;
            }
            Event::CData(_) if path.is_empty() => return false,
            Event::Eof => return roots == 1 && path.is_empty(),
            _ => {}
        }
    }
}

fn shared_probe(xml: &str) -> Option<(String, String)> {
    // Reject unsupported/deep structure before building the compatibility DOM.
    if !supported_probe_xml(xml, "") {
        return None;
    }
    let root = XmlNode::parse(xml).ok()?;
    if root.children_named("Header").count() != 1 || root.children_named("Body").count() != 1 {
        return None;
    }
    let header = root.child("Header")?;
    if header.children_named("Action").count() != 1
        || header.children_named("MessageID").count() != 1
        || header.child("Action")?.text() != format!("{DISCOVERY_NS}/Probe")
    {
        return None;
    }
    let id = header.child("MessageID")?.text();
    if id.is_empty()
        || id.len() > 512
        || reqwest::Url::parse(id).is_err()
        || id.chars().any(char::is_whitespace)
    {
        return None;
    }
    if header.children_named("ReplyTo").count() > 1 {
        return None;
    }
    if let Some(reply) = header.child("ReplyTo")
        && (reply.children_named("Address").count() != 1
            || reply.child("Address")?.text() != format!("{ADDRESSING_NS}/role/anonymous"))
    {
        return None;
    }
    let body = root.child("Body")?;
    if body.children_named("Probe").count() != 1 {
        return None;
    }
    let probe = body.child("Probe")?;
    if probe.children_named("Types").count() > 1 || probe.children_named("Scopes").count() > 1 {
        return None;
    }
    if probe
        .child("Scopes")
        .is_some_and(|scope| !scope.text().is_empty() || !scope.attrs.is_empty())
    {
        return None;
    }
    let types = probe.child("Types").map(XmlNode::text).unwrap_or("");
    supported_probe_xml(xml, types).then(|| (id.to_owned(), types.to_owned()))
}

async fn serve_many(
    socket: UdpSocket,
    devices: Vec<DiscoveredDevice>,
    mut shutdown: oneshot::Receiver<()>,
) {
    let mut buf = vec![0; MAX_DATAGRAM + 1];
    let instance = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(u64::from(u32::MAX)) as u32;
    let mut sequence = format!("urn:uuid:{}", new_uuid());
    let mut number = 0u32;
    let mut recent = VecDeque::new();
    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => break,
            received = socket.recv_from(&mut buf) => {
                let Ok((len, source)) = received else { continue };
                if len > MAX_DATAGRAM { continue; }
                let Ok(xml) = std::str::from_utf8(&buf[..len]) else { continue };
                let Some((id, types)) = shared_probe(xml) else { continue };
                let now = tokio::time::Instant::now();
                recent.retain(|(_, _, time)| now.duration_since(*time) < Duration::from_secs(2));
                if recent.iter().any(|(peer, message, _)| *peer == source && message == &id) { continue; }
                if recent.len() == 128 { recent.pop_front(); }
                recent.push_back((source, id.clone(), now));
                let mut pending: Vec<_> = devices.iter().filter(|d| types_match(&types, &d.types))
                    .map(|device| (rand::random_range(0..=500u64), device)).collect();
                pending.sort_by_key(|(delay, _)| *delay);
                for (delay, device) in pending {
                    tokio::select! { biased; _ = &mut shutdown => return,
                        _ = tokio::time::sleep_until(now + Duration::from_millis(delay)) => {} }
                    if number == u32::MAX { number = 0; sequence = format!("urn:uuid:{}", new_uuid()); }
                    number += 1;
                    let response = build_sequenced_match(&id, device, instance, number, Some(&sequence));
                    if response.len() > MAX_DATAGRAM { continue; }
                    tokio::select! { biased; _ = &mut shutdown => return,
                        _ = socket.send_to(response.as_bytes(), source) => {} }
                }
            }
        }
    }
}

impl Drop for DiscoveryResponder {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

/// Receive loop: answer each probe until the shutdown signal fires.
async fn serve(sock: UdpSocket, dev: DiscoveredDevice, mut shutdown: oneshot::Receiver<()>) {
    let mut buf = vec![0u8; 65_535];
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            r = sock.recv_from(&mut buf) => {
                let Ok((len, src)) = r else { continue };
                let Ok(text) = std::str::from_utf8(&buf[..len]) else { continue };
                if let Some(resp) = probe_response(text, &dev) {
                    let _ = sock.send_to(resp.as_bytes(), src).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_probe_declines_unsupported_matching_and_preserves_correlation() {
        let valid = probe("dn:NetworkVideoTransmitter")
            .replace("<wsd:Probe>", &format!("<wsd:Probe xmlns:dn=\"{NVT_NS}\">"));
        assert_eq!(
            shared_probe(&valid),
            Some((
                "urn:uuid:probe-1234".into(),
                "dn:NetworkVideoTransmitter".into()
            ))
        );
        let alias = valid
            .replace("xmlns:dn", "xmlns:camera")
            .replace("dn:Network", "camera:Network");
        assert_eq!(
            shared_probe(&alias).unwrap().1,
            "camera:NetworkVideoTransmitter"
        );
        for invalid in [
            valid.replace(NVT_NS, "urn:not-onvif"),
            valid.replace(DISCOVERY_NS, "urn:not-discovery"),
            valid.replace("</wsd:Probe>", "<wsd:Scopes>onvif://www.onvif.org/name/missing</wsd:Scopes></wsd:Probe>"),
            valid.replace("</wsd:Probe>", "<wsd:Scopes MatchBy=\"urn:unsupported\"/></wsd:Probe>"),
            valid.replace("</s:Header>", "<wsa:MessageID>urn:duplicate</wsa:MessageID></s:Header>"),
            valid.replace("</s:Header>", "<wsa:ReplyTo><wsa:Address>http://192.0.2.1/reflect</wsa:Address></wsa:ReplyTo></s:Header>"),
            format!("{valid}<extra/>"),
        ] { assert!(shared_probe(&invalid).is_none(), "unsupported probe must not match"); }
        let error = validate_members(&[sample_device(), sample_device()]).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("duplicate"));
        let mut oversized = sample_device();
        oversized.scopes = vec!["x".repeat(MAX_DATAGRAM)];
        assert!(
            validate_members(&[oversized])
                .unwrap_err()
                .to_string()
                .contains("oversized")
        );
        let xml = build_sequenced_match(
            "urn:probe&value",
            &sample_device(),
            7,
            9,
            Some("urn:sequence"),
        );
        let root = XmlNode::parse(&xml).unwrap();
        assert_eq!(
            root.path(&["Header", "RelatesTo"]).unwrap().text(),
            "urn:probe&value"
        );
        let sequence = root.path(&["Header", "AppSequence"]).unwrap();
        assert_eq!(sequence.attr("InstanceId"), Some("7"));
        assert_eq!(sequence.attr("MessageNumber"), Some("9"));
        assert_eq!(sequence.attr("SequenceId"), Some("urn:sequence"));
        assert!(xml.contains(&format!("xmlns:dn=\"{NVT_NS}\"")));
    }

    fn sample_device() -> DiscoveredDevice {
        DiscoveredDevice {
            endpoint: "urn:uuid:mock-0000-0000-0000-000000000001".to_string(),
            types: vec![
                "dn:NetworkVideoTransmitter".to_string(),
                "tds:Device".to_string(),
            ],
            scopes: vec!["onvif://www.onvif.org/name/MockCam".to_string()],
            xaddrs: vec!["http://127.0.0.1:8080/onvif/device".to_string()],
        }
    }

    fn probe(types: &str) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
                          xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery">
               <s:Header>
                 <wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</wsa:Action>
                 <wsa:MessageID>urn:uuid:probe-1234</wsa:MessageID>
               </s:Header>
               <s:Body><wsd:Probe><wsd:Types>{types}</wsd:Types></wsd:Probe></s:Body>
             </s:Envelope>"#
        )
    }

    #[test]
    fn build_probe_match_round_trips_through_the_client_parser() {
        let dev = sample_device();
        let xml = build_probe_match("urn:uuid:probe-1234", &dev);
        let root = XmlNode::parse(&xml).expect("valid XML");
        // RelatesTo correlates the answer to the probe.
        assert_eq!(
            root.path(&["Header", "RelatesTo"]).map(|n| n.text()),
            Some("urn:uuid:probe-1234")
        );
        let m = root
            .child("Body")
            .and_then(|b| b.child("ProbeMatches"))
            .and_then(|m| m.child("ProbeMatch"))
            .expect("a ProbeMatch");
        assert_eq!(
            m.path(&["EndpointReference", "Address"]).map(|n| n.text()),
            Some(dev.endpoint.as_str())
        );
        assert_eq!(
            m.child("XAddrs").map(|n| n.text()),
            Some("http://127.0.0.1:8080/onvif/device")
        );
    }

    #[test]
    fn nvt_probe_matches_and_carries_endpoint() {
        let resp = probe_response(&probe("dn:NetworkVideoTransmitter"), &sample_device());
        assert!(
            resp.unwrap()
                .contains("urn:uuid:mock-0000-0000-0000-000000000001")
        );
    }

    #[test]
    fn empty_types_probe_matches_all() {
        assert!(probe_response(&probe(""), &sample_device()).is_some());
    }

    #[test]
    fn mismatched_type_does_not_match() {
        let dev = DiscoveredDevice {
            types: vec!["tds:Device".to_string()], // no NVT
            ..sample_device()
        };
        assert!(probe_response(&probe("dn:NetworkVideoTransmitter"), &dev).is_none());
    }

    #[test]
    fn non_probe_datagram_is_ignored() {
        assert!(probe_response("not xml", &sample_device()).is_none());
        // A well-formed Hello is not a Probe.
        let hello = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing">
               <s:Header><wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Hello</wsa:Action></s:Header>
               <s:Body/></s:Envelope>"#;
        assert!(probe_response(hello, &sample_device()).is_none());
    }

    #[tokio::test]
    async fn unicast_probe_round_trip() {
        let responder = DiscoveryResponder::spawn("127.0.0.1:0", false, sample_device())
            .await
            .unwrap();
        let target = responder.local_addr();

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client
            .send_to(probe("dn:NetworkVideoTransmitter").as_bytes(), target)
            .await
            .unwrap();

        let mut buf = vec![0u8; 65_535];
        let (len, _) = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            client.recv_from(&mut buf),
        )
        .await
        .expect("responder should reply within 1s")
        .unwrap();

        let text = std::str::from_utf8(&buf[..len]).unwrap();
        let root = XmlNode::parse(text).unwrap();
        let addr = root
            .child("Body")
            .and_then(|b| b.child("ProbeMatches"))
            .and_then(|m| m.child("ProbeMatch"))
            .and_then(|m| m.path(&["EndpointReference", "Address"]))
            .map(|n| n.text().to_string());
        assert_eq!(
            addr.as_deref(),
            Some("urn:uuid:mock-0000-0000-0000-000000000001")
        );

        // Dropping the responder stops it answering.
        drop(responder);
    }
}
