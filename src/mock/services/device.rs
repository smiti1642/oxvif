use crate::mock::helpers::{resp_empty, soap};
use crate::mock::state::SharedState;
use crate::mock::xml_parse::{extract_all_tags, extract_attr, extract_tag};

const NS: &str = r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#;

// ── Stateful Get responses ──────────────────────────────────────────────────

/// The mock's clock. **All six components come from the real current time.**
///
/// Until 0.15 the time of day was live but the date was the literal
/// `2026-04-15`, so the mock reported a timestamp that drifted further into the
/// past every day — 106 days by the time it was noticed. Nothing failed: the
/// hours/minutes/seconds looked right, which is exactly why nobody read the
/// date. What it *did* do was make `HealthCheck`'s clock-skew check warn on a
/// perfectly healthy mock, so the first thing anyone trying the health check
/// without a camera saw was a false positive.
///
/// The conversion is `soap::security::unix_secs_to_ymd_hms`, the same one behind
/// the WS-Security `Created` timestamp, so there is one implementation and the
/// ISO-8601 tests pin it.
pub fn resp_system_date_and_time(state: &SharedState) -> String {
    let s = state.read();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day, hours, mins, secs) =
        crate::soap::security::unix_secs_to_ymd_hms(now as i64);
    let dst = if s.daylight_savings { "true" } else { "false" };
    soap(
        NS,
        &format!(
            r#"<tds:GetSystemDateAndTimeResponse>
          <tds:SystemDateAndTime>
            <tt:DateTimeType>NTP</tt:DateTimeType>
            <tt:DaylightSavings>{dst}</tt:DaylightSavings>
            <tt:TimeZone><tt:TZ>{tz}</tt:TZ></tt:TimeZone>
            <tt:UTCDateTime>
              <tt:Time><tt:Hour>{hours}</tt:Hour><tt:Minute>{mins}</tt:Minute><tt:Second>{secs}</tt:Second></tt:Time>
              <tt:Date><tt:Year>{year}</tt:Year><tt:Month>{month}</tt:Month><tt:Day>{day}</tt:Day></tt:Date>
            </tt:UTCDateTime>
          </tds:SystemDateAndTime>
        </tds:GetSystemDateAndTimeResponse>"#,
            tz = s.timezone,
        ),
    )
}

pub fn resp_device_info(state: &SharedState) -> String {
    let s = state.read();
    soap(
        NS,
        &format!(
            r#"<tds:GetDeviceInformationResponse>
          <tds:Manufacturer>{}</tds:Manufacturer>
          <tds:Model>{}</tds:Model>
          <tds:FirmwareVersion>{}</tds:FirmwareVersion>
          <tds:SerialNumber>{}</tds:SerialNumber>
          <tds:HardwareId>{}</tds:HardwareId>
        </tds:GetDeviceInformationResponse>"#,
            s.info.manufacturer,
            s.info.model,
            s.info.firmware_version,
            s.info.serial_number,
            s.info.hardware_id,
        ),
    )
}

pub fn resp_hostname(state: &SharedState) -> String {
    let s = state.read();
    let dhcp = if s.hostname_from_dhcp {
        "true"
    } else {
        "false"
    };
    soap(
        NS,
        &format!(
            r#"<tds:GetHostnameResponse>
          <tds:HostnameInformation>
            <tt:FromDHCP>{dhcp}</tt:FromDHCP>
            <tt:Name>{name}</tt:Name>
          </tds:HostnameInformation>
        </tds:GetHostnameResponse>"#,
            name = s.hostname,
        ),
    )
}

pub fn resp_ntp(state: &SharedState) -> String {
    let s = state.read();
    let dhcp = if s.ntp.from_dhcp { "true" } else { "false" };
    let servers: String = s
        .ntp.servers
        .iter()
        .map(|srv| {
            format!(
                r#"<tt:NTPManual><tt:Type>DNS</tt:Type><tt:DNSname>{srv}</tt:DNSname></tt:NTPManual>"#
            )
        })
        .collect();
    soap(
        NS,
        &format!(
            r#"<tds:GetNTPResponse>
          <tds:NTPInformation>
            <tt:FromDHCP>{dhcp}</tt:FromDHCP>
            {servers}
          </tds:NTPInformation>
        </tds:GetNTPResponse>"#
        ),
    )
}

pub fn resp_scopes(state: &SharedState) -> String {
    let s = state.read();
    let items: String = s
        .scopes
        .iter()
        .map(|scope| {
            format!(
                r#"<tds:Scopes><tt:ScopeAttribute>Fixed</tt:ScopeAttribute><tt:ScopeItem>{scope}</tt:ScopeItem></tds:Scopes>"#
            )
        })
        .collect();
    soap(
        NS,
        &format!("<tds:GetScopesResponse>{items}</tds:GetScopesResponse>"),
    )
}

pub fn resp_users(state: &SharedState) -> String {
    let s = state.read();
    let items: String = s
        .users
        .iter()
        .map(|u| {
            format!(
                r#"<tds:User><tt:Username>{}</tt:Username><tt:UserLevel>{}</tt:UserLevel></tds:User>"#,
                u.username, u.level,
            )
        })
        .collect();
    soap(
        NS,
        &format!("<tds:GetUsersResponse>{items}</tds:GetUsersResponse>"),
    )
}

pub fn resp_dns(state: &SharedState) -> String {
    let s = state.read();
    let dhcp = if s.dns.from_dhcp { "true" } else { "false" };
    let servers: String = s
        .dns.servers
        .iter()
        .map(|srv| {
            format!(
                r#"<tt:DNSManual><tt:Type>IPv4</tt:Type><tt:IPv4Address>{srv}</tt:IPv4Address></tt:DNSManual>"#
            )
        })
        .collect();
    soap(
        NS,
        &format!(
            r#"<tds:GetDNSResponse>
          <tds:DNSInformation>
            <tt:FromDHCP>{dhcp}</tt:FromDHCP>
            {servers}
          </tds:DNSInformation>
        </tds:GetDNSResponse>"#
        ),
    )
}

pub fn resp_network_default_gateway(state: &SharedState) -> String {
    let s = state.read();
    let addrs: String = s
        .gateway_ipv4
        .iter()
        .map(|a| format!("<tt:IPv4Address>{a}</tt:IPv4Address>"))
        .collect();
    soap(
        NS,
        &format!(
            r#"<tds:GetNetworkDefaultGatewayResponse>
          <tds:NetworkGateway>{addrs}</tds:NetworkGateway>
        </tds:GetNetworkDefaultGatewayResponse>"#
        ),
    )
}

pub fn resp_discovery_mode(state: &SharedState) -> String {
    let s = state.read();
    soap(
        NS,
        &format!(
            r#"<tds:GetDiscoveryModeResponse>
          <tds:DiscoveryMode>{}</tds:DiscoveryMode>
        </tds:GetDiscoveryModeResponse>"#,
            s.discovery_mode,
        ),
    )
}

/// Audit §3 item 1.3 — the getter above has always been state-driven while the
/// dispatcher answered this with `resp_empty`. Exactly the shape of the reported
/// Media2 profile bug: a live getter over a discarded write.
///
/// Only the two values `tt:DiscoveryMode` defines are accepted. A device that
/// silently stored `"Maybe"` and echoed it back would round-trip while
/// describing a state no real camera can be in.
pub fn handle_set_discovery_mode(state: &SharedState, body: &str) -> String {
    let mode = extract_tag(body, "DiscoveryMode").unwrap_or_default();
    if mode != "Discoverable" && mode != "NonDiscoverable" {
        return crate::mock::helpers::resp_soap_fault(
            "env:Sender",
            &format!("InvalidDiscoveryMode-5551: {mode} is not a tt:DiscoveryMode"),
        );
    }
    state.modify(|s| {
        s.discovery_mode = mode.clone();
        eprintln!("    [STATE] discovery mode: {mode}");
    });
    resp_empty("tds", "SetDiscoveryModeResponse")
}

// ── Set handlers (mutate state) ─────────────────────────────────────────────

pub fn handle_set_hostname(state: &SharedState, body: &str) -> String {
    if let Some(name) = extract_tag(body, "Name") {
        state.modify(|s| {
            s.hostname = name;
            eprintln!("    [STATE] hostname updated");
        });
    }
    resp_empty("tds", "SetHostnameResponse")
}

pub fn handle_set_ntp(state: &SharedState, body: &str) -> String {
    // Always honour the FromDHCP toggle, even when the servers list is
    // empty (which it is when the client switches to DHCP mode).
    let servers = extract_all_tags(body, "DNSname");
    let from_dhcp = extract_tag(body, "FromDHCP")
        .map(|v| v == "true")
        .unwrap_or(false);
    state.modify(|s| {
        s.ntp.servers = servers;
        s.ntp.from_dhcp = from_dhcp;
        eprintln!(
            "    [STATE] NTP updated: dhcp={} servers={:?}",
            s.ntp.from_dhcp, s.ntp.servers
        );
    });
    resp_empty("tds", "SetNTPResponse")
}

pub fn handle_set_dns(state: &SharedState, body: &str) -> String {
    let servers = extract_all_tags(body, "IPv4Address");
    let from_dhcp = extract_tag(body, "FromDHCP")
        .map(|v| v == "true")
        .unwrap_or(false);
    state.modify(|s| {
        s.dns.servers = servers;
        s.dns.from_dhcp = from_dhcp;
        eprintln!(
            "    [STATE] DNS updated: dhcp={} servers={:?}",
            s.dns.from_dhcp, s.dns.servers
        );
    });
    resp_empty("tds", "SetDNSResponse")
}

pub fn handle_set_scopes(state: &SharedState, body: &str) -> String {
    // ONVIF SetScopes ships each URI directly as <tds:Scopes>URI</tds:Scopes>.
    // The GetScopesResponse format is richer (<Scopes><ScopeDef/><ScopeItem/></Scopes>),
    // but that doesn't apply to writes — we were looking at the wrong tag
    // here before, which is why oxdm's name/location edits appeared to
    // succeed but never reflected.
    let scopes = extract_all_tags(body, "Scopes");
    if !scopes.is_empty() {
        state.modify(|s| {
            s.scopes = scopes;
            eprintln!("    [STATE] scopes updated: {:?}", s.scopes);
        });
    }
    resp_empty("tds", "SetScopesResponse")
}

pub fn handle_set_system_date_and_time(state: &SharedState, body: &str) -> String {
    let tz = extract_tag(body, "TZ");
    let dst = extract_tag(body, "DaylightSavings");
    if tz.is_some() || dst.is_some() {
        state.modify(|s| {
            if let Some(tz) = tz {
                s.timezone = tz;
                eprintln!("    [STATE] timezone updated");
            }
            if let Some(dst) = dst {
                s.daylight_savings = dst == "true";
            }
        });
    }
    resp_empty("tds", "SetSystemDateAndTimeResponse")
}

pub fn handle_create_users(state: &SharedState, body: &str) -> String {
    // Scope extraction to the <tds:CreateUsers> block so we don't pick up
    // the <wsse:Username>/<wsse:Password> tags from the WS-Security header
    // sent alongside authenticated requests.
    let inner = extract_tag(body, "CreateUsers").unwrap_or_default();
    let usernames = extract_all_tags(&inner, "Username");
    let passwords = extract_all_tags(&inner, "Password");
    let levels = extract_all_tags(&inner, "UserLevel");
    state.modify(|s| {
        for (i, username) in usernames.into_iter().enumerate() {
            let level = levels.get(i).cloned().unwrap_or_else(|| "User".to_string());
            let password = passwords.get(i).cloned().unwrap_or_default();
            eprintln!("    [STATE] user created: {username} ({level})");
            s.users.push(crate::mock::state::MockUser {
                username,
                level,
                password,
            });
        }
    });
    resp_empty("tds", "CreateUsersResponse")
}

pub fn handle_delete_users(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "DeleteUsers").unwrap_or_default();
    let usernames = extract_all_tags(&inner, "Username");
    state.modify(|s| {
        for name in &usernames {
            s.users.retain(|u| u.username != *name);
            eprintln!("    [STATE] user deleted: {name}");
        }
    });
    resp_empty("tds", "DeleteUsersResponse")
}

pub fn handle_set_user(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "SetUser").unwrap_or_default();
    let username = extract_tag(&inner, "Username");
    let level = extract_tag(&inner, "UserLevel");
    let password = extract_tag(&inner, "Password");
    if let Some(username) = username {
        state.modify(|s| {
            if let Some(user) = s.users.iter_mut().find(|u| u.username == username) {
                if let Some(l) = &level {
                    user.level = l.clone();
                }
                if let Some(p) = &password {
                    user.password = p.clone();
                }
                eprintln!("    [STATE] user updated: {username}");
            }
        });
    }
    resp_empty("tds", "SetUserResponse")
}

// ── Static responses (not stateful yet) ─────────────────────────────────────

/// `tds:Capabilities` — the device-level "which services exist and where",
/// which is a **different operation** from each service's
/// `GetServiceCapabilities` ([`resp_service_capabilities`] for this one).
///
/// `Device/{Network,System,IO,Security}` and
/// `Events/WSSubscriptionPolicySupport` were absent until 0.15. Two consequences,
/// both silent:
///
/// - `DeviceCapabilities::{network,system,io,security}` had **no mock coverage
///   at all** — those parsers were exercised only by hand-written unit fixtures,
///   the arrangement that let the `AFModes` defect survive. Real cameras send
///   these; the mock has to.
/// - The device-level type uses bare `bool`, so every absent element parsed as
///   `false` while `GetServiceCapabilities` said `true`. The health check's
///   capability cross-check saw six such mismatches on its first run against
///   this mock.
///
/// Every value here is **chosen to agree with [`resp_service_capabilities`]** on
/// the fourteen attributes the two operations both carry, so the mock is a
/// self-consistent device. Changing one side without the other is what the
/// cross-check exists to catch — including here.
pub fn resp_capabilities(base: &str) -> String {
    soap(
        NS,
        &format!(
            r#"<tds:GetCapabilitiesResponse>
          <tds:Capabilities>
            <tt:Device>
              <tt:XAddr>{base}/onvif/device</tt:XAddr>
              <tt:Network>
                <tt:IPFilter>false</tt:IPFilter>
                <tt:ZeroConfiguration>false</tt:ZeroConfiguration>
                <tt:IPVersion6>true</tt:IPVersion6>
                <tt:DynDNS>false</tt:DynDNS>
              </tt:Network>
              <tt:System>
                <tt:DiscoveryResolve>false</tt:DiscoveryResolve>
                <tt:DiscoveryBye>true</tt:DiscoveryBye>
                <tt:RemoteDiscovery>false</tt:RemoteDiscovery>
                <tt:SystemBackup>false</tt:SystemBackup>
                <tt:SystemLogging>true</tt:SystemLogging>
                <tt:FirmwareUpgrade>true</tt:FirmwareUpgrade>
              </tt:System>
              <tt:IO>
                <tt:InputConnectors>1</tt:InputConnectors>
                <tt:RelayOutputs>2</tt:RelayOutputs>
              </tt:IO>
              <tt:Security>
                <tt:TLS1.2>true</tt:TLS1.2>
                <tt:OnboardKeyGeneration>false</tt:OnboardKeyGeneration>
                <tt:AccessPolicyConfig>false</tt:AccessPolicyConfig>
                <tt:X.509Token>false</tt:X.509Token>
                <tt:UsernameToken>true</tt:UsernameToken>
              </tt:Security>
            </tt:Device>
            <tt:Events>
              <tt:XAddr>{base}/onvif/events</tt:XAddr>
              <tt:WSSubscriptionPolicySupport>true</tt:WSSubscriptionPolicySupport>
              <tt:WSPullPointSupport>true</tt:WSPullPointSupport>
            </tt:Events>
            <tt:Imaging><tt:XAddr>{base}/onvif/imaging</tt:XAddr></tt:Imaging>
            <tt:Media>
              <tt:XAddr>{base}/onvif/media</tt:XAddr>
              <tt:StreamingCapabilities>
                <tt:RTPMulticast>false</tt:RTPMulticast>
                <tt:RTP_TCP>true</tt:RTP_TCP>
                <tt:RTP_RTSP_TCP>true</tt:RTP_RTSP_TCP>
              </tt:StreamingCapabilities>
            </tt:Media>
            <tt:PTZ><tt:XAddr>{base}/onvif/ptz</tt:XAddr></tt:PTZ>
            <tt:Extension>
              <tt:Recording><tt:XAddr>{base}/onvif/recording</tt:XAddr></tt:Recording>
              <tt:Search><tt:XAddr>{base}/onvif/search</tt:XAddr></tt:Search>
              <tt:Replay><tt:XAddr>{base}/onvif/replay</tt:XAddr></tt:Replay>
              <!-- Media2 is advertised via GetServices (ver20/media/wsdl), not
                   here — the Capabilities Media2 extension is non-standard. -->
            </tt:Extension>
          </tds:Capabilities>
        </tds:GetCapabilitiesResponse>"#
        ),
    )
}

pub fn resp_services(base: &str) -> String {
    soap(
        NS,
        &format!(
            r#"<tds:GetServicesResponse>
          <tds:Service><tds:Namespace>http://www.onvif.org/ver10/device/wsdl</tds:Namespace><tds:XAddr>{base}/onvif/device</tds:XAddr><tds:Version><tt:Major>2</tt:Major><tt:Minor>6</tt:Minor></tds:Version></tds:Service>
          <tds:Service><tds:Namespace>http://www.onvif.org/ver10/media/wsdl</tds:Namespace><tds:XAddr>{base}/onvif/media</tds:XAddr><tds:Version><tt:Major>2</tt:Major><tt:Minor>6</tt:Minor></tds:Version></tds:Service>
          <tds:Service><tds:Namespace>http://www.onvif.org/ver20/media/wsdl</tds:Namespace><tds:XAddr>{base}/onvif/media2</tds:XAddr><tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version></tds:Service>
          <tds:Service><tds:Namespace>http://www.onvif.org/ver20/ptz/wsdl</tds:Namespace><tds:XAddr>{base}/onvif/ptz</tds:XAddr><tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version></tds:Service>
          <tds:Service><tds:Namespace>http://www.onvif.org/ver20/imaging/wsdl</tds:Namespace><tds:XAddr>{base}/onvif/imaging</tds:XAddr><tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version></tds:Service>
          <tds:Service><tds:Namespace>http://www.onvif.org/ver10/recording/wsdl</tds:Namespace><tds:XAddr>{base}/onvif/recording</tds:XAddr><tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version></tds:Service>
          <tds:Service><tds:Namespace>http://www.onvif.org/ver10/search/wsdl</tds:Namespace><tds:XAddr>{base}/onvif/search</tds:XAddr><tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version></tds:Service>
          <tds:Service><tds:Namespace>http://www.onvif.org/ver10/replay/wsdl</tds:Namespace><tds:XAddr>{base}/onvif/replay</tds:XAddr><tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version></tds:Service>
        </tds:GetServicesResponse>"#
        ),
    )
}

pub fn resp_network_interfaces(state: &SharedState) -> String {
    let s = state.read();
    let i = &s.interface;
    let enabled = if i.enabled { "true" } else { "false" };
    let dhcp = if i.ipv4_from_dhcp { "true" } else { "false" };
    soap(
        NS,
        &format!(
            r#"<tds:GetNetworkInterfacesResponse>
          <tds:NetworkInterfaces token="{token}">
            <tt:Enabled>{enabled}</tt:Enabled>
            <tt:Info>
              <tt:Name>{name}</tt:Name>
              <tt:HwAddress>{mac}</tt:HwAddress>
              <tt:MTU>{mtu}</tt:MTU>
            </tt:Info>
            <tt:IPv4>
              <tt:Enabled>true</tt:Enabled>
              <tt:Config>
                <tt:Manual>
                  <tt:Address>{address}</tt:Address>
                  <tt:PrefixLength>{prefix}</tt:PrefixLength>
                </tt:Manual>
                <tt:DHCP>{dhcp}</tt:DHCP>
              </tt:Config>
            </tt:IPv4>
          </tds:NetworkInterfaces>
        </tds:GetNetworkInterfacesResponse>"#,
            token = i.token,
            name = i.name,
            mac = i.mac,
            mtu = i.mtu,
            address = i.ipv4_address,
            prefix = i.ipv4_prefix_length,
        ),
    )
}

pub fn handle_set_network_interfaces(state: &SharedState, body: &str) -> String {
    // Read the new values out of the SetNetworkInterfaces body.
    let token = extract_tag(body, "InterfaceToken").unwrap_or_default();
    let enabled = extract_tag(body, "Enabled").map(|v| v == "true");
    let dhcp = extract_tag(body, "FromDHCP").map(|v| v == "true");
    let address = extract_tag(body, "Address");
    let prefix: Option<u32> = extract_tag(body, "PrefixLength").and_then(|p| p.parse().ok());
    // Audit §3 item 1.8. The four reads above landed and this one did not, so
    // the state log said "interface updated" and `GetNetworkInterfaces` reported
    // the old MTU — a *partial* write, which reads as wired from every angle
    // except writing a value and reading it back. Found by
    // `tests/mock_roundtrip.rs` on its first run.
    let mtu: Option<u32> = extract_tag(body, "MTU").and_then(|m| m.parse().ok());

    state.modify(|s| {
        if !token.is_empty() && token != s.interface.token {
            // Mock keeps a single interface; ignore writes to other tokens.
            return;
        }
        if let Some(e) = enabled {
            s.interface.enabled = e;
        }
        if let Some(d) = dhcp {
            s.interface.ipv4_from_dhcp = d;
        }
        if let Some(a) = address {
            s.interface.ipv4_address = a;
        }
        if let Some(p) = prefix {
            s.interface.ipv4_prefix_length = p;
        }
        if let Some(m) = mtu {
            s.interface.mtu = m;
        }
        eprintln!(
            "    [STATE] interface updated: dhcp={} addr={} /{} mtu={}",
            s.interface.ipv4_from_dhcp,
            s.interface.ipv4_address,
            s.interface.ipv4_prefix_length,
            s.interface.mtu,
        );
    });

    // Always report no-reboot-needed so the client UI flow stays deterministic.
    soap(
        NS,
        r#"<tds:SetNetworkInterfacesResponse>
          <tds:RebootNeeded>false</tds:RebootNeeded>
        </tds:SetNetworkInterfacesResponse>"#,
    )
}

pub fn resp_network_protocols(state: &SharedState) -> String {
    let s = state.read();
    let items: String = s
        .protocols
        .iter()
        .map(|p| {
            let enabled = if p.enabled { "true" } else { "false" };
            let ports: String = p
                .ports
                .iter()
                .map(|n| format!("<tt:Port>{n}</tt:Port>"))
                .collect();
            format!(
                r#"<tds:NetworkProtocols><tt:Name>{name}</tt:Name><tt:Enabled>{enabled}</tt:Enabled>{ports}</tds:NetworkProtocols>"#,
                name = p.name,
            )
        })
        .collect();
    soap(
        NS,
        &format!("<tds:GetNetworkProtocolsResponse>{items}</tds:GetNetworkProtocolsResponse>"),
    )
}

pub fn handle_set_network_protocols(state: &SharedState, body: &str) -> String {
    // SetNetworkProtocols sends one or more NetworkProtocols blocks; we
    // pull Name/Enabled/Port out flat and zip them. Each block ships
    // exactly one Port in normal ONVIF traffic, so 1:1 zipping is safe.
    let names = extract_all_tags(body, "Name");
    let enableds = extract_all_tags(body, "Enabled");
    let ports = extract_all_tags(body, "Port");

    if names.is_empty() {
        return resp_empty("tds", "SetNetworkProtocolsResponse");
    }

    state.modify(|s| {
        for (i, name) in names.iter().enumerate() {
            let enabled = enableds.get(i).map(|v| v == "true").unwrap_or(true);
            let port: Option<u32> = ports.get(i).and_then(|p| p.parse().ok());
            // Update existing protocol or insert if camera doesn't have it.
            if let Some(p) = s
                .protocols
                .iter_mut()
                .find(|p| p.name.eq_ignore_ascii_case(name))
            {
                p.enabled = enabled;
                if let Some(port) = port {
                    p.ports = vec![port];
                }
            } else {
                s.protocols.push(crate::mock::state::NetworkProtocolState {
                    name: name.clone(),
                    enabled,
                    ports: port.map(|p| vec![p]).unwrap_or_default(),
                });
            }
            eprintln!("    [STATE] protocol {name}: enabled={enabled} port={port:?}");
        }
    });
    resp_empty("tds", "SetNetworkProtocolsResponse")
}

pub fn handle_set_network_default_gateway(state: &SharedState, body: &str) -> String {
    let addrs = extract_all_tags(body, "IPv4Address");
    state.modify(|s| {
        s.gateway_ipv4 = addrs;
        eprintln!("    [STATE] gateway updated: {:?}", s.gateway_ipv4);
    });
    resp_empty("tds", "SetNetworkDefaultGatewayResponse")
}

pub fn resp_system_log() -> String {
    soap(
        NS,
        r#"<tds:GetSystemLogResponse>
          <tds:SystemLog>
            <tt:String>2026-04-15 12:00:00 mock system started</tt:String>
          </tds:SystemLog>
        </tds:GetSystemLogResponse>"#,
    )
}

pub fn resp_relay_outputs(state: &SharedState) -> String {
    let s = state.read();
    let entries: String = s
        .relay_outputs
        .iter()
        .map(|r| {
            format!(
                r#"<tds:RelayOutputs token="{}"><tt:Properties><tt:Mode>{}</tt:Mode><tt:DelayTime>{}</tt:DelayTime><tt:IdleState>{}</tt:IdleState></tt:Properties></tds:RelayOutputs>"#,
                r.token, r.mode, r.delay_time, r.idle_state
            )
        })
        .collect();
    soap(
        NS,
        &format!("<tds:GetRelayOutputsResponse>{entries}</tds:GetRelayOutputsResponse>"),
    )
}

pub fn handle_set_relay_output_state(state: &SharedState, body: &str) -> String {
    let token = extract_tag(body, "RelayOutputToken").unwrap_or_default();
    let logical_state = extract_tag(body, "LogicalState").unwrap_or_default();
    let exists = state.read().relay_outputs.iter().any(|r| r.token == token);
    if !exists {
        return crate::mock::helpers::resp_soap_fault(
            "s:Sender",
            &format!("Unknown RelayOutput token: {token}"),
        );
    }
    state.modify(|s| {
        if let Some(r) = s.relay_outputs.iter_mut().find(|r| r.token == token) {
            r.logical_state = logical_state.clone();
        }
        s.pending_io_events
            .push(crate::mock::state::PendingIoEvent {
                kind: "RelayOutput",
                token: token.clone(),
                logical_state: logical_state.clone(),
            });
    });
    resp_empty("tds", "SetRelayOutputStateResponse")
}

pub fn handle_set_relay_output_settings(state: &SharedState, body: &str) -> String {
    let token = extract_tag(body, "RelayOutputToken").unwrap_or_default();
    let mode = extract_tag(body, "Mode").unwrap_or_default();
    let delay_time = extract_tag(body, "DelayTime").unwrap_or_default();
    let idle_state = extract_tag(body, "IdleState").unwrap_or_default();
    let exists = state.read().relay_outputs.iter().any(|r| r.token == token);
    if !exists {
        return crate::mock::helpers::resp_soap_fault(
            "s:Sender",
            &format!("Unknown RelayOutput token: {token}"),
        );
    }
    state.modify(|s| {
        if let Some(r) = s.relay_outputs.iter_mut().find(|r| r.token == token) {
            if !mode.is_empty() {
                r.mode = mode.clone();
            }
            if !delay_time.is_empty() {
                r.delay_time = delay_time.clone();
            }
            if !idle_state.is_empty() {
                r.idle_state = idle_state.clone();
            }
        }
    });
    resp_empty("tds", "SetRelayOutputSettingsResponse")
}

pub fn resp_digital_inputs(state: &SharedState) -> String {
    let s = state.read();
    let entries: String = s
        .digital_inputs
        .iter()
        .map(|d| {
            format!(
                r#"<tds:DigitalInputs token="{}" IdleState="{}"/>"#,
                d.token, d.idle_state
            )
        })
        .collect();
    soap(
        NS,
        &format!("<tds:GetDigitalInputsResponse>{entries}</tds:GetDigitalInputsResponse>"),
    )
}

pub fn resp_send_auxiliary_command() -> String {
    soap(
        NS,
        r#"<tds:SendAuxiliaryCommandResponse>
          <tds:AuxiliaryCommandResponse>OK</tds:AuxiliaryCommandResponse>
        </tds:SendAuxiliaryCommandResponse>"#,
    )
}

// ── Storage ───────────────────────────────────────────────────────────────────
//
// Until 0.15 this family was one static fixture emitting a single `SD_01`
// entry with a `LocalPath` and nothing else, and `SetStorageConfiguration`
// was `resp_empty` — audit §5 (Tier 3) for the dead round-trip and §6
// (Tier 4) for the credential fields `StorageConfiguration` parses and the
// mock never fed: `StorageUri` and `User/UserName`.
//
// Optional elements are omitted when empty rather than emitted blank, which
// is what a real device sends. **That distinction is not observable through
// this client** and no test asserts it: `StorageConfiguration` models these
// as `String` via `unwrap_or_default()`, so an omitted element and an empty
// one both parse to `""`. Measured — a renderer changed to emit
// `<tt:LocalPath></tt:LocalPath>` for `CIFS_01` reddens nothing. Recorded so
// the next reader does not mistake the shape for a tested guarantee; making
// it one would mean `Option<String>` on the parser, which is a public API
// change and not this commit's business.

fn render_storage(e: &crate::mock::state::StorageEntry) -> String {
    let mut inner = String::new();
    if !e.local_path.is_empty() {
        inner.push_str(&format!("<tt:LocalPath>{}</tt:LocalPath>", e.local_path));
    }
    if !e.storage_uri.is_empty() {
        inner.push_str(&format!("<tt:StorageUri>{}</tt:StorageUri>", e.storage_uri));
    }
    if !e.user.is_empty() {
        inner.push_str(&format!(
            "<tt:User><tt:UserName>{}</tt:UserName></tt:User>",
            e.user
        ));
    }
    format!(
        "<tds:StorageConfigurations token=\"{token}\">\
           <tt:Data type=\"{ty}\">{inner}</tt:Data>\
         </tds:StorageConfigurations>",
        token = e.token,
        ty = e.storage_type,
    )
}

pub fn resp_storage_configurations(state: &SharedState) -> String {
    let items: String = state.read().storage.iter().map(render_storage).collect();
    soap(
        &format!("{NS} xmlns:tt=\"http://www.onvif.org/ver10/schema\""),
        &format!(
            "<tds:GetStorageConfigurationsResponse>{items}</tds:GetStorageConfigurationsResponse>"
        ),
    )
}

/// `SetStorageConfiguration` — create when the token attribute is absent,
/// update in place when it names an existing entry, fault when it names one
/// that does not exist.
///
/// A device that silently created an entry under a token the caller invented
/// would make a typo indistinguishable from a successful update, so an unknown
/// token is refused rather than treated as a create.
pub fn handle_set_storage_configuration(state: &SharedState, body: &str) -> String {
    let token = extract_attr(body, "StorageConfiguration", "token").unwrap_or_default();
    let storage_type = extract_attr(body, "Data", "type").unwrap_or_default();
    if storage_type.is_empty() {
        return crate::mock::helpers::resp_soap_fault(
            "env:Sender",
            "NoStorageType-STOR-5801: Data/@type is required",
        );
    }
    let local_path = extract_tag(body, "LocalPath").unwrap_or_default();
    let storage_uri = extract_tag(body, "StorageUri").unwrap_or_default();
    let user = extract_tag(body, "UserName").unwrap_or_default();

    if token.is_empty() {
        // Create. Tokens are never reused, matching `ProfilesState`.
        state.modify(|s| {
            let token = format!("Storage_{:03}", s.storage.len() + 1);
            eprintln!("    [STATE] storage created: {token}");
            s.storage.push(crate::mock::state::StorageEntry {
                token,
                storage_type: storage_type.clone(),
                local_path: local_path.clone(),
                storage_uri: storage_uri.clone(),
                user: user.clone(),
            });
        });
        return resp_empty("tds", "SetStorageConfigurationResponse");
    }

    let known = state.read().storage.iter().any(|e| e.token == token);
    if !known {
        return crate::mock::helpers::resp_soap_fault(
            "ter:InvalidArgVal",
            &format!("NoSuchStorage-STOR-5802: {token}"),
        );
    }
    state.modify(|s| {
        if let Some(e) = s.storage.iter_mut().find(|e| e.token == token) {
            e.storage_type = storage_type.clone();
            e.local_path = local_path.clone();
            e.storage_uri = storage_uri.clone();
            e.user = user.clone();
            eprintln!("    [STATE] storage updated: {token}");
        }
    });
    resp_empty("tds", "SetStorageConfigurationResponse")
}

pub fn resp_system_uris(base: &str) -> String {
    soap(
        &format!("{NS} xmlns:tt=\"http://www.onvif.org/ver10/schema\""),
        &format!(
            r#"<tds:GetSystemUrisResponse>
          <tds:SystemLogUris>
            <tt:SystemLogUri><tt:Uri>{base}/syslog</tt:Uri><tt:LogType>System</tt:LogType></tt:SystemLogUri>
          </tds:SystemLogUris>
          <tds:SupportInfoUri>{base}/support</tds:SupportInfoUri>
          <tds:SystemBackupUri>{base}/backup</tds:SystemBackupUri>
        </tds:GetSystemUrisResponse>"#
        ),
    )
}

pub fn resp_start_firmware_upgrade(base: &str) -> String {
    soap(
        NS,
        &format!(
            r#"<tds:StartFirmwareUpgradeResponse>
          <tds:UploadUri>{base}/upload/firmware</tds:UploadUri>
          <tds:UploadDelay>PT0S</tds:UploadDelay>
          <tds:ExpectedDownTime>PT30S</tds:ExpectedDownTime>
        </tds:StartFirmwareUpgradeResponse>"#
        ),
    )
}

pub fn resp_start_system_restore(base: &str) -> String {
    soap(
        NS,
        &format!(
            r#"<tds:StartSystemRestoreResponse>
          <tds:UploadUri>{base}/upload/restore</tds:UploadUri>
          <tds:ExpectedDownTime>PT30S</tds:ExpectedDownTime>
        </tds:StartSystemRestoreResponse>"#
        ),
    )
}

pub fn resp_system_reboot() -> String {
    soap(
        NS,
        r#"<tds:SystemRebootResponse>
          <tds:Message>Rebooting in 30 seconds</tds:Message>
        </tds:SystemRebootResponse>"#,
    )
}

// ── GetServiceCapabilities ───────────────────────────────────────────────────

/// `tds:DeviceServiceCapabilities` — four children: `Network`, `Security`,
/// `System`, and the optional `Misc`.
///
/// Three things here exist to be parsed against, not just to look plausible:
///
/// - `TLS1.2` and `X.509Token` carry **dots** in the attribute name. They are
///   legal XML names and illegal Rust identifiers, so a parser must match the
///   dotted string even though its struct field cannot be spelled that way.
/// - `DiscoveryNotSupported` / `NetworkConfigNotSupported` /
///   `UserConfigNotSupported` are **negative-sense**: absent means the feature
///   *is* supported. Two are omitted here and one is present-and-false, so a
///   parser that inverts them wrongly cannot pass by accident.
/// - `Misc/@AuxiliaryCommands` is the discoverable list behind
///   `SendAuxiliaryCommand`; the values match what `resp_send_auxiliary_command`
///   accepts.
pub fn resp_service_capabilities() -> String {
    soap(
        NS,
        r#"<tds:GetServiceCapabilitiesResponse>
          <tds:Capabilities>
            <tds:Network IPFilter="false"
                         ZeroConfiguration="false"
                         IPVersion6="true"
                         DynDNS="false"
                         Dot11Configuration="false"
                         HostnameFromDHCP="false"
                         NTP="1"
                         DHCPv6="false"/>
            <tds:Security TLS1.0="false"
                          TLS1.1="false"
                          TLS1.2="true"
                          OnboardKeyGeneration="false"
                          AccessPolicyConfig="false"
                          DefaultAccessPolicy="false"
                          Dot1X="false"
                          RemoteUserHandling="false"
                          X.509Token="false"
                          SAMLToken="false"
                          KerberosToken="false"
                          UsernameToken="true"
                          HttpDigest="true"
                          RELToken="false"
                          MaxUsers="8"
                          MaxUserNameLength="32"
                          MaxPasswordLength="64"/>
            <tds:System DiscoveryResolve="false"
                        DiscoveryBye="true"
                        RemoteDiscovery="false"
                        SystemBackup="false"
                        SystemLogging="true"
                        HttpFirmwareUpgrade="true"
                        HttpSystemBackup="false"
                        HttpSystemLogging="false"
                        HttpSupportInformation="false"
                        StorageConfiguration="true"
                        MaxStorageConfigurations="2"
                        UserConfigNotSupported="false"/>
            <tds:Misc AuxiliaryCommands="tt:Wiper|On tt:Wiper|Off tt:IRLamp|On tt:IRLamp|Off tt:IRLamp|Auto"/>
          </tds:Capabilities>
        </tds:GetServiceCapabilitiesResponse>"#,
    )
}
