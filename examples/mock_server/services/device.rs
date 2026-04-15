use crate::helpers::{resp_empty, soap};
use crate::state::SharedState;
use crate::xml_parse::{extract_all_tags, extract_tag};

const NS: &str = r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#;

// ── Stateful Get responses ──────────────────────────────────────────────────

pub fn resp_system_date_and_time(state: &SharedState) -> String {
    let s = state.read().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now % 60;
    let mins = (now / 60) % 60;
    let hours = (now / 3600) % 24;
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
              <tt:Date><tt:Year>2026</tt:Year><tt:Month>4</tt:Month><tt:Day>15</tt:Day></tt:Date>
            </tt:UTCDateTime>
          </tds:SystemDateAndTime>
        </tds:GetSystemDateAndTimeResponse>"#,
            tz = s.timezone,
        ),
    )
}

pub fn resp_device_info(state: &SharedState) -> String {
    let s = state.read().unwrap();
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
            s.manufacturer, s.model, s.firmware_version, s.serial_number, s.hardware_id,
        ),
    )
}

pub fn resp_hostname(state: &SharedState) -> String {
    let s = state.read().unwrap();
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
    let s = state.read().unwrap();
    let dhcp = if s.ntp_from_dhcp { "true" } else { "false" };
    let servers: String = s
        .ntp_servers
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
    let s = state.read().unwrap();
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
    let s = state.read().unwrap();
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
    let s = state.read().unwrap();
    let dhcp = if s.dns_from_dhcp { "true" } else { "false" };
    let servers: String = s
        .dns_servers
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
    let s = state.read().unwrap();
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
    let s = state.read().unwrap();
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

// ── Set handlers (mutate state) ─────────────────────────────────────────────

pub fn handle_set_hostname(state: &SharedState, body: &str) -> String {
    if let Some(name) = extract_tag(body, "Name") {
        state.write().unwrap().hostname = name;
        eprintln!("    [STATE] hostname updated");
    }
    resp_empty("tds", "SetHostnameResponse")
}

pub fn handle_set_ntp(state: &SharedState, body: &str) -> String {
    let servers = extract_all_tags(body, "DNSname");
    if !servers.is_empty() {
        let mut s = state.write().unwrap();
        s.ntp_servers = servers;
        s.ntp_from_dhcp = extract_tag(body, "FromDHCP")
            .map(|v| v == "true")
            .unwrap_or(false);
        eprintln!("    [STATE] NTP updated: {:?}", s.ntp_servers);
    }
    resp_empty("tds", "SetNTPResponse")
}

pub fn handle_set_dns(state: &SharedState, body: &str) -> String {
    let servers = extract_all_tags(body, "IPv4Address");
    if !servers.is_empty() {
        let mut s = state.write().unwrap();
        s.dns_servers = servers;
        s.dns_from_dhcp = extract_tag(body, "FromDHCP")
            .map(|v| v == "true")
            .unwrap_or(false);
        eprintln!("    [STATE] DNS updated: {:?}", s.dns_servers);
    }
    resp_empty("tds", "SetDNSResponse")
}

pub fn handle_set_scopes(state: &SharedState, body: &str) -> String {
    let scopes = extract_all_tags(body, "ScopeItem");
    if !scopes.is_empty() {
        state.write().unwrap().scopes = scopes;
        eprintln!("    [STATE] scopes updated");
    }
    resp_empty("tds", "SetScopesResponse")
}

pub fn handle_set_system_date_and_time(state: &SharedState, body: &str) -> String {
    if let Some(tz) = extract_tag(body, "TZ") {
        state.write().unwrap().timezone = tz;
        eprintln!("    [STATE] timezone updated");
    }
    if let Some(dst) = extract_tag(body, "DaylightSavings") {
        state.write().unwrap().daylight_savings = dst == "true";
    }
    resp_empty("tds", "SetSystemDateAndTimeResponse")
}

pub fn handle_create_users(state: &SharedState, body: &str) -> String {
    let usernames = extract_all_tags(body, "Username");
    let levels = extract_all_tags(body, "UserLevel");
    let mut s = state.write().unwrap();
    for (u, l) in usernames.into_iter().zip(levels.into_iter()) {
        eprintln!("    [STATE] user created: {u} ({l})");
        s.users.push(crate::state::MockUser {
            username: u,
            level: l,
        });
    }
    resp_empty("tds", "CreateUsersResponse")
}

pub fn handle_delete_users(state: &SharedState, body: &str) -> String {
    let usernames = extract_all_tags(body, "Username");
    let mut s = state.write().unwrap();
    for name in &usernames {
        s.users.retain(|u| u.username != *name);
        eprintln!("    [STATE] user deleted: {name}");
    }
    resp_empty("tds", "DeleteUsersResponse")
}

pub fn handle_set_user(state: &SharedState, body: &str) -> String {
    if let Some(username) = extract_tag(body, "Username") {
        let level = extract_tag(body, "UserLevel");
        let mut s = state.write().unwrap();
        if let Some(user) = s.users.iter_mut().find(|u| u.username == username) {
            if let Some(l) = level {
                user.level = l;
            }
            eprintln!("    [STATE] user updated: {username}");
        }
    }
    resp_empty("tds", "SetUserResponse")
}

// ── Static responses (not stateful yet) ─────────────────────────────────────

pub fn resp_capabilities(base: &str) -> String {
    soap(
        NS,
        &format!(
            r#"<tds:GetCapabilitiesResponse>
          <tds:Capabilities>
            <tt:Device><tt:XAddr>{base}/onvif/device</tt:XAddr></tt:Device>
            <tt:Media>
              <tt:XAddr>{base}/onvif/media</tt:XAddr>
              <tt:StreamingCapabilities>
                <tt:RTPMulticast>false</tt:RTPMulticast>
                <tt:RTP_TCP>true</tt:RTP_TCP>
                <tt:RTP_RTSP_TCP>true</tt:RTP_RTSP_TCP>
              </tt:StreamingCapabilities>
            </tt:Media>
            <tt:PTZ><tt:XAddr>{base}/onvif/ptz</tt:XAddr></tt:PTZ>
            <tt:Imaging><tt:XAddr>{base}/onvif/imaging</tt:XAddr></tt:Imaging>
            <tt:Events>
              <tt:XAddr>{base}/onvif/events</tt:XAddr>
              <tt:WSPullPointSupport>true</tt:WSPullPointSupport>
            </tt:Events>
            <tt:Extension>
              <tt:Recording><tt:XAddr>{base}/onvif/recording</tt:XAddr></tt:Recording>
              <tt:Search><tt:XAddr>{base}/onvif/search</tt:XAddr></tt:Search>
              <tt:Replay><tt:XAddr>{base}/onvif/replay</tt:XAddr></tt:Replay>
              <tt:Media2><tt:XAddr>{base}/onvif/media2</tt:XAddr></tt:Media2>
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

pub fn resp_network_interfaces() -> String {
    soap(
        NS,
        r#"<tds:GetNetworkInterfacesResponse>
          <tds:NetworkInterfaces token="eth0">
            <tt:Enabled>true</tt:Enabled>
            <tt:Info>
              <tt:Name>eth0</tt:Name>
              <tt:HwAddress>00:11:22:33:44:55</tt:HwAddress>
              <tt:MTU>1500</tt:MTU>
            </tt:Info>
            <tt:IPv4>
              <tt:Enabled>true</tt:Enabled>
              <tt:Config>
                <tt:FromDHCP>false</tt:FromDHCP>
                <tt:Manual>
                  <tt:Address>192.168.1.100</tt:Address>
                  <tt:PrefixLength>24</tt:PrefixLength>
                </tt:Manual>
              </tt:Config>
            </tt:IPv4>
          </tds:NetworkInterfaces>
        </tds:GetNetworkInterfacesResponse>"#,
    )
}

pub fn resp_set_network_interfaces() -> String {
    soap(
        NS,
        r#"<tds:SetNetworkInterfacesResponse>
          <tds:RebootNeeded>false</tds:RebootNeeded>
        </tds:SetNetworkInterfacesResponse>"#,
    )
}

pub fn resp_network_protocols() -> String {
    soap(
        NS,
        r#"<tds:GetNetworkProtocolsResponse>
          <tds:NetworkProtocols><tt:Name>HTTP</tt:Name><tt:Enabled>true</tt:Enabled><tt:Port>80</tt:Port></tds:NetworkProtocols>
          <tds:NetworkProtocols><tt:Name>HTTPS</tt:Name><tt:Enabled>true</tt:Enabled><tt:Port>443</tt:Port></tds:NetworkProtocols>
          <tds:NetworkProtocols><tt:Name>RTSP</tt:Name><tt:Enabled>true</tt:Enabled><tt:Port>554</tt:Port></tds:NetworkProtocols>
        </tds:GetNetworkProtocolsResponse>"#,
    )
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

pub fn resp_relay_outputs() -> String {
    soap(
        NS,
        r#"<tds:GetRelayOutputsResponse>
          <tds:RelayOutputs token="RelayOutput_1">
            <tt:Properties>
              <tt:Mode>Bistable</tt:Mode>
              <tt:DelayTime>PT0S</tt:DelayTime>
              <tt:IdleState>open</tt:IdleState>
            </tt:Properties>
          </tds:RelayOutputs>
        </tds:GetRelayOutputsResponse>"#,
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

pub fn resp_storage_configurations() -> String {
    soap(
        &format!("{NS} xmlns:tt=\"http://www.onvif.org/ver10/schema\""),
        r#"<tds:GetStorageConfigurationsResponse>
          <tds:StorageConfigurations token="SD_01">
            <tt:Data type="LocalStorage"><tt:LocalPath>/mnt/sd</tt:LocalPath></tt:Data>
          </tds:StorageConfigurations>
        </tds:GetStorageConfigurationsResponse>"#,
    )
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
        </tds:GetSystemUrisResponse>"#
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
