//! Multi-device fleet (`feature = "mock-server"`).
//!
//! A [`Fleet`] runs several independent [`MockServer`]s at once — each bound to
//! its own ephemeral port with its own [`DeviceState`] — so a batch client (a
//! fleet health-scan or an NVR onboarding flow using known URLs) can be exercised
//! against a handful of distinct virtual cameras without any hardware.
//!
//! ```no_run
//! # async fn run() -> std::io::Result<()> {
//! let fleet = oxvif::mock::Fleet::start(3).await?;
//! for url in fleet.device_urls() {
//!     let client = oxvif::OnvifClient::new(url);
//!     let info = client.get_device_info().await.unwrap();
//!     println!("{url} → {} {}", info.manufacturer, info.model);
//! }
//! # Ok(()) }
//! ```
//!
//! Each device is a plain `MockServer`, so per-device state, fault injection and
//! auth all work exactly as they do standalone. Dropping the fleet shuts every
//! device down. HTTP endpoints default to loopback; configured members may bind
//! explicitly to a lab interface. Opt-in [`FleetBuilder::discoverable`] shares
//! one basic WS-Discovery responder across the ready members. Scoped probes,
//! Hello/Bye and Resolve are not supported; this is not full conformance.

use crate::discovery::{DiscoveredDevice, new_uuid};
use crate::mock::discovery_responder::{DiscoveryResponder, validate_members};
use std::io;

use crate::mock::server::{MockServer, MockServerBuilder};
use crate::mock::state::DeviceState;

/// A group of independent mock ONVIF devices, each on its own HTTP port.
///
/// Build one with [`Fleet::start`] (defaults) or [`Fleet::builder`] (custom
/// per-device state). Shuts every device down on drop.
pub struct Fleet {
    discovery: Option<DiscoveryResponder>,
    devices: Vec<MockServer>,
}

/// Builder for a [`Fleet`].
#[derive(Default)]
pub struct FleetBuilder {
    members: Vec<Member>,
    enforce_auth: bool,
    discovery: Option<(String, Option<std::net::Ipv4Addr>)>,
}

struct Member {
    server: MockServerBuilder,
    endpoint: String,
    scopes: Vec<String>,
}

impl FleetBuilder {
    /// Enable a shared UDP 3702 listener on an explicitly selected local IPv4
    /// multicast interface. Members must advertise this reachable address.
    /// Startup fails and cleans up HTTP members if discovery cannot start.
    /// Basic unscoped Probe support only; no Hello/Bye, Resolve or full scope matching.
    pub fn discoverable(mut self, interface: std::net::Ipv4Addr) -> Self {
        self.discovery = Some(("0.0.0.0:3702".into(), Some(interface)));
        self
    }

    /// Add one device seeded with a caller-supplied state.
    pub fn device(mut self, state: DeviceState) -> Self {
        let scopes = state.scopes.clone();
        self.members.push(Member {
            server: MockServerBuilder::default().initial_state(state),
            endpoint: format!("urn:uuid:{}", new_uuid()),
            scopes,
        });
        self
    }

    /// Add a configured server with its stable discovery endpoint and scopes.
    /// The fleet-wide authentication policy overrides the supplied builder's
    /// policy. Do not enable per-server discovery: the fleet owns that listener.
    /// Endpoint metadata is validated before any member starts.
    pub fn member(
        mut self,
        server: MockServerBuilder,
        endpoint: String,
        scopes: Vec<String>,
    ) -> Self {
        self.members.push(Member {
            server,
            endpoint,
            scopes,
        });
        self
    }

    /// Add `n` devices seeded from factory defaults but given distinct
    /// identities (`hostname` / `model` / `serial_number` suffixed `-1`, `-2`,
    /// …, continuing past any already-added devices) so a fleet-scanning client
    /// can tell them apart.
    pub fn devices(mut self, n: usize) -> Self {
        let base = self.members.len();
        for i in 0..n {
            self = self.device(distinct_default(base + i + 1));
        }
        self
    }

    /// Enforce WS-Security `PasswordDigest` on every device (default `false`).
    pub fn enforce_auth(mut self, yes: bool) -> Self {
        self.enforce_auth = yes;
        self
    }

    /// Bind and start every device on its configured port (ephemeral by default).
    /// Invalid network/identity metadata is rejected before any listener starts.
    /// If a later bind fails, previously started HTTP members are shut down.
    pub async fn start(self) -> io::Result<Fleet> {
        let mut endpoints = std::collections::HashSet::new();
        let mut announcements = Vec::with_capacity(self.members.len());
        for member in &self.members {
            let (_, advertise) = member.server.network()?;
            if let Some((_, Some(interface))) = &self.discovery
                && (*interface != advertise || interface.is_loopback())
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "fleet discovery interface must match each non-loopback advertised HTTP address",
                ));
            }
            if member.server.has_discovery()
                || member.endpoint.trim().is_empty()
                || member.endpoint.len() > 256
                || member.endpoint.chars().any(char::is_whitespace)
                || !endpoints.insert(&member.endpoint)
                || member.scopes.len() > 17
                || member
                    .scopes
                    .iter()
                    .any(|s| s.len() > 1024 || s.chars().any(char::is_whitespace))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid/duplicate fleet discovery metadata or per-server discovery enabled",
                ));
            }
            announcements.push(DiscoveredDevice {
                endpoint: member.endpoint.clone(),
                types: vec!["dn:NetworkVideoTransmitter".into(), "tds:Device".into()],
                scopes: member.scopes.clone(),
                xaddrs: vec![format!("http://{advertise}:65535/onvif/device")],
            });
        }
        if self.discovery.is_some() {
            validate_members(&announcements)?;
        }
        let mut devices = Vec::with_capacity(self.members.len());
        for member in self.members {
            match member.server.enforce_auth(self.enforce_auth).start().await {
                Ok(server) => devices.push(server),
                Err(error) => {
                    for server in devices {
                        let _ = server.shutdown().await;
                    }
                    return Err(error);
                }
            }
        }
        for (record, device) in announcements.iter_mut().zip(&devices) {
            record.xaddrs = vec![device.device_url().to_owned()];
        }
        let discovery = match self.discovery {
            Some((address, interface)) => {
                match DiscoveryResponder::spawn_many(&address, interface, announcements).await {
                    Ok(responder) => Some(responder),
                    Err(error) => {
                        for server in devices {
                            let _ = server.shutdown().await;
                        }
                        return Err(error);
                    }
                }
            }
            None => None,
        };
        Ok(Fleet { discovery, devices })
    }
}

impl Fleet {
    /// Stop all members and wait for their HTTP listeners to close.
    /// All members are asked to stop even if one shutdown reports an error.
    pub async fn shutdown(mut self) -> io::Result<()> {
        if let Some(discovery) = self.discovery.take() {
            discovery.shutdown().await;
        }
        let mut error = None;
        for server in self.devices {
            if let Err(e) = server.shutdown().await {
                error.get_or_insert(e);
            }
        }
        match error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Bound address of the shared discovery listener, or `None` when disabled.
    pub fn discovery_addr(&self) -> Option<std::net::SocketAddr> {
        self.discovery.as_ref().map(DiscoveryResponder::local_addr)
    }
    /// Start `n` devices with distinct default identities, each on an ephemeral
    /// port.
    pub async fn start(n: usize) -> io::Result<Self> {
        FleetBuilder::default().devices(n).start().await
    }

    /// Configure a fleet before starting it.
    pub fn builder() -> FleetBuilder {
        FleetBuilder::default()
    }

    /// The running devices, in the order they were added.
    pub fn devices(&self) -> &[MockServer] {
        &self.devices
    }

    /// Device-service URLs for every member — feed straight to a batch scanner.
    pub fn device_urls(&self) -> Vec<&str> {
        self.devices.iter().map(|d| d.device_url()).collect()
    }

    /// The `i`-th device, if present.
    pub fn get(&self, i: usize) -> Option<&MockServer> {
        self.devices.get(i)
    }

    /// Number of devices in the fleet.
    pub fn len(&self) -> usize {
        self.devices.len()
    }

    /// Whether the fleet has no devices.
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

/// A factory-default device whose identity fields are suffixed with `n`, so
/// members of a fleet are individually distinguishable.
fn distinct_default(n: usize) -> DeviceState {
    let mut s = DeviceState::default();
    s.hostname = format!("{}-{n}", s.hostname);
    s.info.model = format!("{}-{n}", s.info.model);
    s.info.serial_number = format!("{}-{n}", s.info.serial_number);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OnvifClient;

    #[tokio::test]
    async fn shared_discovery_reaches_each_member_and_releases_listener() {
        let mut builder = Fleet::builder().devices(4);
        builder.discovery = Some(("127.0.0.1:0".into(), None));
        let fleet = builder.start().await.unwrap();
        let address = fleet.discovery_addr().unwrap();
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut buf = vec![0; 65535];
        let mut identities = std::collections::HashMap::new();
        for round in 0..2 {
            let id = format!("urn:fleet-probe-{round}");
            let probe = format!(
                r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:n="http://www.onvif.org/ver10/network/wsdl"><s:Header><a:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</a:Action><a:MessageID>{id}</a:MessageID></s:Header><s:Body><d:Probe><d:Types>n:NetworkVideoTransmitter</d:Types></d:Probe></s:Body></s:Envelope>"#
            );
            socket.send_to(probe.as_bytes(), address).await.unwrap();
            // Duplicate datagrams must not create an extra wave of responses.
            socket.send_to(probe.as_bytes(), address).await.unwrap();
            let mut found = std::collections::HashSet::new();
            for _ in 0..4 {
                let (len, _) = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    socket.recv_from(&mut buf),
                )
                .await
                .unwrap()
                .unwrap();
                let root =
                    crate::soap::XmlNode::parse(std::str::from_utf8(&buf[..len]).unwrap()).unwrap();
                assert_eq!(root.path(&["Header", "RelatesTo"]).unwrap().text(), id);
                let sequence: u32 = root
                    .path(&["Header", "AppSequence"])
                    .unwrap()
                    .attr("MessageNumber")
                    .unwrap()
                    .parse()
                    .unwrap();
                let record = root.path(&["Body", "ProbeMatches", "ProbeMatch"]).unwrap();
                let endpoint = record
                    .path(&["EndpointReference", "Address"])
                    .unwrap()
                    .text()
                    .to_owned();
                let url = record.child("XAddrs").unwrap().text();
                assert!(
                    fleet.device_urls().contains(&url),
                    "advertise an actual member URL"
                );
                assert!(found.insert(endpoint.clone()), "one response per member");
                let info = OnvifClient::new(url).get_device_info().await.unwrap();
                let member = fleet
                    .devices()
                    .iter()
                    .find(|d| d.device_url() == url)
                    .unwrap();
                assert_eq!(
                    info.serial_number,
                    member.device().read().info.serial_number
                );
                if round == 0 {
                    identities.insert(endpoint, (url.to_owned(), sequence));
                } else {
                    let (old_url, old_sequence) = identities.get(&endpoint).unwrap();
                    assert_eq!(old_url, url);
                    assert!(sequence > *old_sequence);
                }
            }
            assert_eq!(found.len(), 4);
        }
        assert_eq!(identities.len(), 4);
        fleet.shutdown().await.unwrap();
        let rebound = tokio::net::UdpSocket::bind(address).await.unwrap();
        assert_eq!(rebound.local_addr().unwrap(), address);
    }

    #[tokio::test]
    async fn discovery_bind_failure_rolls_back_ready_http_members() {
        let occupied = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let reservation = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let mut builder = Fleet::builder().member(
            MockServer::builder().port(port),
            "urn:rollback-device".into(),
            vec![],
        );
        builder.discovery = Some((occupied.local_addr().unwrap().to_string(), None));
        let error = builder
            .start()
            .await
            .err()
            .expect("occupied UDP port must fail fleet startup");
        assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
        let released = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        assert_eq!(released.local_addr().unwrap().port(), port);
    }

    #[tokio::test]
    async fn configured_members_keep_identity_state_and_release_ports() {
        let mut builder = Fleet::builder();
        for i in 1..=4 {
            let mut state = DeviceState::default();
            state.info.serial_number = format!("FLEET-CONFIG-{i}");
            state.hostname = format!("configured-{i}");
            builder = builder.member(
                MockServer::builder()
                    .initial_state(state)
                    .bind_ip(std::net::Ipv4Addr::LOCALHOST)
                    .advertise_ip(std::net::Ipv4Addr::LOCALHOST),
                format!("urn:uuid:fleet-{i}"),
                Vec::new(),
            );
        }
        let fleet = builder.start().await.unwrap();
        let ports: Vec<_> = fleet.devices().iter().map(MockServer::port).collect();
        for (i, server) in fleet.devices().iter().enumerate() {
            let info = OnvifClient::new(server.device_url())
                .get_device_info()
                .await
                .unwrap();
            assert_eq!(info.serial_number, format!("FLEET-CONFIG-{}", i + 1));
            assert_eq!(server.base_url(), format!("http://127.0.0.1:{}", ports[i]));
        }
        OnvifClient::new(fleet.get(0).unwrap().device_url())
            .set_hostname("isolated-change")
            .await
            .unwrap();
        assert_eq!(
            fleet.get(0).unwrap().device().read().hostname,
            "isolated-change"
        );
        assert_eq!(
            fleet.get(1).unwrap().device().read().hostname,
            "configured-2"
        );
        fleet.shutdown().await.unwrap();
        for port in ports {
            let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .unwrap();
            assert_eq!(listener.local_addr().unwrap().port(), port);
        }
    }

    #[tokio::test]
    async fn failed_member_startup_rolls_back_and_invalid_network_never_starts() {
        let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let reservation = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let first_port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let result = Fleet::builder()
            .member(
                MockServer::builder().port(first_port),
                "urn:first".into(),
                vec![],
            )
            .member(
                MockServer::builder().port(occupied.local_addr().unwrap().port()),
                "urn:second".into(),
                vec![],
            )
            .start()
            .await;
        let error = result.err().expect("occupied port must fail startup");
        assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
        let released = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, first_port))
            .await
            .unwrap();
        assert_eq!(released.local_addr().unwrap().port(), first_port);
        let error = MockServer::builder()
            .bind_ip(std::net::Ipv4Addr::UNSPECIFIED)
            .start()
            .await
            .err()
            .expect("wildcard bind needs advertisement");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("explicit advertise IP"));
        let error = Fleet::builder()
            .member(MockServer::builder(), "urn:same".into(), vec![])
            .member(MockServer::builder(), "urn:same".into(), vec![])
            .start()
            .await
            .err()
            .expect("duplicate endpoint must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(
            error
                .to_string()
                .contains("duplicate fleet discovery metadata")
        );
    }

    #[tokio::test]
    async fn fleet_runs_distinct_devices_on_separate_ports() {
        let fleet = Fleet::start(3).await.unwrap();
        assert_eq!(fleet.len(), 3);
        assert!(!fleet.is_empty());

        // Distinct ephemeral ports.
        let ports: std::collections::HashSet<u16> =
            fleet.devices().iter().map(|d| d.port()).collect();
        assert_eq!(ports.len(), 3, "each device must bind its own port");

        // Distinct identities, reachable over real HTTP.
        let mut serials = Vec::new();
        for url in fleet.device_urls() {
            let client = OnvifClient::new(url);
            let info = client.get_device_info().await.unwrap();
            serials.push(info.serial_number);
        }
        serials.sort();
        serials.dedup();
        assert_eq!(
            serials.len(),
            3,
            "each device must report a distinct serial"
        );
    }

    #[tokio::test]
    async fn fleet_state_is_per_device() {
        let fleet = Fleet::start(2).await.unwrap();
        let a = OnvifClient::new(fleet.get(0).unwrap().device_url());
        let b = OnvifClient::new(fleet.get(1).unwrap().device_url());

        a.set_hostname("cam-a").await.unwrap();
        // Writing device 0 must not leak into device 1.
        assert_eq!(fleet.get(0).unwrap().device().read().hostname, "cam-a");
        assert_ne!(fleet.get(1).unwrap().device().read().hostname, "cam-a");
        let hb = b.get_hostname().await.unwrap();
        assert_ne!(hb.name.as_deref(), Some("cam-a"));
    }

    #[tokio::test]
    async fn builder_mixes_explicit_and_default_devices() {
        let custom = DeviceState {
            hostname: "acme-cam".into(),
            ..Default::default()
        };
        let fleet = Fleet::builder()
            .device(custom)
            .devices(1)
            .start()
            .await
            .unwrap();
        assert_eq!(fleet.len(), 2);
        // The explicit device kept its seeded hostname; the default one didn't.
        assert_eq!(fleet.get(0).unwrap().device().read().hostname, "acme-cam");
        assert_ne!(fleet.get(1).unwrap().device().read().hostname, "acme-cam");
    }
}
