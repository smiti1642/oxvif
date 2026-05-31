# Device Management Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/device/wsdl/devicemgmt.wsdl
- **Namespace:** `http://www.onvif.org/ver10/device/wsdl` (prefix `tds`)
- **ONVIF Profile:** S / T / G (core device management)
- **oxvif status:** ◐ implemented in `src/client/device.rs` (~35 of ~104 operations)

The device service is the entry point — `GetCapabilities` / `GetServices` resolve every other
service URL. oxvif covers the Profile-S/T/G core; the long tail (certificates, IEEE 802.1X,
802.11 wireless, password policy) is unimplemented.

---

## Operations

### System
| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetCapabilities | service URLs + feature flags | ✓ | `get_capabilities` |
| GetServices | service URLs (incl. Media2) | ✓ | `get_services` |
| GetServiceCapabilities | device service capabilities | — | — |
| GetDeviceInformation | manufacturer/model/firmware/serial | ✓ | `get_device_info` |
| GetSystemDateAndTime | device clock | ✓ | `get_system_date_and_time` |
| SetSystemDateAndTime | set device clock | ✓ | `set_system_date_and_time` |
| SetSystemFactoryDefault | factory reset (Hard/Soft) | ✓ | `set_system_factory_default` |
| SystemReboot | reboot | ✓ | `system_reboot` |
| GetSystemLog | system/access log | ✓ | `get_system_log` |
| GetSystemUris | syslog/support/backup URIs | ✓ | `get_system_uris` |
| GetSystemSupportInformation | support-info blob | — | — |
| GetSystemBackup | download config backup | — | — |
| RestoreSystem | restore config backup | — | — |
| StartSystemRestore | restore via upload URI | ✓ | `start_system_restore` |
| UpgradeSystemFirmware | firmware upgrade (deprecated) | — | — |
| StartFirmwareUpgrade | firmware upgrade via upload URI | ✓ | `start_firmware_upgrade` |
| UpgradeFirmware | firmware upgrade by version | — | — |
| SetHashingAlgorithm | password hashing algorithm | — | — |
| GetEndpointReference | device GUID / endpoint ref | — | — |
| GetWsdlUrl | device WSDL location | — | — |

### Scopes & discovery
| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetScopes | list scope URIs | ✓ | `get_scopes` |
| SetScopes | replace configurable scopes | ✓ | `set_scopes` |
| AddScopes | append scopes | — | — |
| RemoveScopes | remove scopes | — | — |
| GetDiscoveryMode | WS-Discovery mode | ✓ | `get_discovery_mode` |
| SetDiscoveryMode | set WS-Discovery mode | ✓ | `set_discovery_mode` |
| GetRemoteDiscoveryMode | remote discovery mode | — | — |
| SetRemoteDiscoveryMode | set remote discovery mode | — | — |
| GetDPAddresses | discovery-proxy addresses | — | — |
| SetDPAddresses | set discovery-proxy addresses | — | — |

### Users & access
| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetUsers | list accounts | ✓ | `get_users` |
| CreateUsers | create accounts | ✓ | `create_users` |
| DeleteUsers | delete accounts | ✓ | `delete_users` |
| SetUser | modify account | ✓ | `set_user` |
| GetUserRoles | list user roles | — | — |
| SetUserRole | create/modify a role | — | — |
| DeleteUserRole | delete a role | — | — |
| GetRemoteUser | remote-user (RTSP) creds | — | — |
| SetRemoteUser | set remote-user creds | — | — |
| GetAccessPolicy | access-policy file | — | — |
| SetAccessPolicy | set access-policy file | — | — |
| Get/SetPasswordComplexityConfiguration, GetPasswordComplexityOptions | password complexity policy | — | — |
| Get/SetPasswordHistoryConfiguration | password history policy | — | — |
| Get/SetAuthFailureWarningConfiguration, GetAuthFailureWarningOptions | auth-failure lockout policy | — | — |

### Network
| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetHostname | hostname + DHCP flag | ✓ | `get_hostname` |
| SetHostname | static hostname | ✓ | `set_hostname` |
| SetHostnameFromDHCP | hostname from DHCP | — | — |
| GetDNS / SetDNS | DNS servers | ✓ | `get_dns` / `set_dns` |
| GetNTP / SetNTP | NTP servers | ✓ | `get_ntp` / `set_ntp` |
| GetDynamicDNS / SetDynamicDNS | DDNS config | — | — |
| GetNetworkInterfaces / SetNetworkInterfaces | iface IP/MAC/MTU | ✓ | `get_network_interfaces` / `set_network_interfaces` |
| GetNetworkProtocols / SetNetworkProtocols | HTTP/HTTPS/RTSP ports | ✓ | `get_network_protocols` / `set_network_protocols` |
| GetNetworkDefaultGateway / SetNetworkDefaultGateway | default gateway | ✓ | `get_network_default_gateway` / `set_network_default_gateway` |
| GetZeroConfiguration / SetZeroConfiguration | zeroconf (link-local) | — | — |
| GetIPAddressFilter / SetIPAddressFilter | IP allow/deny filter | — | — |
| AddIPAddressFilter / RemoveIPAddressFilter | edit IP filter | — | — |

### I/O & storage
| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetRelayOutputs | list relay outputs | ✓ | `get_relay_outputs` |
| SetRelayOutputState | set relay electrical state | ✓ | `set_relay_output_state` |
| SetRelayOutputSettings | configure relay mode/delay | ✓ | `set_relay_output_settings` |
| SendAuxiliaryCommand | aux command (wiper/IR lamp) | ✓ | `send_auxiliary_command` |
| GetStorageConfigurations | list storage configs | ✓ | `get_storage_configurations` |
| SetStorageConfiguration | update a storage config | ✓ | `set_storage_configuration` |
| GetStorageConfiguration | single storage config | — | — |
| CreateStorageConfiguration | create storage config | — | — |
| DeleteStorageConfiguration | delete storage config | — | — |

### Geolocation
| Operation | Purpose | oxvif | method |
|-----------|---------|:----:|--------|
| GetGeoLocation | device geolocation | — | — |
| SetGeoLocation | set geolocation | — | — |
| DeleteGeoLocation | clear geolocation | — | — |

### Security: certificates / TLS / 802.1X / 802.11
All unimplemented. Catalog only — field detail via the WSDL `<wsdl:types>` when implementing.

`CreateCertificate`, `GetCertificates`, `GetCertificatesStatus`, `SetCertificatesStatus`,
`DeleteCertificates`, `GetPkcs10Request`, `LoadCertificates`, `GetClientCertificateMode`,
`SetClientCertificateMode`, `GetCACertificates`, `LoadCertificateWithPrivateKey`,
`GetCertificateInformation`, `LoadCACertificates`,
`CreateDot1XConfiguration`, `SetDot1XConfiguration`, `GetDot1XConfiguration`,
`GetDot1XConfigurations`, `DeleteDot1XConfiguration`,
`GetDot11Capabilities`, `GetDot11Status`, `ScanAvailableDot11Networks`.

---

## Request / response detail (priority unimplemented)

#### GetServiceCapabilities
- **Req:** _(empty)_ · **Resp:** `Capabilities` `tds:DeviceServiceCapabilities` [1]
  (Network / Security / System capability sub-trees).

#### AddScopes
- **Req:** `ScopeItem` `xs:anyURI` [1..*] · **Resp:** _(empty)_

#### RemoveScopes
- **Req:** `ScopeItem` `xs:anyURI` [1..*] · **Resp:** `ScopeItem` `xs:anyURI` [0..*]

#### GetSystemSupportInformation
- **Req:** _(empty)_ · **Resp:** `SupportInformation` `tt:SupportInformation` [1]

#### GetSystemBackup
- **Req:** _(empty)_ · **Resp:** `BackupFiles` `tt:BackupFile` [1..*]

#### RestoreSystem
- **Req:** `BackupFiles` `tt:BackupFile` [1..*] · **Resp:** _(empty)_

#### GetDynamicDNS
- **Req:** _(empty)_ · **Resp:** `DynamicDNSInformation` `tt:DynamicDNSInformation` [1]

#### SetDynamicDNS
- **Req:** `Type` `tt:DynamicDNSType` [1] (`NoUpdate|ClientUpdates|ServerUpdates`);
  `Name` `tt:DNSName` [0..1]; `TTL` `xs:duration` [0..1] · **Resp:** _(empty)_

#### GetZeroConfiguration
- **Req:** _(empty)_ · **Resp:** `ZeroConfiguration` `tt:NetworkZeroConfiguration` [1]

#### SetZeroConfiguration
- **Req:** `InterfaceToken` `tt:ReferenceToken` [1]; `Enabled` `xs:boolean` [1] · **Resp:** _(empty)_

#### GetIPAddressFilter
- **Req:** _(empty)_ · **Resp:** `IPAddressFilter` `tt:IPAddressFilter` [1]

#### SetIPAddressFilter / AddIPAddressFilter / RemoveIPAddressFilter
- **Req:** `IPAddressFilter` `tt:IPAddressFilter` [1] · **Resp:** _(empty)_

#### GetEndpointReference
- **Req:** _(empty)_ · **Resp:** `GUID` `xs:string` [1] (+ extensions)

#### GetWsdlUrl
- **Req:** _(empty)_ · **Resp:** `WsdlUrl` `xs:anyURI` [1]

#### GetDPAddresses
- **Req:** _(empty)_ · **Resp:** `DPAddress` `tt:NetworkHost` [0..*]

#### GetUserRoles
- **Req:** `UserRole` `xs:string` [0..1] · **Resp:** `UserRole` `tt:UserRole` [0..*]

#### SetUserRole
- **Req:** `UserRole` `tt:UserRole` [1] · **Resp:** _(empty)_

#### DeleteUserRole
- **Req:** `UserRole` `xs:string` [1] · **Resp:** _(empty)_

#### GetGeoLocation
- **Req:** _(empty)_ · **Resp:** `Location` `tt:LocationEntity` [0..*]

#### SetGeoLocation
- **Req:** `Location` `tt:LocationEntity` [1..*] · **Resp:** _(empty)_

#### DeleteGeoLocation
- **Req:** `Location` `tt:LocationEntity` [1..*] · **Resp:** _(empty)_

#### GetStorageConfiguration
- **Req:** `Token` `tt:ReferenceToken` [1] · **Resp:** `StorageConfiguration` `tds:StorageConfiguration` [1]

#### CreateStorageConfiguration
- **Req:** `StorageConfiguration` `tds:StorageConfigurationData` [1] · **Resp:** `Token` `tt:ReferenceToken` [1]

#### DeleteStorageConfiguration
- **Req:** `Token` `tt:ReferenceToken` [1] · **Resp:** _(empty)_

#### UpgradeFirmware
- **Req:** `Version` `xs:string` [1] · **Resp:** `ExpectedDownTime` `xs:duration` [1]

#### GetRemoteUser / SetRemoteUser
- **Req/Resp:** `RemoteUser` `tt:RemoteUser` [0..1]

#### GetAccessPolicy / SetAccessPolicy
- **Get Resp / Set Req:** `PolicyFile` `tt:BinaryData` [1]

Complex types (`tt:IPAddressFilter`, `tt:DynamicDNSInformation`, `tt:NetworkZeroConfiguration`,
`tt:LocationEntity`, `tt:UserRole`, `tt:RemoteUser`, `tds:StorageConfiguration`, `tt:BackupFile`,
`tt:SupportInformation`, `tt:NetworkHost`, `tt:BinaryData`): see devicemgmt.wsdl / onvif.xsd.

_Source: devicemgmt.wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
