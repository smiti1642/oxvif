//! ONVIF mock server — handles every operation exercised by `full-workflow`.
//!
//! Run the server, then point the camera example at it:
//!
//! ```sh
//! # Terminal 1 — start the mock server (default port 8080)
//! cargo run --example mock_server
//!
//! # Terminal 2 — run the full workflow against it
//! ONVIF_URL=http://127.0.0.1:18080/onvif/device \
//! ONVIF_USERNAME=admin \
//! ONVIF_PASSWORD=password \
//! cargo run --example camera -- full-workflow
//! ```
//!
//! An optional port number can be supplied as the first argument:
//!
//! ```sh
//! cargo run --example mock_server -- 19090
//! ```
//!
//! All responses are stateless. Write operations (CreateProfile, DeleteProfile,
//! etc.) return plausible canned responses without actually persisting state.

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

// ── State ─────────────────────────────────────────────────────────────────────

struct MockState {
    /// Base URL of this server, e.g. `"http://127.0.0.1:8080"`.
    base: String,
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(18080);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let base = format!("http://{addr}");
    let state = Arc::new(MockState { base: base.clone() });

    let app = Router::new()
        // Single catch-all route — dispatch is done on the SOAPAction header.
        .route("/*path", post(handle_soap))
        .with_state(state);

    let listener = TcpListener::bind(addr).await.expect("bind failed");
    println!("ONVIF mock server listening on {base}");
    println!("  ONVIF_URL={base}/onvif/device");
    println!();

    axum::serve(listener, app).await.expect("serve failed");
}

// ── Request handler ───────────────────────────────────────────────────────────

async fn handle_soap(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    // Body is accepted but not used — this is a stateless mock.
    _body: axum::body::Bytes,
) -> impl IntoResponse {
    let action = extract_action(&headers).unwrap_or_default();
    eprintln!("  → {action}");

    let xml = dispatch(&action, &state.base);

    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "application/soap+xml; charset=utf-8",
        )],
        xml,
    )
}

/// Extract the SOAPAction from the `Content-Type` header.
///
/// ONVIF uses SOAP 1.2 style: `application/soap+xml; ...; action="<URI>"`.
fn extract_action(headers: &HeaderMap) -> Option<String> {
    let ct = headers.get("content-type")?.to_str().ok()?;
    let start = ct.find("action=\"")? + 8;
    let end = ct[start..].find('"')? + start;
    Some(ct[start..end].to_string())
}

// ── Dispatch on full action URI ───────────────────────────────────────────────

fn dispatch(action: &str, base: &str) -> String {
    match action {
        // ── Device ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime" => {
            resp_system_date_and_time()
        }
        "http://www.onvif.org/ver10/device/wsdl/GetCapabilities" => resp_capabilities(base),
        "http://www.onvif.org/ver10/device/wsdl/GetServices" => resp_services(base),
        "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation" => resp_device_info(),
        "http://www.onvif.org/ver10/device/wsdl/GetHostname" => resp_hostname(),
        "http://www.onvif.org/ver10/device/wsdl/GetNTP" => resp_ntp(),
        "http://www.onvif.org/ver10/device/wsdl/GetScopes" => resp_scopes(),
        "http://www.onvif.org/ver10/device/wsdl/GetUsers" => resp_users(),
        "http://www.onvif.org/ver10/device/wsdl/CreateUsers" => {
            resp_empty("tds", "CreateUsersResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/DeleteUsers" => {
            resp_empty("tds", "DeleteUsersResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetUser" => resp_empty("tds", "SetUserResponse"),
        "http://www.onvif.org/ver10/device/wsdl/GetNetworkInterfaces" => resp_network_interfaces(),
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkInterfaces" => {
            resp_set_network_interfaces()
        }
        "http://www.onvif.org/ver10/device/wsdl/GetNetworkProtocols" => resp_network_protocols(),
        "http://www.onvif.org/ver10/device/wsdl/GetDNS" => resp_dns(),
        "http://www.onvif.org/ver10/device/wsdl/SetDNS" => resp_empty("tds", "SetDNSResponse"),
        "http://www.onvif.org/ver10/device/wsdl/GetNetworkDefaultGateway" => {
            resp_network_default_gateway()
        }
        "http://www.onvif.org/ver10/device/wsdl/GetSystemLog" => resp_system_log(),
        "http://www.onvif.org/ver10/device/wsdl/GetRelayOutputs" => resp_relay_outputs(),
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputState" => {
            resp_empty("tds", "SetRelayOutputStateResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputSettings" => {
            resp_empty("tds", "SetRelayOutputSettingsResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkProtocols" => {
            resp_empty("tds", "SetNetworkProtocolsResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault" => {
            resp_empty("tds", "SetSystemFactoryDefaultResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/GetStorageConfigurations" => {
            resp_storage_configurations()
        }
        "http://www.onvif.org/ver10/device/wsdl/SetStorageConfiguration" => {
            resp_empty("tds", "SetStorageConfigurationResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/GetSystemUris" => resp_system_uris(base),
        "http://www.onvif.org/ver10/device/wsdl/GetDiscoveryMode" => resp_discovery_mode(),
        "http://www.onvif.org/ver10/device/wsdl/SetDiscoveryMode" => {
            resp_empty("tds", "SetDiscoveryModeResponse")
        }

        // ── Media1 ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/media/wsdl/GetProfiles" => resp_profiles(),
        "http://www.onvif.org/ver10/media/wsdl/GetProfile" => resp_profile(),
        "http://www.onvif.org/ver10/media/wsdl/GetStreamUri" => resp_stream_uri(),
        "http://www.onvif.org/ver10/media/wsdl/GetSnapshotUri" => resp_snapshot_uri(),
        "http://www.onvif.org/ver10/media/wsdl/CreateProfile" => resp_create_profile(),
        "http://www.onvif.org/ver10/media/wsdl/DeleteProfile" => {
            resp_empty("trt", "DeleteProfileResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoSources" => resp_video_sources(),
        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurations" => {
            resp_video_source_configurations()
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurations" => {
            resp_video_encoder_configurations()
        }
        "http://www.onvif.org/ver10/media/wsdl/GetAudioSources" => resp_audio_sources(),
        "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurations" => {
            resp_audio_encoder_configurations()
        }
        "http://www.onvif.org/ver10/media/wsdl/GetOSDs" => resp_osds(),
        "http://www.onvif.org/ver10/media/wsdl/AddVideoEncoderConfiguration"
        | "http://www.onvif.org/ver10/media/wsdl/RemoveVideoEncoderConfiguration"
        | "http://www.onvif.org/ver10/media/wsdl/AddVideoSourceConfiguration"
        | "http://www.onvif.org/ver10/media/wsdl/RemoveVideoSourceConfiguration" => {
            resp_empty("trt", "ConfigurationResponse")
        }

        // ── Media2 ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver20/media/wsdl/GetProfiles" => resp_profiles_media2(),
        "http://www.onvif.org/ver20/media/wsdl/GetStreamUri" => resp_stream_uri_media2(),
        "http://www.onvif.org/ver20/media/wsdl/GetSnapshotUri" => resp_snapshot_uri_media2(),
        "http://www.onvif.org/ver20/media/wsdl/DeleteProfile" => {
            resp_empty("tr2", "DeleteProfileResponse")
        }

        // ── PTZ ───────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver20/ptz/wsdl/GetStatus" => resp_ptz_status(),
        "http://www.onvif.org/ver20/ptz/wsdl/GetPresets" => resp_ptz_presets(),
        "http://www.onvif.org/ver20/ptz/wsdl/GetNodes" => resp_ptz_nodes(),
        "http://www.onvif.org/ver20/ptz/wsdl/GetConfigurations" => resp_ptz_configurations(),
        "http://www.onvif.org/ver20/ptz/wsdl/AbsoluteMove"
        | "http://www.onvif.org/ver20/ptz/wsdl/RelativeMove"
        | "http://www.onvif.org/ver20/ptz/wsdl/ContinuousMove"
        | "http://www.onvif.org/ver20/ptz/wsdl/Stop"
        | "http://www.onvif.org/ver20/ptz/wsdl/GotoPreset"
        | "http://www.onvif.org/ver20/ptz/wsdl/GotoHomePosition"
        | "http://www.onvif.org/ver20/ptz/wsdl/SetHomePosition" => {
            resp_empty("tptz", "PTZResponse")
        }
        "http://www.onvif.org/ver20/ptz/wsdl/SetPreset" => resp_ptz_set_preset(),

        // ── Imaging ───────────────────────────────────────────────────────────
        "http://www.onvif.org/ver20/imaging/wsdl/GetImagingSettings" => resp_imaging_settings(),
        "http://www.onvif.org/ver20/imaging/wsdl/GetOptions" => resp_imaging_options(),
        "http://www.onvif.org/ver20/imaging/wsdl/GetStatus" => resp_imaging_status(),
        "http://www.onvif.org/ver20/imaging/wsdl/GetMoveOptions" => resp_imaging_move_options(),
        "http://www.onvif.org/ver20/imaging/wsdl/Move"
        | "http://www.onvif.org/ver20/imaging/wsdl/Stop"
        | "http://www.onvif.org/ver20/imaging/wsdl/SetImagingSettings" => {
            resp_empty("timg", "ImagingResponse")
        }

        // ── Recording ─────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/recording/wsdl/GetRecordings" => resp_recordings(),

        // ── Search ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/search/wsdl/FindRecordings" => resp_find_recordings(),
        "http://www.onvif.org/ver10/search/wsdl/GetRecordingSearchResults" => {
            resp_recording_search_results()
        }
        "http://www.onvif.org/ver10/search/wsdl/EndSearch" => {
            resp_empty("tse", "EndSearchResponse")
        }

        // ── Replay ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/replay/wsdl/GetReplayUri" => resp_replay_uri(),

        // ── Unknown ───────────────────────────────────────────────────────────
        other => {
            eprintln!("  [WARN] unhandled action: {other}");
            resp_soap_fault("s:Receiver", &format!("Not implemented: {other}"))
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Wrap a response body in a SOAP 1.2 envelope.
fn soap(extra_ns: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tt="http://www.onvif.org/ver10/schema" {extra_ns}><s:Body>{body}</s:Body></s:Envelope>"#
    )
}

/// Return an empty `<prefix:Tag/>` response (for write operations that return void).
fn resp_empty(prefix: &str, tag: &str) -> String {
    soap("", &format!("<{prefix}:{tag}/>"))
}

fn resp_soap_fault(code: &str, reason: &str) -> String {
    soap(
        "",
        &format!(
            r#"<s:Fault><s:Code><s:Value>{code}</s:Value></s:Code><s:Reason><s:Text xml:lang="en">{reason}</s:Text></s:Reason></s:Fault>"#
        ),
    )
}

// ── Device responses ──────────────────────────────────────────────────────────

fn resp_system_date_and_time() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetSystemDateAndTimeResponse>
          <tds:SystemDateAndTime>
            <tt:DateTimeType>NTP</tt:DateTimeType>
            <tt:DaylightSavings>false</tt:DaylightSavings>
            <tt:TimeZone><tt:TZ>UTC</tt:TZ></tt:TimeZone>
            <tt:UTCDateTime>
              <tt:Time><tt:Hour>12</tt:Hour><tt:Minute>0</tt:Minute><tt:Second>0</tt:Second></tt:Time>
              <tt:Date><tt:Year>2026</tt:Year><tt:Month>4</tt:Month><tt:Day>3</tt:Day></tt:Date>
            </tt:UTCDateTime>
          </tds:SystemDateAndTime>
        </tds:GetSystemDateAndTimeResponse>"#,
    )
}

fn resp_capabilities(base: &str) -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
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

fn resp_services(base: &str) -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        &format!(
            r#"<tds:GetServicesResponse>
          <tds:Service>
            <tds:Namespace>http://www.onvif.org/ver10/device/wsdl</tds:Namespace>
            <tds:XAddr>{base}/onvif/device</tds:XAddr>
            <tds:Version><tt:Major>2</tt:Major><tt:Minor>6</tt:Minor></tds:Version>
          </tds:Service>
          <tds:Service>
            <tds:Namespace>http://www.onvif.org/ver10/media/wsdl</tds:Namespace>
            <tds:XAddr>{base}/onvif/media</tds:XAddr>
            <tds:Version><tt:Major>2</tt:Major><tt:Minor>6</tt:Minor></tds:Version>
          </tds:Service>
          <tds:Service>
            <tds:Namespace>http://www.onvif.org/ver20/media/wsdl</tds:Namespace>
            <tds:XAddr>{base}/onvif/media2</tds:XAddr>
            <tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version>
          </tds:Service>
          <tds:Service>
            <tds:Namespace>http://www.onvif.org/ver20/ptz/wsdl</tds:Namespace>
            <tds:XAddr>{base}/onvif/ptz</tds:XAddr>
            <tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version>
          </tds:Service>
          <tds:Service>
            <tds:Namespace>http://www.onvif.org/ver20/imaging/wsdl</tds:Namespace>
            <tds:XAddr>{base}/onvif/imaging</tds:XAddr>
            <tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version>
          </tds:Service>
          <tds:Service>
            <tds:Namespace>http://www.onvif.org/ver10/recording/wsdl</tds:Namespace>
            <tds:XAddr>{base}/onvif/recording</tds:XAddr>
            <tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version>
          </tds:Service>
          <tds:Service>
            <tds:Namespace>http://www.onvif.org/ver10/search/wsdl</tds:Namespace>
            <tds:XAddr>{base}/onvif/search</tds:XAddr>
            <tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version>
          </tds:Service>
          <tds:Service>
            <tds:Namespace>http://www.onvif.org/ver10/replay/wsdl</tds:Namespace>
            <tds:XAddr>{base}/onvif/replay</tds:XAddr>
            <tds:Version><tt:Major>2</tt:Major><tt:Minor>0</tt:Minor></tds:Version>
          </tds:Service>
        </tds:GetServicesResponse>"#
        ),
    )
}

fn resp_device_info() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetDeviceInformationResponse>
          <tds:Manufacturer>oxvif-mock</tds:Manufacturer>
          <tds:Model>MockCam-1080p</tds:Model>
          <tds:FirmwareVersion>1.0.0</tds:FirmwareVersion>
          <tds:SerialNumber>MOCK-0001</tds:SerialNumber>
          <tds:HardwareId>1.0</tds:HardwareId>
        </tds:GetDeviceInformationResponse>"#,
    )
}

fn resp_hostname() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetHostnameResponse>
          <tds:HostnameInformation>
            <tt:FromDHCP>false</tt:FromDHCP>
            <tt:Name>mock-camera</tt:Name>
          </tds:HostnameInformation>
        </tds:GetHostnameResponse>"#,
    )
}

fn resp_ntp() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetNTPResponse>
          <tds:NTPInformation>
            <tt:FromDHCP>false</tt:FromDHCP>
            <tt:NTPManual>
              <tt:Type>DNS</tt:Type>
              <tt:DNSname>pool.ntp.org</tt:DNSname>
            </tt:NTPManual>
          </tds:NTPInformation>
        </tds:GetNTPResponse>"#,
    )
}

fn resp_scopes() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetScopesResponse>
          <tds:Scopes>
            <tt:ScopeAttribute>Fixed</tt:ScopeAttribute>
            <tt:ScopeItem>onvif://www.onvif.org/name/MockCamera</tt:ScopeItem>
          </tds:Scopes>
          <tds:Scopes>
            <tt:ScopeAttribute>Fixed</tt:ScopeAttribute>
            <tt:ScopeItem>onvif://www.onvif.org/type/video_encoder</tt:ScopeItem>
          </tds:Scopes>
          <tds:Scopes>
            <tt:ScopeAttribute>Fixed</tt:ScopeAttribute>
            <tt:ScopeItem>onvif://www.onvif.org/location/country/taiwan</tt:ScopeItem>
          </tds:Scopes>
        </tds:GetScopesResponse>"#,
    )
}

fn resp_users() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetUsersResponse>
          <tds:User>
            <tt:Username>admin</tt:Username>
            <tt:UserLevel>Administrator</tt:UserLevel>
          </tds:User>
          <tds:User>
            <tt:Username>operator</tt:Username>
            <tt:UserLevel>Operator</tt:UserLevel>
          </tds:User>
        </tds:GetUsersResponse>"#,
    )
}

fn resp_network_interfaces() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
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

fn resp_set_network_interfaces() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:SetNetworkInterfacesResponse>
          <tds:RebootNeeded>false</tds:RebootNeeded>
        </tds:SetNetworkInterfacesResponse>"#,
    )
}

fn resp_network_protocols() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetNetworkProtocolsResponse>
          <tds:NetworkProtocols>
            <tt:Name>HTTP</tt:Name>
            <tt:Enabled>true</tt:Enabled>
            <tt:Port>80</tt:Port>
          </tds:NetworkProtocols>
          <tds:NetworkProtocols>
            <tt:Name>HTTPS</tt:Name>
            <tt:Enabled>true</tt:Enabled>
            <tt:Port>443</tt:Port>
          </tds:NetworkProtocols>
          <tds:NetworkProtocols>
            <tt:Name>RTSP</tt:Name>
            <tt:Enabled>true</tt:Enabled>
            <tt:Port>554</tt:Port>
          </tds:NetworkProtocols>
        </tds:GetNetworkProtocolsResponse>"#,
    )
}

fn resp_dns() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetDNSResponse>
          <tds:DNSInformation>
            <tt:FromDHCP>false</tt:FromDHCP>
            <tt:DNSManual>
              <tt:Type>IPv4</tt:Type>
              <tt:IPv4Address>8.8.8.8</tt:IPv4Address>
            </tt:DNSManual>
            <tt:DNSManual>
              <tt:Type>IPv4</tt:Type>
              <tt:IPv4Address>8.8.4.4</tt:IPv4Address>
            </tt:DNSManual>
          </tds:DNSInformation>
        </tds:GetDNSResponse>"#,
    )
}

fn resp_network_default_gateway() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetNetworkDefaultGatewayResponse>
          <tds:NetworkGateway>
            <tt:IPv4Address>192.168.1.1</tt:IPv4Address>
          </tds:NetworkGateway>
        </tds:GetNetworkDefaultGatewayResponse>"#,
    )
}

fn resp_system_log() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetSystemLogResponse>
          <tds:SystemLog>
            <tt:String>2026-04-03 12:00:00 mock system started</tt:String>
          </tds:SystemLog>
        </tds:GetSystemLogResponse>"#,
    )
}

fn resp_relay_outputs() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
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

// ── Media1 responses ──────────────────────────────────────────────────────────

fn resp_profiles() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetProfilesResponse>
          <trt:Profiles token="Profile_1" fixed="true">
            <tt:Name>mainStream</tt:Name>
          </trt:Profiles>
          <trt:Profiles token="Profile_2" fixed="false">
            <tt:Name>subStream</tt:Name>
          </trt:Profiles>
        </trt:GetProfilesResponse>"#,
    )
}

fn resp_profile() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetProfileResponse>
          <trt:Profile token="Profile_1" fixed="true">
            <tt:Name>mainStream</tt:Name>
          </trt:Profile>
        </trt:GetProfileResponse>"#,
    )
}

fn resp_stream_uri() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetStreamUriResponse>
          <trt:MediaUri>
            <tt:Uri>rtsp://127.0.0.1:554/mock/stream</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetStreamUriResponse>"#,
    )
}

fn resp_snapshot_uri() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetSnapshotUriResponse>
          <trt:MediaUri>
            <tt:Uri>http://127.0.0.1:18080/mock/snapshot.jpg</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetSnapshotUriResponse>"#,
    )
}

fn resp_create_profile() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:CreateProfileResponse>
          <trt:Profile token="Profile_New" fixed="false">
            <tt:Name>oxvif-test-profile</tt:Name>
          </trt:Profile>
        </trt:CreateProfileResponse>"#,
    )
}

fn resp_video_sources() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourcesResponse>
          <trt:VideoSources token="VS_1">
            <tt:Framerate>25</tt:Framerate>
            <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
          </trt:VideoSources>
        </trt:GetVideoSourcesResponse>"#,
    )
}

fn resp_video_source_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourceConfigurationsResponse>
          <trt:Configurations token="VSC_1">
            <tt:Name>VSConfig1</tt:Name>
            <tt:UseCount>2</tt:UseCount>
            <tt:SourceToken>VS_1</tt:SourceToken>
            <tt:Bounds x="0" y="0" width="1920" height="1080"/>
          </trt:Configurations>
        </trt:GetVideoSourceConfigurationsResponse>"#,
    )
}

fn resp_video_encoder_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoEncoderConfigurationsResponse>
          <trt:Configurations token="VEC_1">
            <tt:Name>MainStream</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>H264</tt:Encoding>
            <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
            <tt:Quality>5</tt:Quality>
            <tt:RateControl>
              <tt:FrameRateLimit>25</tt:FrameRateLimit>
              <tt:EncodingInterval>1</tt:EncodingInterval>
              <tt:BitrateLimit>4096</tt:BitrateLimit>
            </tt:RateControl>
            <tt:H264>
              <tt:GovLength>25</tt:GovLength>
              <tt:H264Profile>Main</tt:H264Profile>
            </tt:H264>
          </trt:Configurations>
          <trt:Configurations token="VEC_2">
            <tt:Name>SubStream</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>JPEG</tt:Encoding>
            <tt:Resolution><tt:Width>640</tt:Width><tt:Height>480</tt:Height></tt:Resolution>
            <tt:Quality>3</tt:Quality>
          </trt:Configurations>
        </trt:GetVideoEncoderConfigurationsResponse>"#,
    )
}

fn resp_audio_sources() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioSourcesResponse>
          <trt:AudioSources token="AudioSource_1">
            <tt:Channels>1</tt:Channels>
          </trt:AudioSources>
        </trt:GetAudioSourcesResponse>"#,
    )
}

fn resp_audio_encoder_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationsResponse>
          <trt:Configurations token="AEC_1">
            <tt:Name>AudioEncoder</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>G711</tt:Encoding>
            <tt:Bitrate>64</tt:Bitrate>
            <tt:SampleRate>8</tt:SampleRate>
          </trt:Configurations>
        </trt:GetAudioEncoderConfigurationsResponse>"#,
    )
}

fn resp_osds() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetOSDsResponse>
          <trt:OSDs token="OSD_1">
            <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
            <tt:Type>Text</tt:Type>
            <tt:Position><tt:Type>UpperLeft</tt:Type></tt:Position>
            <tt:TextString>
              <tt:Type>DateAndTime</tt:Type>
            </tt:TextString>
          </trt:OSDs>
        </trt:GetOSDsResponse>"#,
    )
}

// ── Media2 responses ──────────────────────────────────────────────────────────

fn resp_profiles_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetProfilesResponse>
          <tr2:Profiles token="Profile_A" fixed="true">
            <tt:Name>mainStream</tt:Name>
            <tr2:Configurations>
              <tr2:VideoSource token="VSC_1"/>
              <tr2:VideoEncoder token="VEC_1"/>
            </tr2:Configurations>
          </tr2:Profiles>
          <tr2:Profiles token="Profile_B" fixed="false">
            <tt:Name>subStream</tt:Name>
          </tr2:Profiles>
        </tr2:GetProfilesResponse>"#,
    )
}

fn resp_stream_uri_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetStreamUriResponse>
          <tr2:Uri>rtsp://127.0.0.1:554/mock/h265</tr2:Uri>
        </tr2:GetStreamUriResponse>"#,
    )
}

fn resp_snapshot_uri_media2() -> String {
    soap(
        r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
        r#"<tr2:GetSnapshotUriResponse>
          <tr2:Uri>http://127.0.0.1:8080/mock/snapshot2.jpg</tr2:Uri>
        </tr2:GetSnapshotUriResponse>"#,
    )
}

// ── PTZ responses ─────────────────────────────────────────────────────────────

fn resp_ptz_status() -> String {
    soap(
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
        r#"<tptz:GetStatusResponse>
          <tptz:PTZStatus>
            <tt:Position>
              <tt:PanTilt x="0.1" y="-0.2" space="http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace"/>
              <tt:Zoom x="0.0" space="http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace"/>
            </tt:Position>
            <tt:MoveStatus>
              <tt:PanTilt>IDLE</tt:PanTilt>
              <tt:Zoom>IDLE</tt:Zoom>
            </tt:MoveStatus>
          </tptz:PTZStatus>
        </tptz:GetStatusResponse>"#,
    )
}

fn resp_ptz_presets() -> String {
    soap(
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
        r#"<tptz:GetPresetsResponse>
          <tptz:Preset token="Preset_1">
            <tt:Name>Home</tt:Name>
            <tt:PTZPosition>
              <tt:PanTilt x="0.0" y="0.0" space="http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace"/>
              <tt:Zoom x="0.0" space="http://www.onvif.org/ver10/tptz/ZoomSpaces/PositionGenericSpace"/>
            </tt:PTZPosition>
          </tptz:Preset>
          <tptz:Preset token="Preset_2">
            <tt:Name>Door</tt:Name>
            <tt:PTZPosition>
              <tt:PanTilt x="0.5" y="0.2" space="http://www.onvif.org/ver10/tptz/PanTiltSpaces/PositionGenericSpace"/>
            </tt:PTZPosition>
          </tptz:Preset>
        </tptz:GetPresetsResponse>"#,
    )
}

fn resp_ptz_set_preset() -> String {
    soap(
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
        r#"<tptz:SetPresetResponse>
          <tptz:PresetToken>Preset_3</tptz:PresetToken>
        </tptz:SetPresetResponse>"#,
    )
}

fn resp_ptz_nodes() -> String {
    soap(
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
        r#"<tptz:GetNodesResponse>
          <tptz:PTZNode token="PTZNode_1" FixedHomePosition="false">
            <tt:Name>PTZNode</tt:Name>
            <tt:SupportedPTZSpaces/>
            <tt:MaximumNumberOfPresets>100</tt:MaximumNumberOfPresets>
            <tt:HomeSupported>true</tt:HomeSupported>
          </tptz:PTZNode>
        </tptz:GetNodesResponse>"#,
    )
}

fn resp_ptz_configurations() -> String {
    soap(
        r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
        r#"<tptz:GetConfigurationsResponse>
          <tptz:PTZConfiguration token="PTZConfig_1">
            <tt:Name>PTZConfig</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:NodeToken>PTZNode_1</tt:NodeToken>
            <tt:DefaultPTZTimeout>PT10S</tt:DefaultPTZTimeout>
          </tptz:PTZConfiguration>
        </tptz:GetConfigurationsResponse>"#,
    )
}

// ── Imaging responses ─────────────────────────────────────────────────────────

fn resp_imaging_settings() -> String {
    soap(
        r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#,
        r#"<timg:GetImagingSettingsResponse>
          <timg:ImagingSettings>
            <tt:Brightness>60</tt:Brightness>
            <tt:ColorSaturation>50</tt:ColorSaturation>
            <tt:Contrast>45</tt:Contrast>
            <tt:Sharpness>30</tt:Sharpness>
            <tt:IrCutFilter>AUTO</tt:IrCutFilter>
            <tt:WhiteBalance><tt:Mode>AUTO</tt:Mode></tt:WhiteBalance>
            <tt:Exposure><tt:Mode>MANUAL</tt:Mode></tt:Exposure>
          </timg:ImagingSettings>
        </timg:GetImagingSettingsResponse>"#,
    )
}

fn resp_imaging_options() -> String {
    soap(
        r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#,
        r#"<timg:GetOptionsResponse>
          <timg:ImagingOptions>
            <tt:Brightness><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Brightness>
            <tt:ColorSaturation><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:ColorSaturation>
            <tt:Contrast><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Contrast>
            <tt:Sharpness><tt:Min>0</tt:Min><tt:Max>100</tt:Max></tt:Sharpness>
            <tt:IrCutFilterModes>ON</tt:IrCutFilterModes>
            <tt:IrCutFilterModes>OFF</tt:IrCutFilterModes>
            <tt:IrCutFilterModes>AUTO</tt:IrCutFilterModes>
            <tt:WhiteBalance>
              <tt:Mode>AUTO</tt:Mode>
              <tt:Mode>MANUAL</tt:Mode>
            </tt:WhiteBalance>
            <tt:Exposure>
              <tt:Mode>AUTO</tt:Mode>
              <tt:Mode>MANUAL</tt:Mode>
            </tt:Exposure>
          </timg:ImagingOptions>
        </timg:GetOptionsResponse>"#,
    )
}

fn resp_imaging_status() -> String {
    soap(
        r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#,
        r#"<timg:GetStatusResponse>
          <timg:Status>
            <tt:FocusStatus20 xmlns:tt="http://www.onvif.org/ver10/schema">
              <tt:Position>0.5</tt:Position>
              <tt:MoveStatus>IDLE</tt:MoveStatus>
            </tt:FocusStatus20>
          </timg:Status>
        </timg:GetStatusResponse>"#,
    )
}

fn resp_imaging_move_options() -> String {
    soap(
        r#"xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl""#,
        r#"<timg:GetMoveOptionsResponse>
          <timg:MoveOptions>
            <tt:Absolute xmlns:tt="http://www.onvif.org/ver10/schema">
              <tt:PositionSpace><tt:Min>0.0</tt:Min><tt:Max>1.0</tt:Max></tt:PositionSpace>
              <tt:SpeedSpace><tt:Min>0.0</tt:Min><tt:Max>1.0</tt:Max></tt:SpeedSpace>
            </tt:Absolute>
            <tt:Continuous xmlns:tt="http://www.onvif.org/ver10/schema">
              <tt:SpeedSpace><tt:Min>-1.0</tt:Min><tt:Max>1.0</tt:Max></tt:SpeedSpace>
            </tt:Continuous>
          </timg:MoveOptions>
        </timg:GetMoveOptionsResponse>"#,
    )
}

// ── Recording responses ───────────────────────────────────────────────────────

fn resp_recordings() -> String {
    soap(
        r#"xmlns:trc="http://www.onvif.org/ver10/recording/wsdl""#,
        r#"<trc:GetRecordingsResponse>
          <trc:RecordingItems Token="Rec_001">
            <trc:RecordingInformation>
              <tt:RecordingStatus>Recording</tt:RecordingStatus>
              <tt:Source>
                <tt:SourceId>rtsp://mock/live</tt:SourceId>
                <tt:Name>MockCamera</tt:Name>
                <tt:Location>Lab</tt:Location>
                <tt:Description>Mock recording</tt:Description>
              </tt:Source>
              <tt:EarliestRecording>2026-01-01T00:00:00Z</tt:EarliestRecording>
              <tt:LatestRecording>2026-04-01T00:00:00Z</tt:LatestRecording>
            </trc:RecordingInformation>
          </trc:RecordingItems>
          <trc:RecordingItems Token="Rec_002">
            <trc:RecordingInformation>
              <tt:RecordingStatus>Stopped</tt:RecordingStatus>
              <tt:Source>
                <tt:Name>MockCamera</tt:Name>
              </tt:Source>
              <tt:EarliestRecording>2025-12-01T00:00:00Z</tt:EarliestRecording>
              <tt:LatestRecording>2025-12-31T00:00:00Z</tt:LatestRecording>
            </trc:RecordingInformation>
          </trc:RecordingItems>
        </trc:GetRecordingsResponse>"#,
    )
}

// ── Search responses ──────────────────────────────────────────────────────────

fn resp_find_recordings() -> String {
    soap(
        r#"xmlns:tse="http://www.onvif.org/ver10/search/wsdl""#,
        r#"<tse:FindRecordingsResponse>
          <tse:SearchToken>search_mock_001</tse:SearchToken>
        </tse:FindRecordingsResponse>"#,
    )
}

fn resp_recording_search_results() -> String {
    soap(
        r#"xmlns:tse="http://www.onvif.org/ver10/search/wsdl""#,
        r#"<tse:GetRecordingSearchResultsResponse>
          <tse:SearchState>Completed</tse:SearchState>
          <tse:RecordingInformation>
            <tt:RecordingToken>Rec_001</tt:RecordingToken>
            <tt:Source>
              <tt:Name>MockCamera</tt:Name>
            </tt:Source>
            <tt:EarliestRecording>2026-01-01T00:00:00Z</tt:EarliestRecording>
            <tt:LatestRecording>2026-04-01T00:00:00Z</tt:LatestRecording>
            <tt:Content>Motion event</tt:Content>
            <tt:RecordingStatus>Stopped</tt:RecordingStatus>
          </tse:RecordingInformation>
        </tse:GetRecordingSearchResultsResponse>"#,
    )
}

// ── Replay responses ──────────────────────────────────────────────────────────

fn resp_replay_uri() -> String {
    soap(
        r#"xmlns:trp="http://www.onvif.org/ver10/replay/wsdl""#,
        r#"<trp:GetReplayUriResponse>
          <trp:Uri>rtsp://127.0.0.1:554/mock/replay/Rec_001</trp:Uri>
        </trp:GetReplayUriResponse>"#,
    )
}

// ── Storage / system URI / discovery responses ────────────────────────────────

fn resp_storage_configurations() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
             xmlns:tt="http://www.onvif.org/ver10/schema""#,
        r#"<tds:GetStorageConfigurationsResponse>
          <tds:StorageConfigurations token="SD_01">
            <tt:StorageType>LocalStorage</tt:StorageType>
            <tt:LocalPath>/mnt/sd</tt:LocalPath>
            <tt:StorageUri></tt:StorageUri>
            <tt:UserInfo>
              <tt:Username></tt:Username>
              <tt:UseAnonymous>true</tt:UseAnonymous>
            </tt:UserInfo>
          </tds:StorageConfigurations>
        </tds:GetStorageConfigurationsResponse>"#,
    )
}

fn resp_system_uris(base: &str) -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        &format!(
            r#"<tds:GetSystemUrisResponse>
          <tds:FirmwareUpgrade>{base}/firmware</tds:FirmwareUpgrade>
          <tds:SystemLog>{base}/syslog</tds:SystemLog>
          <tds:SupportInfo>{base}/support</tds:SupportInfo>
        </tds:GetSystemUrisResponse>"#
        ),
    )
}

fn resp_discovery_mode() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetDiscoveryModeResponse>
          <tds:DiscoveryMode>Discoverable</tds:DiscoveryMode>
        </tds:GetDiscoveryModeResponse>"#,
    )
}
