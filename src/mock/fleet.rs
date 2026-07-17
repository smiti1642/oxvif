//! Multi-device fleet (`feature = "mock-server"`).
//!
//! A [`Fleet`] runs several independent [`MockServer`]s at once — each bound to
//! its own ephemeral port with its own [`DeviceState`] — so a batch client (a
//! fleet health-scan, a discovery UI, an NVR onboarding flow) can be exercised
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
//! device down. WS-Discovery is deliberately out of scope here: the ONVIF
//! multicast port `3702` is shared per host, so at most one server can answer
//! probes — address fleet members by their URLs instead.

use std::io;

use crate::mock::server::{MockServer, MockServerBuilder};
use crate::mock::state::DeviceState;

/// A group of independent mock ONVIF devices, each on its own ephemeral port.
///
/// Build one with [`Fleet::start`] (defaults) or [`Fleet::builder`] (custom
/// per-device state). Shuts every device down on drop.
pub struct Fleet {
    devices: Vec<MockServer>,
}

/// Builder for a [`Fleet`].
#[derive(Default)]
pub struct FleetBuilder {
    states: Vec<DeviceState>,
    enforce_auth: bool,
}

impl FleetBuilder {
    /// Add one device seeded with a caller-supplied state.
    pub fn device(mut self, state: DeviceState) -> Self {
        self.states.push(state);
        self
    }

    /// Add `n` devices seeded from factory defaults but given distinct
    /// identities (`hostname` / `model` / `serial_number` suffixed `-1`, `-2`,
    /// …, continuing past any already-added devices) so a fleet-scanning client
    /// can tell them apart.
    pub fn devices(mut self, n: usize) -> Self {
        let base = self.states.len();
        for i in 0..n {
            self.states.push(distinct_default(base + i + 1));
        }
        self
    }

    /// Enforce WS-Security `PasswordDigest` on every device (default `false`).
    pub fn enforce_auth(mut self, yes: bool) -> Self {
        self.enforce_auth = yes;
        self
    }

    /// Bind and start every device on its own ephemeral port.
    pub async fn start(self) -> io::Result<Fleet> {
        let mut devices = Vec::with_capacity(self.states.len());
        for state in self.states {
            let server = MockServerBuilder::default()
                .initial_state(state)
                .enforce_auth(self.enforce_auth)
                .start()
                .await?;
            devices.push(server);
        }
        Ok(Fleet { devices })
    }
}

impl Fleet {
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
