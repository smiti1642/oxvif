//! Write-workflow — exercises every Set / Create / Delete operation in oxvif
//! against a minimal embedded ONVIF mock server.
//!
//! No real camera is required. The mock starts automatically on an OS-assigned
//! port and terminates when the process exits.
//!
//! ```sh
//! cargo run --example write_workflow
//! ```
//!
//! Point at a running mock server or real camera instead:
//!
//! ```sh
//! WRITE_WF_URL=http://127.0.0.1:18080/onvif/device \
//! cargo run --example write_workflow
//! ```

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use oxvif::{OsdConfiguration, OsdPosition, OsdTextString};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

// ── Embedded mock server ──────────────────────────────────────────────────────

struct MockState {
    base: String,
}

async fn handle(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    _body: axum::body::Bytes,
) -> impl IntoResponse {
    let action = extract_action(&headers).unwrap_or_default();
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

fn extract_action(headers: &HeaderMap) -> Option<String> {
    let ct = headers.get("content-type")?.to_str().ok()?;
    let start = ct.find("action=\"")? + 8;
    let end = ct[start..].find('"')? + start;
    Some(ct[start..end].to_string())
}

fn soap(extra_ns: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tt="http://www.onvif.org/ver10/schema" {extra_ns}><s:Body>{body}</s:Body></s:Envelope>"#
    )
}

fn empty(prefix: &str, tag: &str) -> String {
    soap("", &format!("<{prefix}:{tag}/>"))
}

fn dispatch(action: &str, base: &str) -> String {
    match action {
        // ── Device ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/device/wsdl/GetCapabilities" => soap(
            r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
            &format!(
                r#"<tds:GetCapabilitiesResponse><tds:Capabilities>
                  <tt:Device><tt:XAddr>{base}/onvif/device</tt:XAddr></tt:Device>
                  <tt:Media><tt:XAddr>{base}/onvif/media</tt:XAddr><tt:StreamingCapabilities><tt:RTPMulticast>false</tt:RTPMulticast><tt:RTP_TCP>true</tt:RTP_TCP><tt:RTP_RTSP_TCP>true</tt:RTP_RTSP_TCP></tt:StreamingCapabilities></tt:Media>
                  <tt:PTZ><tt:XAddr>{base}/onvif/ptz</tt:XAddr></tt:PTZ>
                  <tt:Imaging><tt:XAddr>{base}/onvif/imaging</tt:XAddr></tt:Imaging>
                  <tt:Extension><tt:Media2><tt:XAddr>{base}/onvif/media2</tt:XAddr></tt:Media2></tt:Extension>
                </tds:Capabilities></tds:GetCapabilitiesResponse>"#
            ),
        ),
        "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime" => soap(
            r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
            r#"<tds:GetSystemDateAndTimeResponse><tds:SystemDateAndTime>
              <tt:DateTimeType>NTP</tt:DateTimeType><tt:DaylightSavings>false</tt:DaylightSavings>
              <tt:TimeZone><tt:TZ>UTC</tt:TZ></tt:TimeZone>
              <tt:UTCDateTime>
                <tt:Time><tt:Hour>12</tt:Hour><tt:Minute>0</tt:Minute><tt:Second>0</tt:Second></tt:Time>
                <tt:Date><tt:Year>2026</tt:Year><tt:Month>4</tt:Month><tt:Day>3</tt:Day></tt:Date>
              </tt:UTCDateTime>
            </tds:SystemDateAndTime></tds:GetSystemDateAndTimeResponse>"#,
        ),
        "http://www.onvif.org/ver10/device/wsdl/SetHostname" => empty("tds", "SetHostnameResponse"),
        "http://www.onvif.org/ver10/device/wsdl/SetNTP" => empty("tds", "SetNTPResponse"),
        "http://www.onvif.org/ver10/device/wsdl/SystemReboot" => soap(
            r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
            r#"<tds:SystemRebootResponse>
              <tds:Message>Rebooting in 30 seconds</tds:Message>
            </tds:SystemRebootResponse>"#,
        ),
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkProtocols" => {
            empty("tds", "SetNetworkProtocolsResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetDNS" => empty("tds", "SetDNSResponse"),
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkInterfaces" => soap(
            r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
            r#"<tds:SetNetworkInterfacesResponse>
              <tds:RebootNeeded>false</tds:RebootNeeded>
            </tds:SetNetworkInterfacesResponse>"#,
        ),
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputSettings" => {
            empty("tds", "SetRelayOutputSettingsResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputState" => {
            empty("tds", "SetRelayOutputStateResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault" => {
            empty("tds", "SetSystemFactoryDefaultResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetStorageConfiguration" => {
            empty("tds", "SetStorageConfigurationResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/SetDiscoveryMode" => {
            empty("tds", "SetDiscoveryModeResponse")
        }
        "http://www.onvif.org/ver10/device/wsdl/CreateUsers" => empty("tds", "CreateUsersResponse"),
        "http://www.onvif.org/ver10/device/wsdl/DeleteUsers" => empty("tds", "DeleteUsersResponse"),
        "http://www.onvif.org/ver10/device/wsdl/SetUser" => empty("tds", "SetUserResponse"),

        // ── Media1 ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfiguration" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:GetVideoSourceConfigurationResponse>
              <trt:Configuration token="VSC_1">
                <tt:Name>VSConfig1</tt:Name><tt:UseCount>2</tt:UseCount>
                <tt:SourceToken>VS_1</tt:SourceToken>
                <tt:Bounds x="0" y="0" width="1920" height="1080"/>
              </trt:Configuration>
            </trt:GetVideoSourceConfigurationResponse>"#,
        ),
        "http://www.onvif.org/ver10/media/wsdl/SetVideoSourceConfiguration" => {
            empty("trt", "SetVideoSourceConfigurationResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoSourceConfigurationOptions" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:GetVideoSourceConfigurationOptionsResponse>
              <trt:Options>
                <tt:MaximumNumberOfProfiles>5</tt:MaximumNumberOfProfiles>
                <tt:BoundsRange>
                  <tt:XRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:XRange>
                  <tt:YRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:YRange>
                  <tt:WidthRange><tt:Min>160</tt:Min><tt:Max>1920</tt:Max></tt:WidthRange>
                  <tt:HeightRange><tt:Min>90</tt:Min><tt:Max>1080</tt:Max></tt:HeightRange>
                </tt:BoundsRange>
                <tt:VideoSourceTokensAvailable>VS_1</tt:VideoSourceTokensAvailable>
              </trt:Options>
            </trt:GetVideoSourceConfigurationOptionsResponse>"#,
        ),
        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfiguration" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:GetVideoEncoderConfigurationResponse>
              <trt:Configuration token="VEC_1">
                <tt:Name>MainStream</tt:Name><tt:UseCount>1</tt:UseCount>
                <tt:Encoding>H264</tt:Encoding>
                <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
                <tt:Quality>5</tt:Quality>
                <tt:RateControl>
                  <tt:FrameRateLimit>25</tt:FrameRateLimit>
                  <tt:EncodingInterval>1</tt:EncodingInterval>
                  <tt:BitrateLimit>4096</tt:BitrateLimit>
                </tt:RateControl>
                <tt:H264><tt:GovLength>25</tt:GovLength><tt:H264Profile>Main</tt:H264Profile></tt:H264>
              </trt:Configuration>
            </trt:GetVideoEncoderConfigurationResponse>"#,
        ),
        "http://www.onvif.org/ver10/media/wsdl/SetVideoEncoderConfiguration" => {
            empty("trt", "SetVideoEncoderConfigurationResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetVideoEncoderConfigurationOptions" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:GetVideoEncoderConfigurationOptionsResponse>
              <trt:Options>
                <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
                <tt:H264>
                  <tt:ResolutionsAvailable><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:ResolutionsAvailable>
                  <tt:ResolutionsAvailable><tt:Width>1280</tt:Width><tt:Height>720</tt:Height></tt:ResolutionsAvailable>
                  <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>300</tt:Max></tt:GovLengthRange>
                  <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:FrameRateRange>
                  <tt:EncodingIntervalRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:EncodingIntervalRange>
                  <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>16384</tt:Max></tt:BitrateRange>
                  <tt:H264ProfilesSupported>Baseline</tt:H264ProfilesSupported>
                  <tt:H264ProfilesSupported>Main</tt:H264ProfilesSupported>
                  <tt:H264ProfilesSupported>High</tt:H264ProfilesSupported>
                </tt:H264>
              </trt:Options>
            </trt:GetVideoEncoderConfigurationOptionsResponse>"#,
        ),
        "http://www.onvif.org/ver10/media/wsdl/CreateProfile" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:CreateProfileResponse>
              <trt:Profile token="Profile_WF" fixed="false">
                <tt:Name>write-wf-test</tt:Name>
              </trt:Profile>
            </trt:CreateProfileResponse>"#,
        ),
        "http://www.onvif.org/ver10/media/wsdl/DeleteProfile" => {
            empty("trt", "DeleteProfileResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetOSD" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:GetOSDResponse>
              <trt:OSDConfiguration token="OSD_1">
                <tt:VideoSourceConfigurationToken>VSC_1</tt:VideoSourceConfigurationToken>
                <tt:Type>Text</tt:Type>
                <tt:Position><tt:Type>UpperLeft</tt:Type></tt:Position>
                <tt:TextString><tt:Type>DateAndTime</tt:Type></tt:TextString>
              </trt:OSDConfiguration>
            </trt:GetOSDResponse>"#,
        ),
        "http://www.onvif.org/ver10/media/wsdl/SetOSD" => empty("trt", "SetOSDResponse"),
        "http://www.onvif.org/ver10/media/wsdl/CreateOSD" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:CreateOSDResponse>
              <trt:OSDToken>OSD_WF</trt:OSDToken>
            </trt:CreateOSDResponse>"#,
        ),
        "http://www.onvif.org/ver10/media/wsdl/DeleteOSD" => empty("trt", "DeleteOSDResponse"),
        "http://www.onvif.org/ver10/media/wsdl/GetOSDOptions" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:GetOSDOptionsResponse>
              <trt:OSDOptions>
                <tt:MaximumNumberOfOSDs>8</tt:MaximumNumberOfOSDs>
                <tt:Type>Text</tt:Type><tt:Type>Image</tt:Type>
                <tt:PositionOption>
                  <tt:Type>UpperLeft</tt:Type><tt:Type>LowerRight</tt:Type><tt:Type>Custom</tt:Type>
                </tt:PositionOption>
                <tt:TextOption>
                  <tt:Type>Plain</tt:Type><tt:Type>Date</tt:Type><tt:Type>DateAndTime</tt:Type>
                </tt:TextOption>
              </trt:OSDOptions>
            </trt:GetOSDOptionsResponse>"#,
        ),
        "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfiguration" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:GetAudioEncoderConfigurationResponse>
              <trt:Configuration token="AEC_1">
                <tt:Name>AudioEncoder</tt:Name><tt:UseCount>1</tt:UseCount>
                <tt:Encoding>G711</tt:Encoding>
                <tt:Bitrate>64</tt:Bitrate>
                <tt:SampleRate>8</tt:SampleRate>
              </trt:Configuration>
            </trt:GetAudioEncoderConfigurationResponse>"#,
        ),
        "http://www.onvif.org/ver10/media/wsdl/SetAudioEncoderConfiguration" => {
            empty("trt", "SetAudioEncoderConfigurationResponse")
        }
        "http://www.onvif.org/ver10/media/wsdl/GetAudioEncoderConfigurationOptions" => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            r#"<trt:GetAudioEncoderConfigurationOptionsResponse>
              <trt:Options>
                <tt:Encoding>G711</tt:Encoding>
                <tt:BitrateList><tt:Items>64</tt:Items></tt:BitrateList>
                <tt:SampleRateList><tt:Items>8</tt:Items></tt:SampleRateList>
              </trt:Options>
              <trt:Options>
                <tt:Encoding>AAC</tt:Encoding>
                <tt:BitrateList><tt:Items>64 128 256</tt:Items></tt:BitrateList>
                <tt:SampleRateList><tt:Items>16 32 44</tt:Items></tt:SampleRateList>
              </trt:Options>
            </trt:GetAudioEncoderConfigurationOptionsResponse>"#,
        ),

        // ── PTZ ───────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver20/ptz/wsdl/GetConfiguration" => soap(
            r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
            r#"<tptz:GetConfigurationResponse>
              <tptz:PTZConfiguration token="PTZConfig_1">
                <tt:Name>PTZConfig</tt:Name><tt:UseCount>1</tt:UseCount>
                <tt:NodeToken>PTZNode_1</tt:NodeToken>
                <tt:DefaultPTZTimeout>PT10S</tt:DefaultPTZTimeout>
              </tptz:PTZConfiguration>
            </tptz:GetConfigurationResponse>"#,
        ),
        "http://www.onvif.org/ver20/ptz/wsdl/SetConfiguration" => {
            empty("tptz", "SetConfigurationResponse")
        }
        "http://www.onvif.org/ver20/ptz/wsdl/GetConfigurationOptions" => soap(
            r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
            r#"<tptz:GetConfigurationOptionsResponse>
              <tptz:PTZConfigurationOptions>
                <tt:PTZTimeout>
                  <tt:Min>PT1S</tt:Min>
                  <tt:Max>PT60S</tt:Max>
                </tt:PTZTimeout>
              </tptz:PTZConfigurationOptions>
            </tptz:GetConfigurationOptionsResponse>"#,
        ),
        "http://www.onvif.org/ver20/ptz/wsdl/SetPreset" => soap(
            r#"xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl""#,
            r#"<tptz:SetPresetResponse>
              <tptz:PresetToken>Preset_WF</tptz:PresetToken>
            </tptz:SetPresetResponse>"#,
        ),
        "http://www.onvif.org/ver20/ptz/wsdl/RemovePreset" => empty("tptz", "RemovePresetResponse"),

        // ── Media2 ────────────────────────────────────────────────────────────
        "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurations" => soap(
            r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
            r#"<tr2:GetVideoSourceConfigurationsResponse>
              <tr2:Configurations token="VSC_1">
                <tt:Name>VSConfig1</tt:Name><tt:UseCount>2</tt:UseCount>
                <tt:SourceToken>VS_1</tt:SourceToken>
                <tt:Bounds x="0" y="0" width="1920" height="1080"/>
              </tr2:Configurations>
            </tr2:GetVideoSourceConfigurationsResponse>"#,
        ),
        "http://www.onvif.org/ver20/media/wsdl/SetVideoSourceConfiguration" => {
            empty("tr2", "SetVideoSourceConfigurationResponse")
        }
        "http://www.onvif.org/ver20/media/wsdl/GetVideoSourceConfigurationOptions" => soap(
            r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
            r#"<tr2:GetVideoSourceConfigurationOptionsResponse>
              <tr2:Options>
                <tt:MaximumNumberOfProfiles>5</tt:MaximumNumberOfProfiles>
                <tt:BoundsRange>
                  <tt:XRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:XRange>
                  <tt:YRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:YRange>
                  <tt:WidthRange><tt:Min>160</tt:Min><tt:Max>1920</tt:Max></tt:WidthRange>
                  <tt:HeightRange><tt:Min>90</tt:Min><tt:Max>1080</tt:Max></tt:HeightRange>
                </tt:BoundsRange>
                <tt:VideoSourceTokensAvailable>VS_1</tt:VideoSourceTokensAvailable>
              </tr2:Options>
            </tr2:GetVideoSourceConfigurationOptionsResponse>"#,
        ),
        "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurations" => soap(
            r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
            r#"<tr2:GetVideoEncoderConfigurationsResponse>
              <tr2:Configurations token="VEC2_1">
                <tt:Name>MainStream-H265</tt:Name><tt:UseCount>1</tt:UseCount>
                <tt:Encoding>H265</tt:Encoding>
                <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
                <tt:Quality>7</tt:Quality>
                <tt:RateControl>
                  <tt:FrameRateLimit>30</tt:FrameRateLimit>
                  <tt:BitrateLimit>8192</tt:BitrateLimit>
                </tt:RateControl>
                <tt:GovLength>30</tt:GovLength>
                <tt:Profile>Main</tt:Profile>
              </tr2:Configurations>
            </tr2:GetVideoEncoderConfigurationsResponse>"#,
        ),
        "http://www.onvif.org/ver20/media/wsdl/SetVideoEncoderConfiguration" => {
            empty("tr2", "SetVideoEncoderConfigurationResponse")
        }
        "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderConfigurationOptions" => soap(
            r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
            r#"<tr2:GetVideoEncoderConfigurationOptionsResponse>
              <tr2:Options>
                <tt:Encoding>H265</tt:Encoding>
                <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
                <tt:ResolutionsAvailable><tt:Width>3840</tt:Width><tt:Height>2160</tt:Height></tt:ResolutionsAvailable>
                <tt:ResolutionsAvailable><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:ResolutionsAvailable>
                <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>32768</tt:Max></tt:BitrateRange>
                <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>60</tt:Max></tt:FrameRateRange>
                <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>600</tt:Max></tt:GovLengthRange>
                <tt:ProfilesSupported>Main</tt:ProfilesSupported>
              </tr2:Options>
            </tr2:GetVideoEncoderConfigurationOptionsResponse>"#,
        ),
        "http://www.onvif.org/ver20/media/wsdl/GetVideoEncoderInstances" => soap(
            r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
            r#"<tr2:GetVideoEncoderInstancesResponse>
              <tr2:Info>
                <tt:Total>4</tt:Total>
                <tt:Encoding><tt:Encoding>H265</tt:Encoding><tt:Number>2</tt:Number></tt:Encoding>
                <tt:Encoding><tt:Encoding>H264</tt:Encoding><tt:Number>2</tt:Number></tt:Encoding>
              </tr2:Info>
            </tr2:GetVideoEncoderInstancesResponse>"#,
        ),
        "http://www.onvif.org/ver20/media/wsdl/CreateProfile" => soap(
            r#"xmlns:tr2="http://www.onvif.org/ver20/media/wsdl""#,
            r#"<tr2:CreateProfileResponse>
              <tr2:Token>Profile_WF_M2</tr2:Token>
            </tr2:CreateProfileResponse>"#,
        ),
        "http://www.onvif.org/ver20/media/wsdl/DeleteProfile" => {
            empty("tr2", "DeleteProfileResponse")
        }

        other => {
            eprintln!("  [WARN] unhandled action: {other}");
            soap(
                "",
                &format!(
                    r#"<s:Fault><s:Code><s:Value>s:Receiver</s:Value></s:Code><s:Reason><s:Text xml:lang="en">Not implemented: {other}</s:Text></s:Reason></s:Fault>"#
                ),
            )
        }
    }
}

/// Start the embedded mock on an OS-assigned port. Returns the base URL.
async fn start_mock() -> String {
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(addr).await.expect("bind mock");
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}");

    let state = Arc::new(MockState { base: base.clone() });
    let app = Router::new()
        .route("/{*path}", post(handle))
        .with_state(state);

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock serve");
    });

    base
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn ok(label: &str) {
    println!("  ✓ {label}");
}

fn show<T: std::fmt::Debug>(label: &str, result: Result<T, oxvif::OnvifError>) {
    match result {
        Ok(v) => println!("  ✓ {label} → {v:?}"),
        Err(e) => println!("  ✗ {label}: {e}"),
    }
}

fn check(label: &str, result: Result<(), oxvif::OnvifError>) {
    match result {
        Ok(()) => ok(label),
        Err(e) => println!("  ✗ {label}: {e}"),
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let base = start_mock().await;

    let device_url =
        std::env::var("WRITE_WF_URL").unwrap_or_else(|_| format!("{base}/onvif/device"));
    let media_url = format!("{base}/onvif/media");
    let ptz_url = format!("{base}/onvif/ptz");
    let media2_url = format!("{base}/onvif/media2");

    let c = oxvif::OnvifClient::new(&device_url);

    println!();
    println!("oxvif write-workflow — all Set/Create/Delete operations");
    println!("  device: {device_url}");
    println!();

    // ── Device ────────────────────────────────────────────────────────────────
    println!("── Device ────────────────────────────────────────────────────────");

    check(
        "set_hostname(\"write-wf-camera\")",
        c.set_hostname("write-wf-camera").await,
    );
    check(
        "set_ntp(false, [\"pool.ntp.org\", \"time.cloudflare.com\"])",
        c.set_ntp(false, &["pool.ntp.org", "time.cloudflare.com"])
            .await,
    );
    show("system_reboot()", c.system_reboot().await);

    check(
        "set_network_protocols([HTTP:80, HTTPS:443, RTSP:554])",
        c.set_network_protocols(&[
            ("HTTP", true, [80u32].as_slice()),
            ("HTTPS", true, [443u32].as_slice()),
            ("RTSP", true, [554u32].as_slice()),
        ])
        .await,
    );

    check(
        "set_dns(false, [\"8.8.8.8\", \"8.8.4.4\"])",
        c.set_dns(false, &["8.8.8.8", "8.8.4.4"]).await,
    );

    // set_network_interfaces returns RebootNeeded bool
    show(
        "set_network_interfaces(eth0, 192.168.1.100/24, static)",
        c.set_network_interfaces("eth0", true, "192.168.1.100", 24, false)
            .await,
    );

    check(
        "set_relay_output_settings(RelayOutput_1, Bistable, open, PT0S)",
        c.set_relay_output_settings("RelayOutput_1", "Bistable", "PT0S", "open")
            .await,
    );
    check(
        "set_relay_output_state(RelayOutput_1, active)",
        c.set_relay_output_state("RelayOutput_1", "active").await,
    );

    check(
        "set_system_factory_default(\"Soft\")",
        c.set_system_factory_default("Soft").await,
    );

    check(
        "set_storage_configuration(\"SD_01\", LocalStorage, /mnt/sd)",
        c.set_storage_configuration("SD_01", "LocalStorage", "/mnt/sd", "", "", true)
            .await,
    );

    check(
        "set_discovery_mode(\"Discoverable\")",
        c.set_discovery_mode("Discoverable").await,
    );

    check(
        "create_users([(viewer, pass123, Viewer)])",
        c.create_users(&[("viewer", "pass123", "Viewer")]).await,
    );
    check("delete_users([viewer])", c.delete_users(&["viewer"]).await);
    check(
        "set_user(admin, Administrator)",
        c.set_user("admin", None, "Administrator").await,
    );

    println!();

    // ── Media1 ────────────────────────────────────────────────────────────────
    println!("── Media1 ────────────────────────────────────────────────────────");

    // Get → modify → set video source configuration
    let vsc = c.get_video_source_configuration(&media_url, "VSC_1").await;
    if let Ok(mut vsc) = vsc {
        vsc.name = "VSConfig1-updated".into();
        check(
            "set_video_source_configuration(VSC_1)",
            c.set_video_source_configuration(&media_url, &vsc).await,
        );
    }

    let opts = c
        .get_video_source_configuration_options(&media_url, Some("VSC_1"))
        .await;
    match opts {
        Ok(o) => println!(
            "  ✓ get_video_source_configuration_options → max_profiles={}, sources={:?}",
            o.max_limit.unwrap_or(0),
            o.source_tokens
        ),
        Err(e) => println!("  ✗ get_video_source_configuration_options: {e}"),
    }

    // Get → modify → set video encoder configuration
    let vec1 = c.get_video_encoder_configuration(&media_url, "VEC_1").await;
    if let Ok(mut vec1) = vec1 {
        vec1.quality = 8.0;
        check(
            "set_video_encoder_configuration(VEC_1)",
            c.set_video_encoder_configuration(&media_url, &vec1).await,
        );
    }

    let venc_opts = c
        .get_video_encoder_configuration_options(&media_url, Some("VEC_1"))
        .await;
    match venc_opts {
        Ok(o) => println!(
            "  ✓ get_video_encoder_configuration_options → h264_profiles={:?}",
            o.h264.map(|h| h.profiles).unwrap_or_default()
        ),
        Err(e) => println!("  ✗ get_video_encoder_configuration_options: {e}"),
    }

    // create_profile → delete_profile
    let profile_token = c.create_profile(&media_url, "write-wf-test", None).await;
    match &profile_token {
        Ok(p) => println!("  ✓ create_profile → token={}", p.token),
        Err(e) => println!("  ✗ create_profile: {e}"),
    }
    if let Ok(p) = profile_token {
        check(
            "delete_profile(Profile_WF)",
            c.delete_profile(&media_url, &p.token).await,
        );
    }

    // get_osd → set_osd
    let osd = c.get_osd(&media_url, "OSD_1").await;
    if let Ok(mut osd) = osd {
        osd.position.type_ = "LowerRight".into();
        check(
            "set_osd(OSD_1, LowerRight)",
            c.set_osd(&media_url, &osd).await,
        );
    }

    // create_osd → delete_osd
    let new_osd = OsdConfiguration {
        token: String::new(),
        video_source_config_token: "VSC_1".into(),
        type_: "Text".into(),
        position: OsdPosition {
            type_: "UpperLeft".into(),
            x: None,
            y: None,
        },
        text_string: Some(OsdTextString {
            type_: "Plain".into(),
            plain_text: Some("oxvif write-workflow".into()),
            date_format: None,
            time_format: None,
            font_size: None,
            font_color: None,
            background_color: None,
            is_persistent_text: None,
        }),
        image_path: None,
    };
    let new_osd_token = c.create_osd(&media_url, &new_osd).await;
    match &new_osd_token {
        Ok(tok) => println!("  ✓ create_osd → token={tok}"),
        Err(e) => println!("  ✗ create_osd: {e}"),
    }
    if let Ok(tok) = new_osd_token {
        check("delete_osd(OSD_WF)", c.delete_osd(&media_url, &tok).await);
    }

    let osd_opts = c.get_osd_options(&media_url, "VSC_1").await;
    match osd_opts {
        Ok(o) => println!(
            "  ✓ get_osd_options → max={}, types={:?}, text_types={:?}",
            o.max_osd, o.types, o.text_types
        ),
        Err(e) => println!("  ✗ get_osd_options: {e}"),
    }

    // get_audio_encoder_configuration → set_audio_encoder_configuration
    let aec = c.get_audio_encoder_configuration(&media_url, "AEC_1").await;
    if let Ok(mut aec) = aec {
        aec.bitrate = 64;
        aec.sample_rate = 8;
        check(
            "set_audio_encoder_configuration(AEC_1)",
            c.set_audio_encoder_configuration(&media_url, &aec).await,
        );
    }

    let aec_opts = c
        .get_audio_encoder_configuration_options(&media_url, "AEC_1")
        .await;
    match aec_opts {
        Ok(o) => {
            let encodings: Vec<_> = o
                .options
                .iter()
                .map(|e| format!("{}", e.encoding))
                .collect();
            println!("  ✓ get_audio_encoder_configuration_options → encodings={encodings:?}");
        }
        Err(e) => println!("  ✗ get_audio_encoder_configuration_options: {e}"),
    }

    println!();

    // ── PTZ ───────────────────────────────────────────────────────────────────
    println!("── PTZ ───────────────────────────────────────────────────────────");

    // get_configuration → set_configuration
    let ptz_cfg = c.ptz_get_configuration(&ptz_url, "PTZConfig_1").await;
    if let Ok(mut cfg) = ptz_cfg {
        cfg.default_ptz_timeout = Some("PT5S".into());
        check(
            "ptz_set_configuration(PTZConfig_1, timeout=PT5S)",
            c.ptz_set_configuration(&ptz_url, &cfg, true).await,
        );
    }

    let ptz_opts = c
        .ptz_get_configuration_options(&ptz_url, "PTZConfig_1")
        .await;
    match ptz_opts {
        Ok(o) => println!(
            "  ✓ ptz_get_configuration_options → timeout=[{}, {}]",
            o.ptz_timeout_min.as_deref().unwrap_or("?"),
            o.ptz_timeout_max.as_deref().unwrap_or("?")
        ),
        Err(e) => println!("  ✗ ptz_get_configuration_options: {e}"),
    }

    // set_preset → remove_preset
    let preset_token = c
        .ptz_set_preset(&ptz_url, "Profile_1", Some("WriteWF-Preset"), None)
        .await;
    match &preset_token {
        Ok(tok) => println!("  ✓ ptz_set_preset → token={tok}"),
        Err(e) => println!("  ✗ ptz_set_preset: {e}"),
    }
    if let Ok(tok) = preset_token {
        check(
            "ptz_remove_preset(Profile_1, Preset_WF)",
            c.ptz_remove_preset(&ptz_url, "Profile_1", &tok).await,
        );
    }

    println!();

    // ── Media2 ────────────────────────────────────────────────────────────────
    println!("── Media2 ────────────────────────────────────────────────────────");

    // get VSCs → set first one
    let vscs_m2 = c.get_video_source_configurations_media2(&media2_url).await;
    if let Ok(mut vscs) = vscs_m2 {
        if let Some(vsc) = vscs.first_mut() {
            vsc.name = "VSConfig1-m2-updated".into();
            check(
                "set_video_source_configuration_media2(VSC_1)",
                c.set_video_source_configuration_media2(&media2_url, vsc)
                    .await,
            );
        }
    }

    let vsc_opts_m2 = c
        .get_video_source_configuration_options_media2(&media2_url, Some("VSC_1"))
        .await;
    match vsc_opts_m2 {
        Ok(o) => println!(
            "  ✓ get_video_source_configuration_options_media2 → max_profiles={}, sources={:?}",
            o.max_limit.unwrap_or(0),
            o.source_tokens
        ),
        Err(e) => println!("  ✗ get_video_source_configuration_options_media2: {e}"),
    }

    // get VEC2 → set it back
    let vec2 = c
        .get_video_encoder_configuration_media2(&media2_url, "VEC2_1")
        .await;
    if let Ok(mut v) = vec2 {
        v.quality = 8.0;
        check(
            "set_video_encoder_configuration_media2(VEC2_1)",
            c.set_video_encoder_configuration_media2(&media2_url, &v)
                .await,
        );
    }

    let vec_opts_m2 = c
        .get_video_encoder_configuration_options_media2(&media2_url, Some("VEC2_1"))
        .await;
    match vec_opts_m2 {
        Ok(o) => {
            let encodings: Vec<_> = o
                .options
                .iter()
                .map(|e| format!("{}", e.encoding))
                .collect();
            println!(
                "  ✓ get_video_encoder_configuration_options_media2 → encodings={encodings:?}"
            );
        }
        Err(e) => println!("  ✗ get_video_encoder_configuration_options_media2: {e}"),
    }

    let instances = c
        .get_video_encoder_instances_media2(&media2_url, "VSC_1")
        .await;
    match instances {
        Ok(i) => {
            let enc: Vec<_> = i
                .encodings
                .iter()
                .map(|e| format!("{}×{}", e.encoding, e.number))
                .collect();
            println!(
                "  ✓ get_video_encoder_instances_media2 → total={}, [{enc}]",
                i.total,
                enc = enc.join(", ")
            );
        }
        Err(e) => println!("  ✗ get_video_encoder_instances_media2: {e}"),
    }

    // create_profile_media2 → delete_profile_media2
    let m2_token = c
        .create_profile_media2(&media2_url, "write-wf-media2")
        .await;
    match &m2_token {
        Ok(tok) => println!("  ✓ create_profile_media2 → token={tok}"),
        Err(e) => println!("  ✗ create_profile_media2: {e}"),
    }
    if let Ok(tok) = m2_token {
        check(
            "delete_profile_media2(Profile_WF_M2)",
            c.delete_profile_media2(&media2_url, &tok).await,
        );
    }

    println!();
    println!("write-workflow complete.");
    println!();
}
