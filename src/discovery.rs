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

// ── Public API ────────────────────────────────────────────────────────────────

/// Send a WS-Discovery `Probe` and collect all `ProbeMatch` responses.
///
/// Binds to a random local UDP port, sends a single `Probe` to the ONVIF
/// multicast group (`239.255.255.250:3702`), and returns every device that
/// replies within `timeout_dur`. Duplicate responses (same endpoint UUID) are
/// suppressed.
///
/// Returns an empty `Vec` on any I/O error — treat failures as "no devices
/// found" rather than hard errors.
pub async fn probe(timeout_dur: Duration) -> Vec<DiscoveredDevice> {
    probe_inner(timeout_dur).await.unwrap_or_default()
}

// ── Internal implementation ───────────────────────────────────────────────────

async fn probe_inner(timeout_dur: Duration) -> std::io::Result<Vec<DiscoveredDevice>> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_multicast_ttl_v4(4)?;

    let message_id = new_uuid();
    let xml = build_probe(&message_id);
    socket.send_to(xml.as_bytes(), WSD_MULTICAST).await?;

    let mut buf = vec![0u8; 65535];
    let mut devices: Vec<DiscoveredDevice> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
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
                    for d in collect_probe_matches(&root) {
                        if seen.insert(d.endpoint.clone()) {
                            devices.push(d);
                        }
                    }
                }
            }
            _ => break,
        }
    }

    Ok(devices)
}

fn collect_probe_matches(root: &XmlNode) -> Vec<DiscoveredDevice> {
    let body = root.child("Body").unwrap_or(root);
    let matches = body.child("ProbeMatches").unwrap_or(body);
    matches
        .children_named("ProbeMatch")
        .map(DiscoveredDevice::from_xml)
        .collect()
}

fn build_probe(message_id: &str) -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<s:Envelope"#,
            r#" xmlns:s="http://www.w3.org/2003/05/soap-envelope""#,
            r#" xmlns:wsa="http://www.w3.org/2005/08/addressing""#,
            r#" xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery""#,
            r#" xmlns:dn="http://www.onvif.org/ver10/network/wsdl">"#,
            r#"<s:Header>"#,
            r#"<wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</wsa:Action>"#,
            r#"<wsa:MessageID>uuid:{}</wsa:MessageID>"#,
            r#"<wsa:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</wsa:To>"#,
            r#"</s:Header>"#,
            r#"<s:Body>"#,
            r#"<wsd:Probe><wsd:Types>dn:NetworkVideoTransmitter</wsd:Types></wsd:Probe>"#,
            r#"</s:Body>"#,
            r#"</s:Envelope>"#,
        ),
        message_id
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
    fn test_build_probe_is_valid_xml() {
        let xml = build_probe("test-uuid-1234");
        assert!(
            XmlNode::parse(&xml).is_ok(),
            "build_probe output should be valid XML"
        );
        assert!(xml.contains("NetworkVideoTransmitter"));
        assert!(xml.contains("test-uuid-1234"));
    }

    #[test]
    fn test_new_uuid_has_five_parts() {
        let uuid = new_uuid();
        let parts: Vec<&str> = uuid.split('-').collect();
        assert_eq!(parts.len(), 5, "UUID should have 5 dash-separated parts");
    }
}
