# DeviceIO Service

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).

- **WSDL:** https://www.onvif.org/ver10/deviceio.wsdl
- **Namespace:** `http://www.onvif.org/ver10/deviceio/wsdl` (prefix `tmd`)
- **ONVIF Profile:** T
- **oxvif status:** ❌ not implemented (ROADMAP short-term). No `src/client/deviceio.rs`.

DeviceIO exposes the physical I/O of a device: video/audio sources & outputs, relay outputs,
digital inputs, and serial ports. Several operations overlap the Device service (relay outputs)
or Media (video/audio source configs) but are addressed at the DeviceIO endpoint.

---

## Operations & detail

Cardinality: `[1]`/`[0..1]`/`[0..*]`/`[1..*]`; all listed members are elements unless noted.

| Operation | Purpose | Req → Resp |
|-----------|---------|------------|
| GetServiceCapabilities | service capabilities | _empty_ → `Capabilities` `tmd:Capabilities` [1] |
| GetVideoSources | list video inputs | _empty_ → `Token` `tt:ReferenceToken` [0..*] |
| GetVideoOutputs | list video outputs | _empty_ → `VideoOutputs` `tt:VideoOutput` [0..*] |
| GetAudioSources | list audio inputs | _empty_ → `Token` `tt:ReferenceToken` [0..*] |
| GetAudioOutputs | list audio outputs | _empty_ → `Token` `tt:ReferenceToken` [0..*] |
| GetVideoSourceConfiguration | read video-source config | `VideoSourceToken` [1] → `VideoSourceConfiguration` `tt:VideoSourceConfiguration` [1] |
| SetVideoSourceConfiguration | write video-source config | `Configuration` `tt:VideoSourceConfiguration` [1], `ForcePersistence` `xs:boolean` [1] → _empty_ |
| GetVideoSourceConfigurationOptions | options | `VideoSourceToken` [1] → `VideoSourceConfigurationOptions` `tt:VideoSourceConfigurationOptions` [1] |
| GetVideoOutputConfiguration | read video-output config | `VideoOutputToken` [1] → `VideoOutputConfiguration` `tt:VideoOutputConfiguration` [1] |
| SetVideoOutputConfiguration | write video-output config | `Configuration` `tt:VideoOutputConfiguration` [1], `ForcePersistence` `xs:boolean` [1] → _empty_ |
| GetVideoOutputConfigurationOptions | options | `VideoOutputToken` [1] → `…Options` [1] |
| GetAudioSourceConfiguration / SetAudioSourceConfiguration / …Options | audio-source config | mirror video-source ops with `tt:AudioSourceConfiguration` |
| GetAudioOutputConfiguration / SetAudioOutputConfiguration / …Options | audio-output config | mirror with `tt:AudioOutputConfiguration` |
| GetRelayOutputs | list relay outputs | _empty_ → `RelayOutputs` `tt:RelayOutput` [0..*] (shares `tds` shape) |
| GetRelayOutputOptions | relay output options | `RelayOutputToken` `tt:ReferenceToken` [0..1] → `RelayOutputOptions` `tmd:RelayOutputOptions` [0..*] |
| SetRelayOutputSettings | configure relay | `RelayOutput` `tt:RelayOutput` [1] → _empty_ |
| SetRelayOutputState | set relay state | `RelayOutputToken` [1], `LogicalState` `tt:RelayLogicalState` [1] → _empty_ |
| GetDigitalInputs | list digital inputs | _empty_ → `DigitalInputs` `tt:DigitalInput` [0..*] |
| GetDigitalInputConfigurationOptions | DI options | `Token` `tt:ReferenceToken` [0..1] → `DigitalInputOptions` `tmd:DigitalInputConfigurationOptions` [1] |
| SetDigitalInputConfigurations | configure DIs | `DigitalInputs` `tt:DigitalInput` [1..*] → _empty_ |
| GetSerialPorts | list serial ports | _empty_ → `SerialPort` `tmd:SerialPort` [0..*] |
| GetSerialPortConfiguration | read serial config | `SerialPortToken` [1] → `SerialPortConfiguration` `tmd:SerialPortConfiguration` [1] |
| SetSerialPortConfiguration | write serial config | `SerialPortConfiguration` `tmd:SerialPortConfiguration` [1], `ForcePersistance` `xs:boolean` [1] → _empty_ |
| GetSerialPortConfigurationOptions | serial options | `SerialPortToken` [1] → `SerialPortOptions` `tmd:SerialPortConfigurationOptions` [1] |
| SendReceiveSerialCommand | serial transceive | `Token` [0..1], `SerialData` `tmd:SerialData` [0..1], `TimeOut` `xs:duration` [0..1], `DataLength` `xs:integer` [0..1], `Delimiter` `xs:string` [0..1] → `SerialData` `tmd:SerialData` [0..1] |

> Note `SetSerialPortConfiguration` uses the misspelled `ForcePersistance` attribute as defined in the WSDL.

Complex types (`tmd:Capabilities`, `tmd:RelayOutputOptions`, `tmd:DigitalInputConfigurationOptions`,
`tmd:SerialPort`, `tmd:SerialPortConfiguration`, `tmd:SerialData`, `tt:VideoOutput`,
`tt:DigitalInput`): see deviceio.wsdl `<wsdl:types>` / onvif.xsd.

_Source: deviceio.wsdl operation list + `<wsdl:types>` (fetched 2026-05)._
