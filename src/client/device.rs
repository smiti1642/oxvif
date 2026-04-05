// ── Device Service ────────────────────────────────────────────────────────────

use super::OnvifClient;
use crate::error::OnvifError;
use crate::soap::{find_response, parse_soap_body};
use crate::types::{
    Capabilities, DeviceInfo, DnsInformation, Hostname, NetworkGateway, NetworkInterface,
    NetworkProtocol, NtpInfo, OnvifService, RelayOutput, SetDateTimeRequest, StorageConfiguration,
    SystemDateTime, SystemLog, SystemUris, User, xml_escape,
};

impl OnvifClient {
    /// Retrieve service endpoint URLs from the device.
    ///
    /// This is typically the first call made after constructing a client. The
    /// returned [`Capabilities`] provides the URLs needed for all subsequent
    /// media, PTZ, events, and imaging operations.
    pub async fn get_capabilities(&self) -> Result<Capabilities, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetCapabilities";
        const BODY: &str =
            "<tds:GetCapabilities><tds:Category>All</tds:Category></tds:GetCapabilities>";

        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetCapabilitiesResponse")?;
        Capabilities::from_xml(resp)
    }

    /// Retrieve all service endpoints advertised by the device.
    ///
    /// `GetServices` is the correct ONVIF mechanism for discovering every
    /// service URL, including Media2. Many devices do not include the Media2
    /// URL in `GetCapabilities` — call this as a fallback:
    ///
    /// ```no_run
    /// # use oxvif::{OnvifClient, OnvifError};
    /// # async fn run() -> Result<(), OnvifError> {
    /// let client = OnvifClient::new("http://192.168.1.1/onvif/device_service");
    /// let caps   = client.get_capabilities().await?;
    /// let media2_url = match caps.media2.url {
    ///     Some(u) => u,
    ///     None => client.get_services().await?
    ///         .into_iter()
    ///         .find(|s| s.is_media2())
    ///         .map(|s| s.url)
    ///         .expect("device does not support Media2"),
    /// };
    /// # Ok(()) }
    /// ```
    pub async fn get_services(&self) -> Result<Vec<OnvifService>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetServices";
        const BODY: &str = "<tds:GetServices><tds:IncludeCapability>false</tds:IncludeCapability></tds:GetServices>";

        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetServicesResponse")?;
        OnvifService::vec_from_xml(resp)
    }

    /// Retrieve the device clock and compute the UTC offset for WS-Security.
    ///
    /// Call this before [`with_utc_offset`](Self::with_utc_offset) when the
    /// device clock may differ from local UTC:
    ///
    /// ```no_run
    /// # use oxvif::{OnvifClient, OnvifError};
    /// # async fn run() -> Result<(), OnvifError> {
    /// let client = OnvifClient::new("http://192.168.1.1/onvif/device_service");
    /// let dt     = client.get_system_date_and_time().await?;
    /// let client = client.with_credentials("admin", "pass")
    ///                    .with_utc_offset(dt.utc_offset_secs());
    /// # Ok(()) }
    /// ```
    pub async fn get_system_date_and_time(&self) -> Result<SystemDateTime, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetSystemDateAndTime";
        const BODY: &str = "<tds:GetSystemDateAndTime/>";

        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetSystemDateAndTimeResponse")?;
        SystemDateTime::from_xml(resp)
    }

    /// Set the device clock.
    ///
    /// - `req.datetime_type` — `"Manual"` or `"NTP"`.
    /// - `req.timezone` — POSIX TZ string, e.g. `"CST-8"` for UTC+8.
    /// - `req.utc_datetime` — required when `datetime_type == "Manual"`.
    ///
    /// # Example
    /// ```no_run
    /// # use oxvif::{OnvifClient, OnvifError, SetDateTimeRequest, UtcDateTime};
    /// # async fn run() -> Result<(), OnvifError> {
    /// let client = OnvifClient::new("http://192.168.1.1/onvif/device_service");
    /// client.set_system_date_and_time(&SetDateTimeRequest {
    ///     datetime_type: "Manual".into(),
    ///     daylight_savings: false,
    ///     timezone: "CST-8".into(),
    ///     utc_datetime: Some(UtcDateTime {
    ///         year: 2026, month: 4, day: 5,
    ///         hour: 10, minute: 0, second: 0,
    ///     }),
    /// }).await?;
    /// # Ok(()) }
    /// ```
    pub async fn set_system_date_and_time(
        &self,
        req: &SetDateTimeRequest,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetSystemDateAndTime";

        let utc_el = if let Some(dt) = req.utc_datetime {
            format!(
                "<tds:UTCDateTime>\
                   <tt:Time>\
                     <tt:Hour>{h}</tt:Hour>\
                     <tt:Minute>{m}</tt:Minute>\
                     <tt:Second>{s}</tt:Second>\
                   </tt:Time>\
                   <tt:Date>\
                     <tt:Year>{y}</tt:Year>\
                     <tt:Month>{mo}</tt:Month>\
                     <tt:Day>{d}</tt:Day>\
                   </tt:Date>\
                 </tds:UTCDateTime>",
                h = dt.hour,
                m = dt.minute,
                s = dt.second,
                y = dt.year,
                mo = dt.month,
                d = dt.day,
            )
        } else {
            String::new()
        };

        let body = format!(
            "<tds:SetSystemDateAndTime>\
               <tds:DateTimeType>{dt_type}</tds:DateTimeType>\
               <tds:DaylightSavings>{dst}</tds:DaylightSavings>\
               <tds:TimeZone><tt:TZ>{tz}</tt:TZ></tds:TimeZone>\
               {utc_el}\
             </tds:SetSystemDateAndTime>",
            dt_type = xml_escape(&req.datetime_type),
            dst = req.daylight_savings,
            tz = xml_escape(&req.timezone),
        );

        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetSystemDateAndTimeResponse")?;
        Ok(())
    }

    /// Retrieve manufacturer, model, firmware version, and serial number.
    pub async fn get_device_info(&self) -> Result<DeviceInfo, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation";
        const BODY: &str = "<tds:GetDeviceInformation/>";

        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetDeviceInformationResponse")?;
        DeviceInfo::from_xml(resp)
    }

    /// Retrieve the device hostname and whether it is assigned by DHCP.
    pub async fn get_hostname(&self) -> Result<Hostname, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
        const BODY: &str = "<tds:GetHostname/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetHostnameResponse")?;
        Hostname::from_xml(resp)
    }

    /// Set the device hostname.
    ///
    /// Most devices require a reboot for the change to take effect.
    pub async fn set_hostname(&self, name: &str) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetHostname";
        let name = xml_escape(name);
        let body = format!("<tds:SetHostname><tds:Name>{name}</tds:Name></tds:SetHostname>");
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetHostnameResponse")?;
        Ok(())
    }

    /// Retrieve the NTP server configuration.
    ///
    /// Returns whether servers come from DHCP and the list of manually
    /// configured server addresses.
    pub async fn get_ntp(&self) -> Result<NtpInfo, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetNTP";
        const BODY: &str = "<tds:GetNTP/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetNTPResponse")?;
        NtpInfo::from_xml(resp)
    }

    /// Set the NTP server configuration.
    ///
    /// When `from_dhcp` is `true`, `servers` is ignored; DHCP provides the
    /// NTP servers. When `false`, each entry in `servers` is sent as a
    /// `NTPManual` element (accepted as either a DNS hostname or an IP
    /// address string).
    pub async fn set_ntp(&self, from_dhcp: bool, servers: &[&str]) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetNTP";
        let from_dhcp_str = if from_dhcp { "true" } else { "false" };
        let server_els: String = servers
            .iter()
            .map(|s| {
                format!(
                    "<tds:NTPManual>\
                       <tt:Type>DNS</tt:Type>\
                       <tt:DNSname>{}</tt:DNSname>\
                     </tds:NTPManual>",
                    xml_escape(s)
                )
            })
            .collect();
        let body = format!(
            "<tds:SetNTP>\
               <tds:FromDHCP>{from_dhcp_str}</tds:FromDHCP>\
               {server_els}\
             </tds:SetNTP>"
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetNTPResponse")?;
        Ok(())
    }

    /// Initiate a device reboot.
    ///
    /// Returns the device's informational reboot message (e.g.
    /// `"Rebooting in 30 seconds"`). The connection will drop shortly after.
    pub async fn system_reboot(&self) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SystemReboot";
        const BODY: &str = "<tds:SystemReboot/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "SystemRebootResponse")?;
        Ok(resp
            .child("Message")
            .map(|n| n.text().to_string())
            .unwrap_or_default())
    }

    /// Retrieve the device's scope URIs.
    ///
    /// Scopes describe device metadata such as name, location, and hardware model
    /// (e.g. `"onvif://www.onvif.org/name/Camera1"`). Use them for device
    /// filtering in WS-Discovery.
    pub async fn get_scopes(&self) -> Result<Vec<String>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetScopes";
        const BODY: &str = "<tds:GetScopes/>";

        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetScopesResponse")?;
        Ok(resp
            .children_named("Scopes")
            .filter_map(|n| n.child("ScopeItem").map(|s| s.text().to_string()))
            .collect())
    }

    /// Replace all configurable scopes on the device.
    ///
    /// Each entry should be a full scope URI, e.g.
    /// `"onvif://www.onvif.org/name/FrontDoor"`.
    /// Fixed (non-configurable) scopes reported by the device are not affected.
    pub async fn set_scopes(&self, scopes: &[&str]) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetScopes";
        use crate::types::xml_escape;
        let scope_els: String = scopes
            .iter()
            .map(|s| format!("<tds:Scopes>{}</tds:Scopes>", xml_escape(s)))
            .collect();
        let body = format!("<tds:SetScopes>{scope_els}</tds:SetScopes>");

        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetScopesResponse")?;
        Ok(())
    }

    /// Retrieve user accounts configured on the device.
    pub async fn get_users(&self) -> Result<Vec<User>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetUsers";
        const BODY: &str = "<tds:GetUsers/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetUsersResponse")?;
        User::vec_from_xml(resp)
    }

    /// Create one or more user accounts.
    ///
    /// Each element of `users` is `(username, password, user_level)`.
    /// `user_level` must be one of `"Administrator"`, `"Operator"`, `"User"`.
    pub async fn create_users(&self, users: &[(&str, &str, &str)]) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/CreateUsers";
        let user_els: String = users
            .iter()
            .map(|(u, p, l)| {
                format!(
                    "<tds:User>\
                       <tt:Username>{}</tt:Username>\
                       <tt:Password>{}</tt:Password>\
                       <tt:UserLevel>{}</tt:UserLevel>\
                     </tds:User>",
                    xml_escape(u),
                    xml_escape(p),
                    xml_escape(l)
                )
            })
            .collect();
        let body = format!("<tds:CreateUsers>{user_els}</tds:CreateUsers>");
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "CreateUsersResponse")?;
        Ok(())
    }

    /// Delete user accounts by username.
    pub async fn delete_users(&self, usernames: &[&str]) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/DeleteUsers";
        let user_els: String = usernames
            .iter()
            .map(|u| format!("<tds:Username>{}</tds:Username>", xml_escape(u)))
            .collect();
        let body = format!("<tds:DeleteUsers>{user_els}</tds:DeleteUsers>");
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "DeleteUsersResponse")?;
        Ok(())
    }

    /// Modify an existing user account.
    ///
    /// `password` may be `None` to leave the password unchanged.
    pub async fn set_user(
        &self,
        username: &str,
        password: Option<&str>,
        user_level: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetUser";
        let pass_el = password
            .map(|p| format!("<tt:Password>{}</tt:Password>", xml_escape(p)))
            .unwrap_or_default();
        let body = format!(
            "<tds:SetUser>\
               <tds:User>\
                 <tt:Username>{}</tt:Username>\
                 {pass_el}\
                 <tt:UserLevel>{}</tt:UserLevel>\
               </tds:User>\
             </tds:SetUser>",
            xml_escape(username),
            xml_escape(user_level)
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetUserResponse")?;
        Ok(())
    }

    /// Retrieve all network interfaces and their IPv4/IPv6 configuration.
    pub async fn get_network_interfaces(&self) -> Result<Vec<NetworkInterface>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetNetworkInterfaces";
        const BODY: &str = "<tds:GetNetworkInterfaces/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetNetworkInterfacesResponse")?;
        NetworkInterface::vec_from_xml(resp)
    }

    /// Update the IPv4 configuration of a network interface.
    ///
    /// Returns `true` if the device requires a reboot to apply the change.
    pub async fn set_network_interfaces(
        &self,
        token: &str,
        enabled: bool,
        ipv4_address: &str,
        prefix_length: u32,
        from_dhcp: bool,
    ) -> Result<bool, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetNetworkInterfaces";
        let enabled_str = if enabled { "true" } else { "false" };
        let from_dhcp_str = if from_dhcp { "true" } else { "false" };
        let body = format!(
            "<tds:SetNetworkInterfaces>\
               <tds:InterfaceToken>{}</tds:InterfaceToken>\
               <tds:NetworkInterface>\
                 <tt:Enabled>{enabled_str}</tt:Enabled>\
                 <tt:IPv4>\
                   <tt:Enabled>true</tt:Enabled>\
                   <tt:DHCP>{from_dhcp_str}</tt:DHCP>\
                   <tt:Manual>\
                     <tt:Address>{}</tt:Address>\
                     <tt:PrefixLength>{prefix_length}</tt:PrefixLength>\
                   </tt:Manual>\
                 </tt:IPv4>\
               </tds:NetworkInterface>\
             </tds:SetNetworkInterfaces>",
            xml_escape(token),
            xml_escape(ipv4_address)
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "SetNetworkInterfacesResponse")?;
        let reboot = resp
            .child("RebootNeeded")
            .map(|n| n.text() == "true" || n.text() == "1")
            .unwrap_or(false);
        Ok(reboot)
    }

    /// Retrieve the enabled network protocols (HTTP, HTTPS, RTSP, etc.).
    pub async fn get_network_protocols(&self) -> Result<Vec<NetworkProtocol>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetNetworkProtocols";
        const BODY: &str = "<tds:GetNetworkProtocols/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetNetworkProtocolsResponse")?;
        NetworkProtocol::vec_from_xml(resp)
    }

    /// Retrieve the DNS server configuration.
    pub async fn get_dns(&self) -> Result<DnsInformation, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetDNS";
        const BODY: &str = "<tds:GetDNS/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetDNSResponse")?;
        DnsInformation::from_xml(resp)
    }

    /// Set the DNS server configuration.
    ///
    /// When `from_dhcp` is `true`, `servers` is ignored.
    pub async fn set_dns(&self, from_dhcp: bool, servers: &[&str]) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetDNS";
        let from_dhcp_str = if from_dhcp { "true" } else { "false" };
        let server_els: String = servers
            .iter()
            .map(|s| {
                format!(
                    "<tds:DNSManual>\
                       <tt:Type>IPv4</tt:Type>\
                       <tt:IPv4Address>{}</tt:IPv4Address>\
                     </tds:DNSManual>",
                    xml_escape(s)
                )
            })
            .collect();
        let body = format!(
            "<tds:SetDNS>\
               <tds:FromDHCP>{from_dhcp_str}</tds:FromDHCP>\
               {server_els}\
             </tds:SetDNS>"
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetDNSResponse")?;
        Ok(())
    }

    /// Retrieve the default IPv4 and IPv6 gateway addresses.
    pub async fn get_network_default_gateway(&self) -> Result<NetworkGateway, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetNetworkDefaultGateway";
        const BODY: &str = "<tds:GetNetworkDefaultGateway/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetNetworkDefaultGatewayResponse")?;
        NetworkGateway::from_xml(resp)
    }

    /// Retrieve the device system log.
    ///
    /// `log_type` is typically `"System"` or `"Access"`.
    pub async fn get_system_log(&self, log_type: &str) -> Result<SystemLog, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetSystemLog";
        let body = format!(
            "<tds:GetSystemLog><tds:LogType>{}</tds:LogType></tds:GetSystemLog>",
            xml_escape(log_type)
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetSystemLogResponse")?;
        SystemLog::from_xml(resp)
    }

    /// Retrieve all relay output port configurations.
    pub async fn get_relay_outputs(&self) -> Result<Vec<RelayOutput>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetRelayOutputs";
        const BODY: &str = "<tds:GetRelayOutputs/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetRelayOutputsResponse")?;
        RelayOutput::vec_from_xml(resp)
    }

    /// Configure the properties of a relay output port.
    ///
    /// - `mode`: `"Bistable"` (latching) or `"Monostable"` (timed).
    /// - `delay_time`: ISO 8601 duration for monostable mode, e.g. `"PT1S"`.
    /// - `idle_state`: `"closed"` or `"open"`.
    pub async fn set_relay_output_settings(
        &self,
        relay_token: &str,
        mode: &str,
        delay_time: &str,
        idle_state: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputSettings";
        let body = format!(
            "<tds:SetRelayOutputSettings>\
               <tds:RelayOutputToken>{}</tds:RelayOutputToken>\
               <tds:Properties>\
                 <tt:Mode>{}</tt:Mode>\
                 <tt:DelayTime>{}</tt:DelayTime>\
                 <tt:IdleState>{}</tt:IdleState>\
               </tds:Properties>\
             </tds:SetRelayOutputSettings>",
            xml_escape(relay_token),
            xml_escape(mode),
            xml_escape(delay_time),
            xml_escape(idle_state)
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetRelayOutputSettingsResponse")?;
        Ok(())
    }

    /// Set the electrical state of a relay output port.
    ///
    /// `state` must be `"active"` or `"inactive"`.
    pub async fn set_relay_output_state(
        &self,
        relay_token: &str,
        state: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetRelayOutputState";
        let body = format!(
            "<tds:SetRelayOutputState>\
               <tds:RelayOutputToken>{}</tds:RelayOutputToken>\
               <tds:LogicalState>{}</tds:LogicalState>\
             </tds:SetRelayOutputState>",
            xml_escape(relay_token),
            xml_escape(state)
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetRelayOutputStateResponse")?;
        Ok(())
    }

    /// Enable or disable network protocols (HTTP, HTTPS, RTSP, etc.).
    ///
    /// Each element of `protocols` is `(name, enabled, ports)`.
    /// `name` is typically `"HTTP"`, `"HTTPS"`, or `"RTSP"`.
    pub async fn set_network_protocols(
        &self,
        protocols: &[(&str, bool, &[u32])],
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetNetworkProtocols";
        let proto_els: String = protocols
            .iter()
            .map(|(name, enabled, ports)| {
                let port_els: String = ports
                    .iter()
                    .map(|p| format!("<tt:Port>{p}</tt:Port>"))
                    .collect();
                format!(
                    "<tds:NetworkProtocols>\
                       <tt:Name>{}</tt:Name>\
                       <tt:Enabled>{enabled}</tt:Enabled>\
                       {port_els}\
                     </tds:NetworkProtocols>",
                    xml_escape(name)
                )
            })
            .collect();
        let body = format!("<tds:SetNetworkProtocols>{proto_els}</tds:SetNetworkProtocols>");
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetNetworkProtocolsResponse")?;
        Ok(())
    }

    /// Restore the device to factory defaults.
    ///
    /// `default_type` must be `"Hard"` (full reset, including network settings)
    /// or `"Soft"` (reset configuration but keep network settings).
    pub async fn set_system_factory_default(&self, default_type: &str) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetSystemFactoryDefault";
        let body = format!(
            "<tds:SetSystemFactoryDefault>\
               <tds:FactoryDefault>{}</tds:FactoryDefault>\
             </tds:SetSystemFactoryDefault>",
            xml_escape(default_type)
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetSystemFactoryDefaultResponse")?;
        Ok(())
    }

    /// Retrieve all storage locations (SD card, NAS, etc.) configured on the device.
    pub async fn get_storage_configurations(
        &self,
    ) -> Result<Vec<StorageConfiguration>, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetStorageConfigurations";
        const BODY: &str = "<tds:GetStorageConfigurations/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetStorageConfigurationsResponse")?;
        StorageConfiguration::vec_from_xml(resp)
    }

    /// Create or update a storage configuration entry.
    ///
    /// Pass `token = ""` to create a new entry; supply an existing token to
    /// update it.  `storage_type` is the `type` attribute of the `Data` element
    /// (e.g. `"LocalStorage"` or `"NFS"`).
    pub async fn set_storage_configuration(
        &self,
        token: &str,
        storage_type: &str,
        local_path: &str,
        storage_uri: &str,
        user: &str,
    ) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetStorageConfiguration";
        let token_attr = if token.is_empty() {
            String::new()
        } else {
            format!(" token=\"{}\"", xml_escape(token))
        };
        let user_el = if user.is_empty() {
            String::new()
        } else {
            format!(
                "<tt:User><tt:UserName>{}</tt:UserName></tt:User>",
                xml_escape(user)
            )
        };
        let local_path_el = if local_path.is_empty() {
            String::new()
        } else {
            format!("<tt:LocalPath>{}</tt:LocalPath>", xml_escape(local_path))
        };
        let storage_uri_el = if storage_uri.is_empty() {
            String::new()
        } else {
            format!("<tt:StorageUri>{}</tt:StorageUri>", xml_escape(storage_uri))
        };
        let body = format!(
            "<tds:SetStorageConfiguration>\
               <tds:StorageConfiguration{token_attr}>\
                 <tt:Data type=\"{storage_type_escaped}\">\
                   {local_path_el}{storage_uri_el}{user_el}\
                 </tt:Data>\
               </tds:StorageConfiguration>\
             </tds:SetStorageConfiguration>",
            storage_type_escaped = xml_escape(storage_type),
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetStorageConfigurationResponse")?;
        Ok(())
    }

    /// Retrieve HTTP URIs for firmware upgrade, system log, and support-info download.
    pub async fn get_system_uris(&self) -> Result<SystemUris, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetSystemUris";
        const BODY: &str = "<tds:GetSystemUris/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetSystemUrisResponse")?;
        SystemUris::from_xml(resp)
    }

    /// Retrieve the current WS-Discovery mode.
    ///
    /// Returns `"Discoverable"` or `"NonDiscoverable"`.
    pub async fn get_discovery_mode(&self) -> Result<String, OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/GetDiscoveryMode";
        const BODY: &str = "<tds:GetDiscoveryMode/>";
        let xml = self.call(&self.device_url, ACTION, BODY).await?;
        let body_node = parse_soap_body(&xml)?;
        let resp = find_response(&body_node, "GetDiscoveryModeResponse")?;
        Ok(resp
            .child("DiscoveryMode")
            .map(|n| n.text().to_string())
            .unwrap_or_default())
    }

    /// Set the WS-Discovery mode.
    ///
    /// `mode` must be `"Discoverable"` or `"NonDiscoverable"`.
    pub async fn set_discovery_mode(&self, mode: &str) -> Result<(), OnvifError> {
        const ACTION: &str = "http://www.onvif.org/ver10/device/wsdl/SetDiscoveryMode";
        let body = format!(
            "<tds:SetDiscoveryMode>\
               <tds:DiscoveryMode>{}</tds:DiscoveryMode>\
             </tds:SetDiscoveryMode>",
            xml_escape(mode)
        );
        let xml = self.call(&self.device_url, ACTION, &body).await?;
        let body_node = parse_soap_body(&xml)?;
        find_response(&body_node, "SetDiscoveryModeResponse")?;
        Ok(())
    }
}
