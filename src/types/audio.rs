use super::{xml_escape, xml_str, xml_u32};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── AudioSource ───────────────────────────────────────────────────────────────

/// A physical audio input returned by `GetAudioSources`.
#[derive(Debug, Clone)]
pub struct AudioSource {
    /// Opaque token; pass to `AudioSourceConfiguration.source_token`.
    pub token: String,
    /// Number of audio channels this source provides.
    pub channels: u32,
}

impl AudioSource {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("AudioSources")
            .map(|n| {
                let token = n
                    .attr("token")
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("AudioSources/@token"))?
                    .to_string();
                Ok(Self {
                    token,
                    channels: xml_u32(n, "Channels").unwrap_or(1),
                })
            })
            .collect()
    }
}

// ── AudioSourceConfiguration ──────────────────────────────────────────────────

/// Audio source configuration returned by `GetAudioSourceConfigurations`.
#[derive(Debug, Clone)]
pub struct AudioSourceConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    pub name: String,
    /// Number of profiles referencing this configuration.
    pub use_count: u32,
    /// Token of the physical `AudioSource` this config reads from.
    pub source_token: String,
}

impl AudioSourceConfiguration {
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(|n| {
                let token = n
                    .attr("token")
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("Configurations/@token"))?
                    .to_string();
                Ok(Self {
                    token,
                    name: xml_str(n, "Name").unwrap_or_default(),
                    use_count: xml_u32(n, "UseCount").unwrap_or(0),
                    source_token: xml_str(n, "SourceToken").unwrap_or_default(),
                })
            })
            .collect()
    }
}

// ── AudioEncoding ─────────────────────────────────────────────────────────────

/// Audio compression format.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AudioEncoding {
    #[default]
    G711,
    G726,
    Aac,
    Other(String),
}

impl AudioEncoding {
    pub(crate) fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "G711" => Self::G711,
            "G726" => Self::G726,
            "AAC" => Self::Aac,
            _ => Self::Other(s.to_string()),
        }
    }

    /// Returns the ONVIF wire string for this encoding (e.g. `"G711"`).
    pub fn as_str(&self) -> &str {
        match self {
            Self::G711 => "G711",
            Self::G726 => "G726",
            Self::Aac => "AAC",
            Self::Other(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for AudioEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── AudioEncoderConfiguration ─────────────────────────────────────────────────

/// Audio codec settings returned by `GetAudioEncoderConfiguration(s)`.
///
/// Pass a modified copy to `set_audio_encoder_configuration`.
#[derive(Debug, Clone)]
pub struct AudioEncoderConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    pub name: String,
    /// Number of profiles referencing this configuration.
    pub use_count: u32,
    pub encoding: AudioEncoding,
    /// Bitrate in kbps (e.g. 64).
    pub bitrate: u32,
    /// Sample rate in kHz (e.g. 8).
    pub sample_rate: u32,
}

impl AudioEncoderConfiguration {
    pub(crate) fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        let token = node
            .attr("token")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| SoapError::missing("Configurations/@token"))?
            .to_string();
        Ok(Self {
            token,
            name: xml_str(node, "Name").unwrap_or_default(),
            use_count: xml_u32(node, "UseCount").unwrap_or(0),
            encoding: xml_str(node, "Encoding")
                .map(|s| AudioEncoding::from_str(&s))
                .unwrap_or_default(),
            bitrate: xml_u32(node, "Bitrate").unwrap_or(0),
            sample_rate: xml_u32(node, "SampleRate").unwrap_or(0),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }

    /// Serialise to a `<trt:Configuration>` XML fragment for
    /// `SetAudioEncoderConfiguration`.
    pub(crate) fn to_xml_body(&self) -> String {
        format!(
            "<trt:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Encoding>{encoding}</tt:Encoding>\
               <tt:Bitrate>{bitrate}</tt:Bitrate>\
               <tt:SampleRate>{sample_rate}</tt:SampleRate>\
             </trt:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            encoding = self.encoding,
            bitrate = self.bitrate,
            sample_rate = self.sample_rate,
        )
    }
}

// ── AudioEncoderConfigurationOptions ─────────────────────────────────────────

/// Valid options for one audio encoding type.
#[derive(Debug, Clone, Default)]
pub struct AudioEncoderOptions {
    pub encoding: AudioEncoding,
    /// Supported bitrates in kbps.
    pub bitrate_list: Vec<u32>,
    /// Supported sample rates in kHz.
    pub sample_rate_list: Vec<u32>,
}

/// Valid parameter ranges for `SetAudioEncoderConfiguration`.
///
/// Contains one [`AudioEncoderOptions`] entry per encoding the device supports.
#[derive(Debug, Clone, Default)]
pub struct AudioEncoderConfigurationOptions {
    pub options: Vec<AudioEncoderOptions>,
}

impl AudioEncoderConfigurationOptions {
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let options = resp
            .children_named("Options")
            .map(|opt| {
                let encoding = xml_str(opt, "Encoding")
                    .map(|s| AudioEncoding::from_str(&s))
                    .unwrap_or_default();

                let bitrate_list = opt
                    .path(&["BitrateList", "Items"])
                    .map(|n| {
                        n.text()
                            .split_whitespace()
                            .filter_map(|s| s.parse().ok())
                            .collect()
                    })
                    .unwrap_or_default();

                let sample_rate_list = opt
                    .path(&["SampleRateList", "Items"])
                    .map(|n| {
                        n.text()
                            .split_whitespace()
                            .filter_map(|s| s.parse().ok())
                            .collect()
                    })
                    .unwrap_or_default();

                AudioEncoderOptions {
                    encoding,
                    bitrate_list,
                    sample_rate_list,
                }
            })
            .collect();

        Ok(Self { options })
    }
}
