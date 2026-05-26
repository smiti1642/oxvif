# Audio (Media1 / Media2)

> Reference for implementing oxvif — not part of the crate. Shared types: [types.md](types.md).
> Audio operations are not a separate ONVIF service — they live in the Media1 (`trt`) and
> Media2 (`tr2`) WSDLs. This file is the README-style audio view across both.

- **WSDL:** https://www.onvif.org/ver10/media/wsdl/media.wsdl · https://www.onvif.org/ver20/media/wsdl/media.wsdl
- **ONVIF Profile:** S (input) / T (output, backchannel)
- **oxvif status:** ◐ input config covered; outputs/decoders/backchannel mostly unimplemented.

---

## Operations & oxvif coverage

### Sources / inputs (Media1)
| Operation | oxvif | method |
|-----------|:----:|--------|
| GetAudioSources | ✓ | `get_audio_sources` |
| GetAudioSourceConfigurations | ✓ | `get_audio_source_configurations` |
| GetAudioSourceConfiguration | — | — |
| SetAudioSourceConfiguration | — | — |
| GetAudioSourceConfigurationOptions | — | — |
| Add/RemoveAudioSourceConfiguration | — | — |

### Encoders (Media1)
| Operation | oxvif | method |
|-----------|:----:|--------|
| GetAudioEncoderConfigurations | ✓ | `get_audio_encoder_configurations` |
| GetAudioEncoderConfiguration | ✓ | `get_audio_encoder_configuration` |
| SetAudioEncoderConfiguration | ✓ | `set_audio_encoder_configuration` |
| GetAudioEncoderConfigurationOptions | ✓ | `get_audio_encoder_configuration_options` |
| Add/RemoveAudioEncoderConfiguration | — | — |

### Media2 audio
| Operation | oxvif | method |
|-----------|:----:|--------|
| GetAudioSourceConfigurations | ✓ | `get_audio_source_configurations_media2` |
| GetAudioEncoderConfigurations | ✓ | `get_audio_encoder_configurations_media2` |
| GetAudioEncoderConfigurationOptions | ✓ | `get_audio_encoder_configuration_options_media2` |
| SetAudioEncoderConfiguration | ✓ | `set_audio_encoder_configuration_media2` |
| GetAudioOutputConfigurations | ✓ | `get_audio_output_configurations_media2` |
| GetAudioDecoderConfigurations | ✓ | `get_audio_decoder_configurations_media2` |
| SetAudioSource/Output/DecoderConfiguration | — | — |
| GetAudio*ConfigurationOptions (source/output/decoder) | — | — |

### Outputs / backchannel (Media1 + Media2) — unimplemented
`GetAudioOutputs` (Media1), `Get/SetAudioOutputConfiguration(s)`, `GetAudioOutputConfigurationOptions`,
`Get/SetAudioDecoderConfiguration`, `GetAudioDecoderConfigurationOptions`,
plus Media2 audio clips / multicast decoder (see [media2.md](media2.md)).

---

## Field shapes

The audio config ops follow the **same family patterns** documented in [media1.md](media1.md)
(`Get/Set/Options/Add/Remove<Kind>Configuration`) and the Media2 shape in [media2.md](media2.md).
Concrete config types: `tt:AudioSourceConfiguration`, `tt:AudioEncoderConfiguration`
(`Encoding` ∈ G711/G726/AAC, `Bitrate`, `SampleRate`), `tt:AudioOutputConfiguration`,
`tt:AudioDecoderConfiguration` — see onvif.xsd.

_Source: media.wsdl + media2 wsdl (fetched 2026-05)._
