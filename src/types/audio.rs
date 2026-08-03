use super::{xml_escape, xml_str, xml_u32};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── AudioSource ───────────────────────────────────────────────────────────────

/// A physical audio input returned by `GetAudioSources`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct AudioSourceConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    /// Human-readable name. Many devices simply echo the token here.
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AudioEncoding {
    /// ITU-T G.711 PCM (µ-law / A-law). The ONVIF baseline codec, and the
    /// default here because every conformant device supports it.
    #[default]
    G711,
    /// ITU-T G.726 ADPCM.
    G726,
    /// MPEG-4 AAC.
    Aac,
    /// An encoding string this crate does not model, kept verbatim as the
    /// device reported it (Media2 devices may advertise `MP4A-LATM`, `PCMU`, …).
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
///
/// **One Rust type, two ONVIF types.** Media1's
/// `tt:AudioEncoderConfiguration` requires [`multicast`](Self::multicast) and
/// [`session_timeout`](Self::session_timeout); Media2's
/// `tt:AudioEncoder2Configuration` makes `Multicast` optional, puts it before
/// `Bitrate`, and has no `SessionTimeout` at all. Both are read into this
/// struct and each service is written in its own shape — so a value that
/// arrived from one service can be sent to the other, but a Media2 write
/// carries no `session_timeout` for the device to store.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct AudioEncoderConfiguration {
    /// Opaque token for this configuration.
    pub token: String,
    /// Human-readable name. Many devices simply echo the token here.
    pub name: String,
    /// Number of profiles referencing this configuration.
    pub use_count: u32,
    /// Compression format the encoder produces.
    pub encoding: AudioEncoding,
    /// Bitrate in kbps (e.g. 64).
    pub bitrate: u32,
    /// Sample rate in kHz (e.g. 8).
    pub sample_rate: u32,
    /// Multicast streaming settings, if the device reported any.
    ///
    /// **Required** by `tt:AudioEncoderConfiguration` (Media1) and optional in
    /// `tt:AudioEncoder2Configuration` (Media2), so a Media1 `Set` that omits
    /// it is schema-invalid. Read it with the configuration and pass it back.
    pub multicast: Option<crate::types::MulticastConfiguration>,
    /// RTSP session timeout as an ISO 8601 duration (e.g. `"PT60S"`).
    ///
    /// Required by the Media1 type, absent from the Media2 one.
    pub session_timeout: Option<String>,
    /// Channel count, **if the device sent one**.
    ///
    /// `Channels` is *not* a member of either ONVIF audio encoder
    /// configuration type — it belongs to [`AudioSource`]. Both types end in an
    /// `<xs:any>` wildcard, so a vendor may add it, and it is read back and
    /// written out at the end of the sequence where the wildcard allows it.
    ///
    /// It was `u32` defaulting to `1` until 0.15, which meant every device
    /// appeared to report mono whether it had said anything or not, and the
    /// element was emitted mid-sequence where no schema permits it. Use
    /// [`AudioSource::channels`] for the physical channel count.
    pub channels: Option<u32>,
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
            multicast: node
                .child("Multicast")
                .map(crate::types::MulticastConfiguration::from_xml),
            session_timeout: xml_str(node, "SessionTimeout"),
            channels: xml_u32(node, "Channels"),
        })
    }

    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("Configurations")
            .map(Self::from_xml)
            .collect()
    }

    /// The `Multicast`, `SessionTimeout` and vendor `Channels` elements, in
    /// that order — the tail the two services order differently.
    fn tail(&self) -> (String, String, String) {
        let multicast = self
            .multicast
            .as_ref()
            .map(crate::types::MulticastConfiguration::to_xml_body)
            .unwrap_or_default();
        let session_timeout = self
            .session_timeout
            .as_deref()
            .map(|v| format!("<tt:SessionTimeout>{}</tt:SessionTimeout>", xml_escape(v)))
            .unwrap_or_default();
        // Only when the device sent one — see the field doc. Last, because that
        // is where the type's `<xs:any>` wildcard admits a vendor element.
        let channels = self
            .channels
            .map(|c| format!("<tt:Channels>{c}</tt:Channels>"))
            .unwrap_or_default();
        (multicast, session_timeout, channels)
    }

    /// Serialise to a `<trt:Configuration>` XML fragment for
    /// `SetAudioEncoderConfiguration`.
    ///
    /// Element order follows the `onvif.xsd` `AudioEncoderConfiguration`
    /// sequence — `Encoding`, `Bitrate`, `SampleRate`, `Multicast`,
    /// `SessionTimeout` — as [`VideoEncoderConfiguration`](crate::types::VideoEncoderConfiguration)
    /// already does for its own. `Multicast` and `SessionTimeout` are
    /// **required** here; omitting them, as this did until 0.15, makes the
    /// request invalid against the schema the device validates it with.
    pub(crate) fn to_xml_body(&self) -> String {
        let (multicast, session_timeout, channels) = self.tail();
        format!(
            "<trt:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Encoding>{encoding}</tt:Encoding>\
               <tt:Bitrate>{bitrate}</tt:Bitrate>\
               <tt:SampleRate>{sample_rate}</tt:SampleRate>\
               {multicast}{session_timeout}{channels}\
             </trt:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            // `AudioEncoding::Other` carries the device's raw string; going
            // through `Display` would put it on the wire unescaped.
            encoding = xml_escape(self.encoding.as_str()),
            bitrate = self.bitrate,
            sample_rate = self.sample_rate,
        )
    }

    /// Serialise to a `<tr2:Configuration>` XML fragment for
    /// `SetAudioEncoderConfiguration` (Media2).
    ///
    /// **Not just a prefix change.** Media2's `tt:AudioEncoder2Configuration`
    /// puts `Multicast` *before* `Bitrate` and `SampleRate`, and has no
    /// `SessionTimeout` at all, where Media1's puts it after them. The two
    /// fragments are identical only when `multicast` is `None`, which is why
    /// this is a separate function and not a parameterised prefix.
    pub(crate) fn to_xml_body_media2(&self) -> String {
        let (multicast, _, channels) = self.tail();
        format!(
            "<tr2:Configuration token=\"{token}\">\
               <tt:Name>{name}</tt:Name>\
               <tt:UseCount>{use_count}</tt:UseCount>\
               <tt:Encoding>{encoding}</tt:Encoding>\
               {multicast}\
               <tt:Bitrate>{bitrate}</tt:Bitrate>\
               <tt:SampleRate>{sample_rate}</tt:SampleRate>\
               {channels}\
             </tr2:Configuration>",
            token = xml_escape(&self.token),
            name = xml_escape(&self.name),
            use_count = self.use_count,
            encoding = xml_escape(self.encoding.as_str()),
            bitrate = self.bitrate,
            sample_rate = self.sample_rate,
        )
    }
}

// ── AudioEncoderConfigurationOptions ─────────────────────────────────────────

/// Valid options for one audio encoding type.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct AudioEncoderOptions {
    /// The encoding these options apply to.
    pub encoding: AudioEncoding,
    /// Supported bitrates in kbps.
    pub bitrate_list: Vec<u32>,
    /// Supported sample rates in kHz.
    pub sample_rate_list: Vec<u32>,
}

/// Valid parameter ranges for `SetAudioEncoderConfiguration`.
///
/// Contains one [`AudioEncoderOptions`] entry per encoding the device supports.
///
/// The two services nest this response differently — Media1 wraps the entries
/// one level deeper than Media2 — and both shapes are accepted. Until 0.15
/// only Media2's was, which gave a Media1 device a single entry holding the
/// default encoding and two empty lists rather than an error.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct AudioEncoderConfigurationOptions {
    /// One entry per encoding the device accepts; empty if it advertised none.
    pub options: Vec<AudioEncoderOptions>,
}

impl AudioEncoderConfigurationOptions {
    /// Parse either service's options response.
    ///
    /// **The two services nest differently, and this parser read only Media2's
    /// shape until 0.15.**
    ///
    /// ```text
    /// Media1  Response/Options              tt:AudioEncoderConfigurationOptions   ← a wrapper
    ///                 /Options              tt:AudioEncoderConfigurationOption    ← repeated, the real entry
    /// Media2  Response/Options              tt:AudioEncoder2ConfigurationOptions  ← repeated, IS the entry
    /// ```
    ///
    /// Reading only the outer level gave a Media1 device exactly one entry,
    /// with the *default* encoding (`G711`) and two empty lists, because
    /// `Encoding` and the lists are one level further down. Nothing errored —
    /// `AudioEncoderOptions` derives `Default` — so the caller saw a device
    /// that plausibly supports G711 at no bitrate.
    ///
    /// It survived because the unit fixture and the mock both used the flat
    /// shape: the fourth instance in this crate of a parser, a fixture and the
    /// mock agreeing with each other and with no conformant device (see
    /// `CLAUDE.md`, *Data nested in `Extension` levels*). Descend when the
    /// child is a wrapper, take it as-is when it is not.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let options = resp
            .children_named("Options")
            .flat_map(|o| {
                let nested: Vec<&XmlNode> = o.children_named("Options").collect();
                if nested.is_empty() { vec![o] } else { nested }
            })
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
