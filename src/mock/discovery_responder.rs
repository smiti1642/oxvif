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
//! UDP multicast is flaky in CI / containers (and port 3702 is a shared,
//! single-listener resource), so the reusable core here is split into pure
//! functions ([`build_probe_match`], [`probe_response`]) plus a spawnable
//! listener; tests drive the pure path and a loopback **unicast** round-trip,
//! never multicast.
//!
//! Simplification: only the `Probe`'s `<Types>` filter is honoured (an AND
//! match by local name, empty = match all) — the rarely-used `<Scopes>` filter
//! is ignored, matching how ONVIF discovery clients actually probe.

use std::net::Ipv4Addr;

use tokio::net::UdpSocket;
use tokio::sync::oneshot;

use crate::discovery::{DiscoveredDevice, new_uuid};
use crate::soap::XmlNode;
use crate::types::xml_escape;

/// ONVIF WS-Discovery multicast group.
const WSD_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);

/// Build a `ProbeMatches` envelope advertising `dev`, correlated to the probe's
/// `MessageID` via `<wsa:RelatesTo>`.
///
/// Uses the same legacy WS-Addressing (2004/08) + WS-Discovery (2005/04)
/// namespaces `discovery::build_probe` sends and ONVIF Device Manager uses —
/// the wire format the widest range of camera firmware accepts.
pub(crate) fn build_probe_match(relates_to: &str, dev: &DiscoveredDevice) -> String {
    let types = join_escaped(&dev.types);
    let scopes = join_escaped(&dev.scopes);
    let xaddrs = join_escaped(&dev.xaddrs);
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<s:Envelope"#,
            r#" xmlns:s="http://www.w3.org/2003/05/soap-envelope""#,
            r#" xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing""#,
            r#" xmlns:wsd="http://schemas.xmlsoap.org/ws/2005/04/discovery">"#,
            r#"<s:Header>"#,
            r#"<wsa:MessageID>urn:uuid:{msg_id}</wsa:MessageID>"#,
            r#"<wsa:RelatesTo>{relates_to}</wsa:RelatesTo>"#,
            r#"<wsa:To>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</wsa:To>"#,
            r#"<wsa:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/ProbeMatches</wsa:Action>"#,
            r#"<wsd:AppSequence InstanceId="1" MessageNumber="1"/>"#,
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
}

impl DiscoveryResponder {
    /// Bind a UDP socket at `bind_addr`, optionally join the ONVIF multicast
    /// group, and answer probes advertising `dev` on a background task.
    ///
    /// Pass `join_multicast = true` with `bind_addr = "0.0.0.0:3702"` for real
    /// LAN discoverability; `false` with an ephemeral port (`"127.0.0.1:0"`) for
    /// a unicast round-trip in tests.
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
        tokio::spawn(serve(sock, dev, rx));
        Ok(Self {
            addr,
            shutdown: Some(tx),
        })
    }

    /// The address the responder is bound to (useful when the port was `0`).
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.addr
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
