//! Unit tests for the Device-service methods on `OnvifClient`
//! (`src/client/device.rs`).

use super::*;
use crate::tests::common::*;
use std::sync::Arc;

// ── XML response fixtures ─────────────────────────────────────────────────

fn capabilities_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetCapabilitiesResponse>
              <tds:Capabilities>
                <tt:Device> <tt:XAddr>http://192.168.1.1/onvif/device_service</tt:XAddr> </tt:Device>
                <tt:Media>  <tt:XAddr>http://192.168.1.1/onvif/media_service</tt:XAddr>  </tt:Media>
                <tt:PTZ>    <tt:XAddr>http://192.168.1.1/onvif/ptz_service</tt:XAddr>    </tt:PTZ>
              </tds:Capabilities>
            </tds:GetCapabilitiesResponse>
          </s:Body>
        </s:Envelope>"#
}

fn device_info_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body>
            <tds:GetDeviceInformationResponse>
              <tds:Manufacturer>Hikvision</tds:Manufacturer>
              <tds:Model>DS-2CD2085G1-I</tds:Model>
              <tds:FirmwareVersion>V5.6.1</tds:FirmwareVersion>
              <tds:SerialNumber>SN123456</tds:SerialNumber>
              <tds:HardwareId>0x00</tds:HardwareId>
            </tds:GetDeviceInformationResponse>
          </s:Body>
        </s:Envelope>"#
}

fn soap_fault_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
          <s:Body>
            <s:Fault>
              <s:Code><s:Value>s:Sender</s:Value></s:Code>
              <s:Reason><s:Text xml:lang="en">Not Authorized</s:Text></s:Reason>
            </s:Fault>
          </s:Body>
        </s:Envelope>"#
}

// ── get_capabilities ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_capabilities_returns_correct_urls() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(capabilities_xml()));

    let caps = client.get_capabilities().await.unwrap();
    assert_eq!(
        caps.device.url.as_deref(),
        Some("http://192.168.1.1/onvif/device_service")
    );
    assert_eq!(
        caps.media.url.as_deref(),
        Some("http://192.168.1.1/onvif/media_service")
    );
    assert_eq!(
        caps.ptz.url.as_deref(),
        Some("http://192.168.1.1/onvif/ptz_service")
    );
}

#[tokio::test]
async fn test_get_capabilities_soap_fault_returns_error() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(soap_fault_xml()));

    let err = client.get_capabilities().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::Fault { .. })
    ));
}

#[tokio::test]
async fn test_get_capabilities_transport_error_propagates() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(Arc::new(ErrorTransport { status: 503 }));

    let err = client.get_capabilities().await.unwrap_err();
    assert!(matches!(err, OnvifError::Transport(_)));
}

// ── WS-Security ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_credentials_add_ws_security_header() {
    let (transport, captured) = RecordingTransport::new(capabilities_xml());
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_credentials("admin", "password")
        .with_transport(transport);

    client.get_capabilities().await.unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        body.contains("<wsse:Security>"),
        "WS-Security element must be present"
    );
    assert!(body.contains("<wsse:Username>admin</wsse:Username>"));
}

#[tokio::test]
async fn test_no_credentials_omits_security_header() {
    let (transport, captured) = RecordingTransport::new(capabilities_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.get_capabilities().await.unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        !body.contains("<wsse:Security>"),
        "no credentials → no security header"
    );
}

// ── get_device_info ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_device_info_returns_correct_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(device_info_xml()));

    let info = client.get_device_info().await.unwrap();
    assert_eq!(info.manufacturer, "Hikvision");
    assert_eq!(info.model, "DS-2CD2085G1-I");
    assert_eq!(info.firmware_version, "V5.6.1");
    assert_eq!(info.serial_number, "SN123456");
    assert_eq!(info.hardware_id, "0x00");
}

// ── Device management fixtures ────────────────────────────────────────────

fn hostname_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetHostnameResponse>
              <tds:HostnameInformation>
                <tt:FromDHCP>false</tt:FromDHCP>
                <tt:Name>ONVIF-Camera</tt:Name>
              </tds:HostnameInformation>
            </tds:GetHostnameResponse>
          </s:Body>
        </s:Envelope>"#
}

fn hostname_dhcp_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetHostnameResponse>
              <tds:HostnameInformation>
                <tt:FromDHCP>true</tt:FromDHCP>
              </tds:HostnameInformation>
            </tds:GetHostnameResponse>
          </s:Body>
        </s:Envelope>"#
}

fn ntp_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetNTPResponse>
              <tds:NTPInformation>
                <tt:FromDHCP>false</tt:FromDHCP>
                <tt:NTPManual>
                  <tt:Type>DNS</tt:Type>
                  <tt:DNSname>pool.ntp.org</tt:DNSname>
                </tt:NTPManual>
                <tt:NTPManual>
                  <tt:Type>IPv4</tt:Type>
                  <tt:IPv4Address>192.168.1.1</tt:IPv4Address>
                </tt:NTPManual>
              </tds:NTPInformation>
            </tds:GetNTPResponse>
          </s:Body>
        </s:Envelope>"#
}

fn system_reboot_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body>
            <tds:SystemRebootResponse>
              <tds:Message>Rebooting in 30 seconds</tds:Message>
            </tds:SystemRebootResponse>
          </s:Body>
        </s:Envelope>"#
}

// ── get_hostname ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_hostname_returns_name_and_flag() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(hostname_xml()));

    let h = client.get_hostname().await.unwrap();
    assert!(!h.from_dhcp);
    assert_eq!(h.name.as_deref(), Some("ONVIF-Camera"));
}

#[tokio::test]
async fn test_get_hostname_dhcp_no_name() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(hostname_dhcp_xml()));

    let h = client.get_hostname().await.unwrap();
    assert!(h.from_dhcp);
    assert!(h.name.is_none());
}

#[tokio::test]
async fn test_get_hostname_uses_device_url() {
    let (transport, captured) = RecordingTransport::new(hostname_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.get_hostname().await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.url, "http://192.168.1.1/onvif/device_service");
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/GetHostname"
    );
}

// ── set_hostname ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_set_hostname_sends_name() {
    let set_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body><tds:SetHostnameResponse/></s:Body>
        </s:Envelope>"#;

    let (transport, captured) = RecordingTransport::new(set_xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.set_hostname("NewName").await.unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("NewName"));
}

// ── get_ntp ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_ntp_returns_servers() {
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(ntp_xml()));

    let ntp = client.get_ntp().await.unwrap();
    assert!(!ntp.from_dhcp);
    assert_eq!(ntp.servers.len(), 2);
    assert_eq!(ntp.servers[0], "pool.ntp.org");
    assert_eq!(ntp.servers[1], "192.168.1.1");
}

// ── set_ntp ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_set_ntp_sends_from_dhcp_false_and_servers() {
    let set_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body><tds:SetNTPResponse/></s:Body>
        </s:Envelope>"#;

    let (transport, captured) = RecordingTransport::new(set_xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_ntp(false, &["pool.ntp.org", "time.google.com"])
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("<tds:FromDHCP>false</tds:FromDHCP>"));
    assert!(body.contains("pool.ntp.org"));
    assert!(body.contains("time.google.com"));
}

#[tokio::test]
async fn test_set_ntp_from_dhcp_true_sends_no_servers() {
    let set_xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                          xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body><tds:SetNTPResponse/></s:Body>
        </s:Envelope>"#;

    let (transport, captured) = RecordingTransport::new(set_xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.set_ntp(true, &[]).await.unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("<tds:FromDHCP>true</tds:FromDHCP>"));
    assert!(!body.contains("NTPManual"));
}

// ── system_reboot ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_system_reboot_returns_message() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(system_reboot_xml()));

    let msg = client.system_reboot().await.unwrap();
    assert_eq!(msg, "Rebooting in 30 seconds");
}

#[tokio::test]
async fn test_system_reboot_uses_device_url() {
    let (transport, captured) = RecordingTransport::new(system_reboot_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.system_reboot().await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.url, "http://192.168.1.1/onvif/device_service");
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SystemReboot"
    );
}

// ── Negative / error-path tests ───────────────────────────────────────────────

// ── Malformed XML ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_capabilities_malformed_xml_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock("this is not xml at all"));
    let result = client.get_capabilities().await;
    assert!(result.is_err(), "expected Err on malformed XML");
}

#[tokio::test]
async fn test_get_capabilities_soap_fault_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(
        &make_soap_fault_xml("s:Sender", "Sender not Authorized"),
    ));
    let result = client.get_capabilities().await;
    assert!(
        matches!(
            result,
            Err(OnvifError::Soap(crate::soap::SoapError::Fault { ref code, .. }))
            if code == "s:Sender"
        ),
        "expected SOAP Fault error, got: {result:?}"
    );
}

// ── HTTP transport error ──────────────────────────────────────────────────

#[tokio::test]
async fn test_get_capabilities_http_error_returns_err() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(Arc::new(ErrorTransport { status: 401 }));
    let result = client.get_capabilities().await;
    assert!(
        matches!(
            result,
            Err(OnvifError::Transport(
                crate::transport::TransportError::HttpStatus { status: 401, .. }
            ))
        ),
        "expected HTTP 401 transport error"
    );
}

// ── get_scopes ────────────────────────────────────────────────────────────────

fn get_scopes_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                      xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                      xmlns:tt="http://www.onvif.org/ver10/schema">
          <s:Body>
            <tds:GetScopesResponse>
              <tds:Scopes>
                <tt:ScopeAttribute>Fixed</tt:ScopeAttribute>
                <tt:ScopeItem>onvif://www.onvif.org/name/Camera1</tt:ScopeItem>
              </tds:Scopes>
              <tds:Scopes>
                <tt:ScopeAttribute>Fixed</tt:ScopeAttribute>
                <tt:ScopeItem>onvif://www.onvif.org/location/country/taiwan</tt:ScopeItem>
              </tds:Scopes>
            </tds:GetScopesResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_get_scopes_returns_uris() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_scopes_xml()));

    let scopes = client.get_scopes().await.unwrap();

    assert_eq!(scopes.len(), 2);
    assert!(scopes[0].contains("name/Camera1"));
    assert!(scopes[1].contains("country/taiwan"));
}

// ── set_scopes ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_set_scopes_sends_scope_elements() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetScopesResponse/></s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .set_scopes(&[
            "onvif://www.onvif.org/name/FrontDoor",
            "onvif://www.onvif.org/location/Building1",
        ])
        .await
        .unwrap();
    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("onvif://www.onvif.org/name/FrontDoor"));
    assert!(body.contains("onvif://www.onvif.org/location/Building1"));
}

#[tokio::test]
async fn test_set_scopes_xml_escapes_value() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetScopesResponse/></s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .set_scopes(&["onvif://www.onvif.org/name/<&>"])
        .await
        .unwrap();
    assert!(captured.lock().unwrap().body.contains("&lt;&amp;&gt;"));
}

#[tokio::test]
async fn test_set_scopes_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("env:Sender", "InvalidScope");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let res = client.set_scopes(&["onvif://www.onvif.org/name/Bad"]).await;
    assert!(res.is_err());
}

// ── set_system_date_and_time ──────────────────────────────────────────────────

#[tokio::test]
async fn test_set_system_date_and_time_manual_sends_utc_fields() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetSystemDateAndTimeResponse/></s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .set_system_date_and_time(&crate::types::SetDateTimeRequest {
            datetime_type: "Manual".into(),
            daylight_savings: false,
            timezone: "CST-8".into(),
            utc_datetime: Some(crate::types::UtcDateTime {
                year: 2026,
                month: 4,
                day: 5,
                hour: 10,
                minute: 30,
                second: 0,
            }),
        })
        .await
        .unwrap();
    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("Manual"));
    assert!(body.contains("CST-8"));
    assert!(body.contains("2026"));
    assert!(body.contains("10"));
    assert!(body.contains("30"));
}

#[tokio::test]
async fn test_set_system_date_and_time_ntp_omits_utc_element() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetSystemDateAndTimeResponse/></s:Body>
       </s:Envelope>"#;
    let (transport, captured) = RecordingTransport::new(xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);
    client
        .set_system_date_and_time(&crate::types::SetDateTimeRequest {
            datetime_type: "NTP".into(),
            daylight_savings: false,
            timezone: "UTC".into(),
            utc_datetime: None,
        })
        .await
        .unwrap();
    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("NTP"));
    assert!(!body.contains("UTCDateTime"));
}

#[tokio::test]
async fn test_set_system_date_and_time_soap_fault_returns_err() {
    let xml = make_soap_fault_xml("env:Sender", "InvalidDateTimeType");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));
    let res = client
        .set_system_date_and_time(&crate::types::SetDateTimeRequest {
            datetime_type: "Manual".into(),
            daylight_savings: false,
            timezone: "UTC".into(),
            utc_datetime: None,
        })
        .await;
    assert!(res.is_err());
}

// ── get_users ─────────────────────────────────────────────────────────────────

fn get_users_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetUsersResponse>
             <tds:User>
               <tt:Username>admin</tt:Username>
               <tt:UserLevel>Administrator</tt:UserLevel>
             </tds:User>
             <tds:User>
               <tt:Username>operator</tt:Username>
               <tt:UserLevel>Operator</tt:UserLevel>
             </tds:User>
           </tds:GetUsersResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_users_returns_list() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_users_xml()));

    let users = client.get_users().await.unwrap();
    assert_eq!(users.len(), 2);
    assert_eq!(users[0].username, "admin");
    assert_eq!(users[0].user_level, "Administrator");
    assert_eq!(users[1].username, "operator");
}

// ── create_users ──────────────────────────────────────────────────────────────

fn create_users_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:CreateUsersResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_create_users_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(create_users_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .create_users(&[("newuser", "pass123", "Operator")])
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/CreateUsers"
    );
    assert!(c.body.contains("<tt:Username>newuser</tt:Username>"));
    assert!(c.body.contains("<tt:UserLevel>Operator</tt:UserLevel>"));
}

// ── delete_users ──────────────────────────────────────────────────────────────

fn delete_users_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:DeleteUsersResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_delete_users_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(delete_users_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.delete_users(&["operator"]).await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/DeleteUsers"
    );
    assert!(c.body.contains("<tds:Username>operator</tds:Username>"));
}

#[tokio::test]
async fn test_delete_users_transport_error() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(Arc::new(ErrorTransport { status: 500 }));
    let err = client.delete_users(&["operator"]).await.unwrap_err();
    assert!(matches!(err, OnvifError::Transport(_)));
}

// ── set_user ─────────────────────────────────────────────────────────────────

fn set_user_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetUserResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_user_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_user_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_user("admin", Some("newpass"), "Administrator")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.action, "http://www.onvif.org/ver10/device/wsdl/SetUser");
    assert!(c.body.contains("<tt:Username>admin</tt:Username>"));
    assert!(c.body.contains("<tt:Password>newpass</tt:Password>"));
    assert!(
        c.body
            .contains("<tt:UserLevel>Administrator</tt:UserLevel>")
    );
}

// ── get_network_interfaces ────────────────────────────────────────────────────

fn get_network_interfaces_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkInterfacesResponse>
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
                   <tt:Manual>
                     <tt:Address>192.168.1.100</tt:Address>
                     <tt:PrefixLength>24</tt:PrefixLength>
                   </tt:Manual>
                   <tt:DHCP>false</tt:DHCP>
                 </tt:Config>
               </tt:IPv4>
             </tds:NetworkInterfaces>
           </tds:GetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_network_interfaces_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_network_interfaces_xml()));

    let ifaces = client.get_network_interfaces().await.unwrap();
    assert_eq!(ifaces.len(), 1);
    let iface = &ifaces[0];
    assert_eq!(iface.token, "eth0");
    assert!(iface.enabled);
    assert_eq!(iface.name, "eth0");
    assert_eq!(iface.hw_address, "00:11:22:33:44:55");
    assert_eq!(iface.mtu, 1500);
    assert_eq!(iface.ipv4_address, "192.168.1.100");
    assert_eq!(iface.ipv4_prefix_length, 24);
    assert!(!iface.ipv4_from_dhcp);
}

#[tokio::test]
async fn test_get_network_interfaces_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkInterfacesResponse>
             <tds:NetworkInterfaces>
               <tt:Enabled>true</tt:Enabled>
             </tds:NetworkInterfaces>
           </tds:GetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_network_interfaces().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── set_network_interfaces ────────────────────────────────────────────────────

fn set_network_interfaces_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:SetNetworkInterfacesResponse>
             <tds:RebootNeeded>false</tds:RebootNeeded>
           </tds:SetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_network_interfaces_sends_ipv4_body() {
    use crate::types::{IpStackConfig, ManualAddress, NetworkInterfaceConfig};
    let (transport, captured) = RecordingTransport::new(set_network_interfaces_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let cfg = NetworkInterfaceConfig {
        enabled: true,
        mtu: None,
        ipv4: Some(IpStackConfig {
            enabled: true,
            from_dhcp: false,
            manual: vec![ManualAddress {
                address: "192.168.1.200".into(),
                prefix_length: 24,
            }],
        }),
        ipv6: None,
    };
    let reboot = client.set_network_interfaces("eth0", &cfg).await.unwrap();

    assert!(!reboot);
    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkInterfaces"
    );
    assert!(
        c.body
            .contains("<tds:InterfaceToken>eth0</tds:InterfaceToken>")
    );
    assert!(c.body.contains("<tt:IPv4>"));
    assert!(c.body.contains("<tt:Address>192.168.1.200</tt:Address>"));
    assert!(c.body.contains("<tt:PrefixLength>24</tt:PrefixLength>"));
    assert!(c.body.contains("<tt:DHCP>false</tt:DHCP>"));
    // No IPv6 block when ipv6 is None.
    assert!(!c.body.contains("<tt:IPv6>"));
}

#[tokio::test]
async fn test_set_network_interfaces_reboot_needed() {
    use crate::types::{IpStackConfig, ManualAddress, NetworkInterfaceConfig};
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body>
           <tds:SetNetworkInterfacesResponse>
             <tds:RebootNeeded>true</tds:RebootNeeded>
           </tds:SetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfg = NetworkInterfaceConfig {
        enabled: true,
        mtu: None,
        ipv4: Some(IpStackConfig {
            enabled: true,
            from_dhcp: false,
            manual: vec![ManualAddress {
                address: "10.0.0.1".into(),
                prefix_length: 8,
            }],
        }),
        ipv6: None,
    };
    let reboot = client.set_network_interfaces("eth0", &cfg).await.unwrap();
    assert!(reboot);
}

#[tokio::test]
async fn test_set_network_interfaces_sends_ipv6_body_and_mtu() {
    use crate::types::{IpStackConfig, ManualAddress, NetworkInterfaceConfig};
    let (transport, captured) = RecordingTransport::new(set_network_interfaces_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    let cfg = NetworkInterfaceConfig {
        enabled: true,
        mtu: Some(1500),
        ipv4: None,
        ipv6: Some(IpStackConfig {
            enabled: true,
            from_dhcp: false,
            manual: vec![
                ManualAddress {
                    address: "2001:db8::1".into(),
                    prefix_length: 64,
                },
                ManualAddress {
                    address: "2001:db8::2".into(),
                    prefix_length: 64,
                },
            ],
        }),
    };
    client.set_network_interfaces("eth0", &cfg).await.unwrap();
    let c = captured.lock().unwrap();
    assert!(c.body.contains("<tt:MTU>1500</tt:MTU>"));
    assert!(c.body.contains("<tt:IPv6>"));
    // Two Manual entries
    let manual_count = c.body.matches("<tt:Manual>").count();
    assert_eq!(
        manual_count, 2,
        "expected 2 Manual entries, body={}",
        c.body
    );
    assert!(c.body.contains("<tt:Address>2001:db8::1</tt:Address>"));
    assert!(c.body.contains("<tt:Address>2001:db8::2</tt:Address>"));
    // IPv6 DHCP bool maps to Stateful/Off enum
    assert!(c.body.contains("<tt:DHCP>Off</tt:DHCP>"));
    // No IPv4 block when ipv4 is None.
    assert!(!c.body.contains("<tt:IPv4>"));
}

// ── get_network_protocols ─────────────────────────────────────────────────────

fn get_network_protocols_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkProtocolsResponse>
             <tds:NetworkProtocols>
               <tt:Name>HTTP</tt:Name>
               <tt:Enabled>true</tt:Enabled>
               <tt:Port>80</tt:Port>
             </tds:NetworkProtocols>
             <tds:NetworkProtocols>
               <tt:Name>RTSP</tt:Name>
               <tt:Enabled>true</tt:Enabled>
               <tt:Port>554</tt:Port>
             </tds:NetworkProtocols>
           </tds:GetNetworkProtocolsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_network_protocols_returns_list() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_network_protocols_xml()));

    let protos = client.get_network_protocols().await.unwrap();
    assert_eq!(protos.len(), 2);
    assert_eq!(protos[0].name, "HTTP");
    assert!(protos[0].enabled);
    assert_eq!(protos[0].ports, vec![80]);
    assert_eq!(protos[1].name, "RTSP");
    assert_eq!(protos[1].ports, vec![554]);
}

// ── get_dns ───────────────────────────────────────────────────────────────────

fn get_dns_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetDNSResponse>
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
           </tds:GetDNSResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_dns_returns_servers() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_dns_xml()));

    let dns = client.get_dns().await.unwrap();
    assert!(!dns.from_dhcp);
    assert_eq!(dns.servers, vec!["8.8.8.8", "8.8.4.4"]);
}

#[tokio::test]
async fn test_get_dns_missing_dns_information_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:GetDNSResponse/></s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_dns().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── set_dns ───────────────────────────────────────────────────────────────────

fn set_dns_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetDNSResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_dns_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_dns_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_dns(false, &["1.1.1.1", "9.9.9.9"])
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(c.action, "http://www.onvif.org/ver10/device/wsdl/SetDNS");
    assert!(c.body.contains("<tds:FromDHCP>false</tds:FromDHCP>"));
    assert!(c.body.contains("<tt:IPv4Address>1.1.1.1</tt:IPv4Address>"));
    assert!(c.body.contains("<tt:IPv4Address>9.9.9.9</tt:IPv4Address>"));
}

// ── get_network_default_gateway ───────────────────────────────────────────────

fn get_network_default_gateway_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkDefaultGatewayResponse>
             <tds:NetworkGateway>
               <tt:IPv4Address>192.168.1.1</tt:IPv4Address>
             </tds:NetworkGateway>
           </tds:GetNetworkDefaultGatewayResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_network_default_gateway_returns_address() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_network_default_gateway_xml()));

    let gw = client.get_network_default_gateway().await.unwrap();
    assert_eq!(gw.ipv4_addresses, vec!["192.168.1.1"]);
    assert!(gw.ipv6_addresses.is_empty());
}

#[tokio::test]
async fn test_get_network_default_gateway_missing_node_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:GetNetworkDefaultGatewayResponse/></s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_network_default_gateway().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── get_system_log ────────────────────────────────────────────────────────────

fn get_system_log_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetSystemLogResponse>
             <tds:SystemLog>
               <tt:String>2026-04-03 12:00:00 system started</tt:String>
             </tds:SystemLog>
           </tds:GetSystemLogResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_system_log_returns_string() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_system_log_xml()));

    let log = client.get_system_log("System").await.unwrap();
    assert_eq!(
        log.string.as_deref(),
        Some("2026-04-03 12:00:00 system started")
    );
}

#[tokio::test]
async fn test_get_system_log_missing_system_log_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:GetSystemLogResponse/></s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_system_log("System").await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── get_relay_outputs ─────────────────────────────────────────────────────────

fn get_relay_outputs_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetRelayOutputsResponse>
             <tds:RelayOutputs token="RelayOutput_1">
               <tt:Properties>
                 <tt:Mode>Bistable</tt:Mode>
                 <tt:DelayTime>PT0S</tt:DelayTime>
                 <tt:IdleState>open</tt:IdleState>
               </tt:Properties>
             </tds:RelayOutputs>
           </tds:GetRelayOutputsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_relay_outputs_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_relay_outputs_xml()));

    let relays = client.get_relay_outputs().await.unwrap();
    assert_eq!(relays.len(), 1);
    assert_eq!(relays[0].token, "RelayOutput_1");
    assert_eq!(relays[0].mode, "Bistable");
    assert_eq!(relays[0].idle_state, "open");
}

#[tokio::test]
async fn test_get_relay_outputs_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetRelayOutputsResponse>
             <tds:RelayOutputs>
               <tt:Properties><tt:Mode>Bistable</tt:Mode></tt:Properties>
             </tds:RelayOutputs>
           </tds:GetRelayOutputsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_relay_outputs().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

// ── set_relay_output_state ────────────────────────────────────────────────────

fn set_relay_output_state_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetRelayOutputStateResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_relay_output_state_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_relay_output_state_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_relay_output_state("RelayOutput_1", "active")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputState"
    );
    assert!(
        c.body
            .contains("<tds:RelayOutputToken>RelayOutput_1</tds:RelayOutputToken>")
    );
    assert!(
        c.body
            .contains("<tds:LogicalState>active</tds:LogicalState>")
    );
}

// ── set_relay_output_settings ─────────────────────────────────────────────────

fn set_relay_output_settings_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetRelayOutputSettingsResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_relay_output_settings_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_relay_output_settings_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_relay_output_settings("Relay_1", "Monostable", "PT2S", "open")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputSettings"
    );
    assert!(
        c.body
            .contains("<tds:RelayOutputToken>Relay_1</tds:RelayOutputToken>")
    );
    assert!(c.body.contains("<tt:Mode>Monostable</tt:Mode>"));
    assert!(c.body.contains("<tt:DelayTime>PT2S</tt:DelayTime>"));
    assert!(c.body.contains("<tt:IdleState>open</tt:IdleState>"));
}

// ── get_digital_inputs ────────────────────────────────────────────────────────

fn get_digital_inputs_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body>
           <tds:GetDigitalInputsResponse>
             <tds:DigitalInputs token="DigitalInput_1" IdleState="closed"/>
             <tds:DigitalInputs token="DigitalInput_2" IdleState="open"/>
           </tds:GetDigitalInputsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_digital_inputs_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_digital_inputs_xml()));

    let inputs = client.get_digital_inputs().await.unwrap();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0].token, "DigitalInput_1");
    assert_eq!(inputs[0].idle_state, "closed");
    assert_eq!(inputs[1].token, "DigitalInput_2");
    assert_eq!(inputs[1].idle_state, "open");
}

#[tokio::test]
async fn test_get_digital_inputs_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body>
           <tds:GetDigitalInputsResponse>
             <tds:DigitalInputs IdleState="closed"/>
           </tds:GetDigitalInputsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_digital_inputs().await.unwrap_err();
    assert!(matches!(err, OnvifError::Soap(_)));
}

#[tokio::test]
async fn test_get_digital_inputs_missing_idle_state_ok() {
    // Some firmwares omit IdleState entirely. Treat as unknown rather than err.
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body>
           <tds:GetDigitalInputsResponse>
             <tds:DigitalInputs token="DI_A"/>
           </tds:GetDigitalInputsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let inputs = client.get_digital_inputs().await.unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].token, "DI_A");
    assert_eq!(inputs[0].idle_state, "");
}

// ── set_network_protocols ─────────────────────────────────────────────────────

fn set_network_protocols_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetNetworkProtocolsResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_network_protocols_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_network_protocols_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_network_protocols(&[("HTTP", true, &[80u32]), ("RTSP", true, &[554u32])])
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetNetworkProtocols"
    );
    assert!(c.body.contains("<tt:Name>HTTP</tt:Name>"));
    assert!(c.body.contains("<tt:Name>RTSP</tt:Name>"));
    assert!(c.body.contains("<tt:Port>554</tt:Port>"));
}

// ── set_system_factory_default ────────────────────────────────────────────────

fn set_system_factory_default_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetSystemFactoryDefaultResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_system_factory_default_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_system_factory_default_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.set_system_factory_default("Soft").await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault"
    );
    assert!(
        c.body
            .contains("<tds:FactoryDefault>Soft</tds:FactoryDefault>")
    );
}

// ── get_storage_configurations ────────────────────────────────────────────────

fn get_storage_configurations_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetStorageConfigurationsResponse>
             <tds:StorageConfigurations token="SD_01">
               <tt:Data type="LocalStorage">
                 <tt:LocalPath>/mnt/sd</tt:LocalPath>
               </tt:Data>
             </tds:StorageConfigurations>
           </tds:GetStorageConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_storage_configurations_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_storage_configurations_xml()));
    let configs = client.get_storage_configurations().await.unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].token, "SD_01");
    assert_eq!(configs[0].storage_type, "LocalStorage");
    assert_eq!(configs[0].local_path, "/mnt/sd");
}

#[tokio::test]
async fn test_get_storage_configurations_missing_token_returns_err() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetStorageConfigurationsResponse>
             <tds:StorageConfigurations>
               <tt:StorageType>LocalStorage</tt:StorageType>
             </tds:StorageConfigurations>
           </tds:GetStorageConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let err = client.get_storage_configurations().await.unwrap_err();
    assert!(matches!(
        err,
        OnvifError::Soap(crate::soap::SoapError::MissingField(_))
    ));
}

// ── set_storage_configuration ─────────────────────────────────────────────────

fn set_storage_configuration_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetStorageConfigurationResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_storage_configuration_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_storage_configuration_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_storage_configuration("SD_01", "LocalStorage", "/mnt/sd", "", "")
        .await
        .unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetStorageConfiguration"
    );
    assert!(c.body.contains("type=\"LocalStorage\""));
    assert!(c.body.contains("<tt:LocalPath>/mnt/sd</tt:LocalPath>"));
}

// ── get_system_uris ───────────────────────────────────────────────────────────

fn get_system_uris_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetSystemUrisResponse>
             <tds:SystemLogUris>
               <tt:SystemLogUri>
                 <tt:Uri>http://192.168.1.1/log</tt:Uri>
                 <tt:LogType>System</tt:LogType>
               </tt:SystemLogUri>
             </tds:SystemLogUris>
             <tds:SupportInfoUri>http://192.168.1.1/support</tds:SupportInfoUri>
             <tds:SystemBackupUri>http://192.168.1.1/backup</tds:SystemBackupUri>
           </tds:GetSystemUrisResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_system_uris_returns_fields() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_system_uris_xml()));
    let uris = client.get_system_uris().await.unwrap();
    assert_eq!(
        uris.system_log_uri.as_deref(),
        Some("http://192.168.1.1/log")
    );
    assert_eq!(
        uris.support_info_uri.as_deref(),
        Some("http://192.168.1.1/support")
    );
    assert_eq!(
        uris.system_backup_uri.as_deref(),
        Some("http://192.168.1.1/backup")
    );
}

// ── get_discovery_mode ────────────────────────────────────────────────────────

fn get_discovery_mode_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body>
           <tds:GetDiscoveryModeResponse>
             <tds:DiscoveryMode>Discoverable</tds:DiscoveryMode>
           </tds:GetDiscoveryModeResponse>
         </s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_get_discovery_mode_returns_value() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(get_discovery_mode_xml()));
    let mode = client.get_discovery_mode().await.unwrap();
    assert_eq!(mode, "Discoverable");
}

// ── set_discovery_mode ────────────────────────────────────────────────────────

fn set_discovery_mode_response_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
         <s:Body><tds:SetDiscoveryModeResponse/></s:Body>
       </s:Envelope>"#
}

#[tokio::test]
async fn test_set_discovery_mode_sends_correct_body() {
    let (transport, captured) = RecordingTransport::new(set_discovery_mode_response_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client.set_discovery_mode("NonDiscoverable").await.unwrap();

    let c = captured.lock().unwrap();
    assert_eq!(
        c.action,
        "http://www.onvif.org/ver10/device/wsdl/SetDiscoveryMode"
    );
    assert!(
        c.body
            .contains("<tds:DiscoveryMode>NonDiscoverable</tds:DiscoveryMode>")
    );
}

// DnsInformation search_domains

#[tokio::test]
async fn test_get_dns_parses_search_domains() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetDNSResponse>
             <tds:DNSInformation>
               <tt:FromDHCP>false</tt:FromDHCP>
               <tt:SearchDomain>example.com</tt:SearchDomain>
               <tt:SearchDomain>local</tt:SearchDomain>
               <tt:DNSManual>
                 <tt:Type>IPv4</tt:Type>
                 <tt:IPv4Address>1.1.1.1</tt:IPv4Address>
               </tt:DNSManual>
             </tds:DNSInformation>
           </tds:GetDNSResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let dns = client.get_dns().await.unwrap();
    assert_eq!(dns.search_domains, vec!["example.com", "local"]);
    assert_eq!(dns.servers, vec!["1.1.1.1"]);
}

// NetworkInterface IPv6

#[tokio::test]
async fn test_get_network_interfaces_parses_ipv6() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkInterfacesResponse>
             <tds:NetworkInterfaces token="eth0">
               <tt:Enabled>true</tt:Enabled>
               <tt:Info>
                 <tt:Name>eth0</tt:Name>
                 <tt:HwAddress>AA:BB:CC:DD:EE:FF</tt:HwAddress>
                 <tt:MTU>1500</tt:MTU>
               </tt:Info>
               <tt:IPv4>
                 <tt:Enabled>true</tt:Enabled>
                 <tt:Config>
                   <tt:Manual>
                     <tt:Address>10.0.0.1</tt:Address>
                     <tt:PrefixLength>8</tt:PrefixLength>
                   </tt:Manual>
                   <tt:DHCP>false</tt:DHCP>
                 </tt:Config>
               </tt:IPv4>
               <tt:IPv6>
                 <tt:Enabled>true</tt:Enabled>
                 <tt:Config>
                   <tt:DHCP>Stateful</tt:DHCP>
                   <tt:Manual>
                     <tt:Address>2001:db8::1</tt:Address>
                     <tt:PrefixLength>64</tt:PrefixLength>
                   </tt:Manual>
                 </tt:Config>
               </tt:IPv6>
             </tds:NetworkInterfaces>
           </tds:GetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let ifaces = client.get_network_interfaces().await.unwrap();
    let iface = &ifaces[0];
    assert!(iface.ipv6_enabled);
    assert!(iface.ipv6_from_dhcp);
    assert_eq!(iface.ipv6_address.as_deref(), Some("2001:db8::1"));
}

// NetworkInterface DHCP address fallback (some vendors put IP under FromDHCP, not Manual)

#[tokio::test]
async fn test_get_network_interfaces_reads_address_from_dhcp_element() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetNetworkInterfacesResponse>
             <tds:NetworkInterfaces token="eth0">
               <tt:Enabled>true</tt:Enabled>
               <tt:Info>
                 <tt:Name>eth0</tt:Name>
                 <tt:HwAddress>00:13:e2:24:01:c3</tt:HwAddress>
                 <tt:MTU>1500</tt:MTU>
               </tt:Info>
               <tt:IPv4>
                 <tt:Enabled>true</tt:Enabled>
                 <tt:Config>
                   <tt:FromDHCP>
                     <tt:Address>192.168.50.18</tt:Address>
                     <tt:PrefixLength>24</tt:PrefixLength>
                   </tt:FromDHCP>
                   <tt:DHCP>true</tt:DHCP>
                 </tt:Config>
               </tt:IPv4>
             </tds:NetworkInterfaces>
           </tds:GetNetworkInterfacesResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let ifaces = client.get_network_interfaces().await.unwrap();
    let iface = &ifaces[0];
    assert_eq!(iface.ipv4_address, "192.168.50.18");
    assert_eq!(iface.ipv4_prefix_length, 24);
    assert!(iface.ipv4_from_dhcp);
}

#[tokio::test]
async fn test_get_storage_configurations_parses_user() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetStorageConfigurationsResponse>
             <tds:StorageConfigurations token="NAS_1">
               <tt:Data type="NFS">
                 <tt:StorageUri>nfs://192.168.1.50/share</tt:StorageUri>
                 <tt:User>
                   <tt:UserName>admin</tt:UserName>
                 </tt:User>
               </tt:Data>
             </tds:StorageConfigurations>
           </tds:GetStorageConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfgs = client.get_storage_configurations().await.unwrap();
    assert_eq!(cfgs[0].storage_type, "NFS");
    assert_eq!(cfgs[0].storage_uri, "nfs://192.168.1.50/share");
    assert_eq!(cfgs[0].user, "admin");
}

#[tokio::test]
async fn test_get_storage_configurations_no_user_is_empty() {
    let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                     xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
                     xmlns:tt="http://www.onvif.org/ver10/schema">
         <s:Body>
           <tds:GetStorageConfigurationsResponse>
             <tds:StorageConfigurations token="SD_1">
               <tt:Data type="LocalStorage">
                 <tt:LocalPath>/mnt/sd</tt:LocalPath>
               </tt:Data>
             </tds:StorageConfigurations>
           </tds:GetStorageConfigurationsResponse>
         </s:Body>
       </s:Envelope>"#;
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
    let cfgs = client.get_storage_configurations().await.unwrap();
    assert!(cfgs[0].user.is_empty());
}

#[tokio::test]
async fn test_ws_security_escapes_username() {
    let (transport, captured) = RecordingTransport::new(capabilities_xml());
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_credentials("user<&>name", "password")
        .with_transport(transport);

    client.get_capabilities().await.unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(
        body.contains("user&lt;&amp;&gt;name"),
        "username with XML special chars must be escaped: {body}"
    );
}

// ── SetNetworkDefaultGateway ──────────────────────────────────────────────

#[tokio::test]
async fn test_set_network_default_gateway_sends_addresses() {
    let xml = empty_response_xml("SetNetworkDefaultGatewayResponse");
    let (transport, captured) = RecordingTransport::new(&xml);
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .set_network_default_gateway(&["192.168.1.1", "10.0.0.1"])
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("192.168.1.1"));
    assert!(body.contains("10.0.0.1"));
}

#[tokio::test]
async fn test_set_network_default_gateway_soap_fault() {
    let xml = make_soap_fault_xml("s:Sender", "Action not supported");
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(&xml));

    let err = client
        .set_network_default_gateway(&["192.168.1.1"])
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Action not supported"));
}

// ── SendAuxiliaryCommand ──────────────────────────────────────────────────

fn send_auxiliary_command_xml() -> &'static str {
    r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                    xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
          <s:Body>
            <tds:SendAuxiliaryCommandResponse>
              <tds:AuxiliaryCommandResponse>OK</tds:AuxiliaryCommandResponse>
            </tds:SendAuxiliaryCommandResponse>
          </s:Body>
        </s:Envelope>"#
}

#[tokio::test]
async fn test_send_auxiliary_command_returns_response() {
    let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
        .with_transport(mock(send_auxiliary_command_xml()));

    let resp = client.send_auxiliary_command("tt:Wiper|On").await.unwrap();
    assert_eq!(resp, "OK");
}

#[tokio::test]
async fn test_send_auxiliary_command_escapes_input() {
    let (transport, captured) = RecordingTransport::new(send_auxiliary_command_xml());
    let client =
        OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(transport);

    client
        .send_auxiliary_command("tt:Wiper|On&Off")
        .await
        .unwrap();

    let body = captured.lock().unwrap().body.clone();
    assert!(body.contains("On&amp;Off"), "must escape: {body}");
}

// ── NET 3: Stage 2 targets' current good behaviour ────────────────────────────
//
// Stage 2 touches `src/client/device.rs`. `get_discovery_mode` is the operation
// it lands next to, so its happy path is pinned end to end here: the action URI,
// the request body, and the parsed value for both legal modes.

mod discovery_mode_happy_path {
    use super::*;

    fn response(mode: &str) -> String {
        format!(
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                            xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
                 <s:Body>
                   <tds:GetDiscoveryModeResponse>
                     <tds:DiscoveryMode>{mode}</tds:DiscoveryMode>
                   </tds:GetDiscoveryModeResponse>
                 </s:Body>
               </s:Envelope>"#
        )
    }

    #[tokio::test]
    async fn get_discovery_mode_pins_action_body_and_parsed_value() {
        for mode in ["Discoverable", "NonDiscoverable"] {
            let (transport, captured) = RecordingTransport::new(&response(mode));
            let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
                .with_transport(transport);

            let got = client.get_discovery_mode().await.unwrap();
            assert_eq!(got, mode);

            let c = captured.lock().unwrap();
            assert_eq!(c.url, "http://192.168.1.1/onvif/device_service");
            assert_eq!(
                c.action,
                "http://www.onvif.org/ver10/device/wsdl/GetDiscoveryMode"
            );
            assert!(
                c.body.contains("<tds:GetDiscoveryMode/>"),
                "body was: {}",
                c.body
            );
        }
    }

    /// A `GetDiscoveryModeResponse` that omits `DiscoveryMode` currently yields
    /// `Ok("")`, not an error. Pinned so Stage 2 cannot change it by accident.
    #[tokio::test]
    async fn get_discovery_mode_without_the_element_yields_an_empty_string() {
        let xml = r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
                                  xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
                       <s:Body><tds:GetDiscoveryModeResponse/></s:Body>
                     </s:Envelope>"#;
        let client =
            OnvifClient::new("http://192.168.1.1/onvif/device_service").with_transport(mock(xml));
        assert_eq!(client.get_discovery_mode().await.unwrap(), "");
    }

    /// A SOAP Fault on `GetDiscoveryMode` surfaces as `SoapError::Fault` with
    /// the device's code and reason preserved.
    #[tokio::test]
    async fn get_discovery_mode_propagates_a_soap_fault() {
        let client = OnvifClient::new("http://192.168.1.1/onvif/device_service")
            .with_transport(mock(&make_soap_fault_xml("ter:NotAuthorized", "denied")));

        match client.get_discovery_mode().await.unwrap_err() {
            OnvifError::Soap(crate::soap::SoapError::Fault { code, reason, .. }) => {
                assert_eq!(code, "ter:NotAuthorized");
                assert_eq!(reason, "denied");
            }
            other => panic!("expected SoapError::Fault, got {other:?}"),
        }
    }
}
