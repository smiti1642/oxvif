use crate::helpers::soap;

pub fn resp_system_date_and_time() -> String {
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

pub fn resp_capabilities(base: &str) -> String {
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

pub fn resp_services(base: &str) -> String {
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

pub fn resp_device_info() -> String {
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

pub fn resp_hostname() -> String {
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

pub fn resp_ntp() -> String {
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

pub fn resp_scopes() -> String {
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

pub fn resp_users() -> String {
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

pub fn resp_network_interfaces() -> String {
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

pub fn resp_set_network_interfaces() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:SetNetworkInterfacesResponse>
          <tds:RebootNeeded>false</tds:RebootNeeded>
        </tds:SetNetworkInterfacesResponse>"#,
    )
}

pub fn resp_network_protocols() -> String {
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

pub fn resp_dns() -> String {
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

pub fn resp_network_default_gateway() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetNetworkDefaultGatewayResponse>
          <tds:NetworkGateway>
            <tt:IPv4Address>192.168.1.1</tt:IPv4Address>
          </tds:NetworkGateway>
        </tds:GetNetworkDefaultGatewayResponse>"#,
    )
}

pub fn resp_system_log() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetSystemLogResponse>
          <tds:SystemLog>
            <tt:String>2026-04-03 12:00:00 mock system started</tt:String>
          </tds:SystemLog>
        </tds:GetSystemLogResponse>"#,
    )
}

pub fn resp_relay_outputs() -> String {
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

pub fn resp_send_auxiliary_command() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:SendAuxiliaryCommandResponse>
          <tds:AuxiliaryCommandResponse>OK</tds:AuxiliaryCommandResponse>
        </tds:SendAuxiliaryCommandResponse>"#,
    )
}

pub fn resp_storage_configurations() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
             xmlns:tt="http://www.onvif.org/ver10/schema""#,
        r#"<tds:GetStorageConfigurationsResponse>
          <tds:StorageConfigurations token="SD_01">
            <tt:Data type="LocalStorage">
              <tt:LocalPath>/mnt/sd</tt:LocalPath>
            </tt:Data>
          </tds:StorageConfigurations>
        </tds:GetStorageConfigurationsResponse>"#,
    )
}

pub fn resp_system_uris(base: &str) -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
             xmlns:tt="http://www.onvif.org/ver10/schema""#,
        &format!(
            r#"<tds:GetSystemUrisResponse>
          <tds:SystemLogUris>
            <tt:SystemLogUri>
              <tt:Uri>{base}/syslog</tt:Uri>
              <tt:LogType>System</tt:LogType>
            </tt:SystemLogUri>
          </tds:SystemLogUris>
          <tds:SupportInfoUri>{base}/support</tds:SupportInfoUri>
        </tds:GetSystemUrisResponse>"#
        ),
    )
}

pub fn resp_discovery_mode() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:GetDiscoveryModeResponse>
          <tds:DiscoveryMode>Discoverable</tds:DiscoveryMode>
        </tds:GetDiscoveryModeResponse>"#,
    )
}

pub fn resp_system_reboot() -> String {
    soap(
        r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl""#,
        r#"<tds:SystemRebootResponse>
          <tds:Message>Rebooting in 30 seconds</tds:Message>
        </tds:SystemRebootResponse>"#,
    )
}
