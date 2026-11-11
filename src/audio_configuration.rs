//! Service-qualified audio source, output and decoder configuration.
//!
//! Configuration operations only affect SOAP settings. Audio packets use a
//! separately negotiated RTSP backchannel.

use crate::{
    OnvifClient, OnvifError, OnvifSession,
    configuration_write::{ConfigurationWrite, ConfigurationWriteError},
    media_workflow::MediaService,
    scoped_rpc::{MEDIA2, SCHEMA, ScopedService, field, required, scalar, text, token},
    soap::{
        SoapError,
        qualified::{QualifiedElement, invalid},
    },
    transport::EndpointPolicy,
    types::xml_escape,
};

const MEDIA1: &str = "http://www.onvif.org/ver10/media/wsdl";

/// One Media service's source, output and decoder configuration endpoint.
pub struct AudioConfigurations {
    service: ScopedService,
    binding: MediaService,
}

impl OnvifClient {
    /// Bind audio configuration to an explicit Media service on the device origin.
    pub fn audio_configurations(
        &self,
        endpoint: &str,
        binding: MediaService,
    ) -> Result<AudioConfigurations, OnvifError> {
        let url = EndpointPolicy::default().check(self.device_url(), endpoint)?;
        Ok(AudioConfigurations::new(self, url, binding))
    }
}

impl OnvifSession {
    /// Bind audio configuration to the selected service, respecting the session's endpoint policy.
    pub fn audio_configurations(
        &self,
        binding: MediaService,
    ) -> Result<AudioConfigurations, OnvifError> {
        Ok(AudioConfigurations::new(
            self.client(),
            self.trusted_media(binding)?,
            binding,
        ))
    }
}

/// Distinct kinds of audio configuration with different wire fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFamily {
    /// Audio input source association.
    Source,
    /// Speaker output association and level.
    Output,
    /// Decoder identity and name.
    Decoder,
}

/// Mutable standard fields of a selected audio configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioSettings {
    /// Audio input source association.
    Source {
        /// Audio source token.
        source_token: String,
    },
    /// Audio output association and volume controls.
    Output {
        /// Physical audio output token.
        output_token: String,
        /// Optional advertised send-primacy URI.
        send_primacy: Option<String>,
        /// Level in the device's advertised signed range.
        level: i32,
    },
    /// The standard decoder configuration only has the common name and identity.
    Decoder,
}

impl AudioSettings {
    fn family(&self) -> AudioFamily {
        match self {
            Self::Source { .. } => AudioFamily::Source,
            Self::Output { .. } => AudioFamily::Output,
            Self::Decoder => AudioFamily::Decoder,
        }
    }
}

/// Read-only identity and editable values of one audio configuration.
#[derive(Debug, Clone)]
pub struct AudioConfiguration {
    /// Configuration token, distinct from a profile, source or output token.
    pub token: String,
    /// Configuration name.
    pub name: String,
    /// Device-reported number of uses.
    pub use_count: u32,
    /// Service-specific standard fields.
    pub settings: AudioSettings,
    /// Original configuration XML when a vendor field is present.
    /// A typed replacement of that configuration is refused.
    pub unmodeled: Option<QualifiedElement>,
}

/// One advertised decoder format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderFormat {
    /// Codec name, including unrecognized Media2 encodings.
    pub encoding: String,
    /// Bitrates in kbps.
    pub bitrates: Vec<u32>,
    /// Sample rates in kHz.
    pub sample_rates: Vec<u32>,
}

/// Type-specific options, obtained with a configuration or profile token.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioConfigurationOptions {
    /// Physical inputs offered by the device.
    Source {
        /// Allowed source tokens.
        inputs: Vec<String>,
    },
    /// Output associations and level range.
    Output {
        /// Allowed output tokens.
        outputs: Vec<String>,
        /// Advertised send-primacy URIs.
        send_primacies: Vec<String>,
        /// Inclusive signed bounds.
        level_range: (i32, i32),
    },
    /// Media1 codec-specific or Media2 named format options.
    Decoder(Vec<DecoderFormat>),
}

type Operation = (&'static str, &'static str);

struct Operations {
    one: Operation,
    all: Operation,
    compatible: Operation,
    options: Operation,
    set: Operation,
}

macro_rules! audio_operations {
    ($family:literal) => {
        Operations {
            one: (
                concat!("GetAudio", $family, "Configuration"),
                concat!("GetAudio", $family, "ConfigurationResponse"),
            ),
            all: (
                concat!("GetAudio", $family, "Configurations"),
                concat!("GetAudio", $family, "ConfigurationsResponse"),
            ),
            compatible: (
                concat!("GetCompatibleAudio", $family, "Configurations"),
                concat!("GetCompatibleAudio", $family, "ConfigurationsResponse"),
            ),
            options: (
                concat!("GetAudio", $family, "ConfigurationOptions"),
                concat!("GetAudio", $family, "ConfigurationOptionsResponse"),
            ),
            set: (
                concat!("SetAudio", $family, "Configuration"),
                concat!("SetAudio", $family, "ConfigurationResponse"),
            ),
        }
    };
}

impl AudioFamily {
    fn operations(self) -> Operations {
        match self {
            Self::Source => audio_operations!("Source"),
            Self::Output => audio_operations!("Output"),
            Self::Decoder => audio_operations!("Decoder"),
        }
    }
}

impl AudioConfigurations {
    fn new(client: &OnvifClient, url: reqwest::Url, binding: MediaService) -> Self {
        let namespace = match binding {
            MediaService::Media1 => MEDIA1,
            MediaService::Media2 => MEDIA2,
        };
        Self {
            service: ScopedService {
                client: client.direct(),
                url,
                namespace,
            },
            binding,
        }
    }

    /// List configurations, optionally limited to those compatible with a profile.
    pub async fn list(
        &self,
        family: AudioFamily,
        profile: Option<&str>,
    ) -> Result<Vec<AudioConfiguration>, OnvifError> {
        let ops = family.operations();
        let op = if profile.is_some() && self.binding == MediaService::Media1 {
            ops.compatible
        } else {
            ops.all
        };
        let fields = selectors(None, profile)?;
        let response = self.service.call(op.0, op.1, &fields).await?;
        self.entries(&response, "Configurations", family)
    }

    /// Read a configuration by token. Media2 selects it via the plural operation.
    pub async fn get(
        &self,
        family: AudioFamily,
        configuration: &str,
    ) -> Result<AudioConfiguration, OnvifError> {
        let ops = family.operations();
        let (op, field) = if self.binding == MediaService::Media1 {
            (ops.one, "Configuration")
        } else {
            (ops.all, "Configurations")
        };
        let fields = selectors(Some(configuration), None)?;
        let response = self.service.call(op.0, op.1, &fields).await?;
        let mut result = self.entries(&response, field, family)?;
        if result.len() != 1 || result[0].token != configuration {
            return Err(invalid("expected a single matching audio configuration").into());
        }
        Ok(result.remove(0))
    }

    /// Read options scoped to a configuration and/or compatible profile.
    pub async fn options(
        &self,
        family: AudioFamily,
        configuration: Option<&str>,
        profile: Option<&str>,
    ) -> Result<AudioConfigurationOptions, OnvifError> {
        let op = family.operations().options;
        let fields = selectors(configuration, profile)?;
        let response = self.service.call(op.0, op.1, &fields).await?;
        let ns = self.service.namespace;
        let opts = || required(&response, ns, "Options");
        Ok(match family {
            AudioFamily::Source => AudioConfigurationOptions::Source {
                inputs: texts(opts()?, "InputTokensAvailable")?,
            },
            AudioFamily::Output => {
                let opts = opts()?;
                let range = required(opts, SCHEMA, "OutputLevelRange")?;
                let min = scalar(range, SCHEMA, "Min")?;
                let max = scalar(range, SCHEMA, "Max")?;
                if min > max {
                    return Err(invalid("invalid audio output level range").into());
                }
                AudioConfigurationOptions::Output {
                    outputs: texts(opts, "OutputTokensAvailable")?,
                    send_primacies: texts(opts, "SendPrimacyOptions")?,
                    level_range: (min, max),
                }
            }
            AudioFamily::Decoder => AudioConfigurationOptions::Decoder(match self.binding {
                MediaService::Media1 => [
                    ("AACDecOptions", "AAC"),
                    ("G711DecOptions", "G711"),
                    ("G726DecOptions", "G726"),
                ]
                .into_iter()
                .filter_map(|(name, codec)| {
                    opts()
                        .and_then(|node| node.one(SCHEMA, name))
                        .transpose()
                        .map(|node| {
                            node.and_then(|node| {
                                decoder_format(node, codec.into(), "Bitrate", "SampleRateRange")
                            })
                        })
                })
                .collect::<Result<Vec<_>, _>>()?,
                MediaService::Media2 => response
                    .children()
                    .filter(|node| node.namespace() == ns && node.local_name() == "Options")
                    .map(|node| {
                        decoder_format(
                            node,
                            text(node, SCHEMA, "Encoding")?,
                            "BitrateList",
                            "SampleRateList",
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            }),
        })
    }

    /// Validate against fresh options and replace a modeled configuration.
    /// Media1 sends ForcePersistence=true. Media2 has no persistence field.
    pub async fn set(
        &self,
        configuration: &str,
        name: &str,
        settings: &AudioSettings,
        profile: Option<&str>,
    ) -> Result<ConfigurationWrite<AudioConfiguration>, ConfigurationWriteError> {
        use ConfigurationWriteError as Error;
        if name.chars().take(65).count() > 64 || name.chars().any(char::is_control) {
            return Err(Error::invalid("invalid audio configuration name"));
        }
        let family = settings.family();
        let current = self
            .get(family, configuration)
            .await
            .map_err(Error::Preflight)?;
        if current.unmodeled.is_some() {
            return Err(Error::invalid("audio configuration has unmodeled fields"));
        }
        let options = self
            .options(family, Some(configuration), profile)
            .await
            .map_err(Error::Preflight)?;
        let fields = configuration_fields(settings, &options)?;
        let persistence = if self.binding == MediaService::Media1 {
            "<ForcePersistence>true</ForcePersistence>"
        } else {
            ""
        };
        let body = format!(
            "<Configuration token=\"{}\">{}<tt:UseCount>{}</tt:UseCount>{fields}</Configuration>{persistence}",
            xml_escape(configuration),
            field("tt:Name", name),
            current.use_count,
        );
        let op = family.operations().set;
        self.service
            .call(op.0, op.1, &body)
            .await
            .map_err(Error::after_send)?;
        let observed = self.get(family, configuration).await;
        Ok(ConfigurationWrite {
            token: configuration.into(),
            observed,
        })
    }

    fn entries(
        &self,
        response: &QualifiedElement,
        name: &str,
        family: AudioFamily,
    ) -> Result<Vec<AudioConfiguration>, OnvifError> {
        let mut tokens = std::collections::HashSet::new();
        let mut result = Vec::new();
        for item in response.children() {
            if item.namespace() != self.service.namespace
                || item.local_name() != name
                || result.len() >= 1024
            {
                return Err(invalid("invalid audio configuration list").into());
            }
            let config = parse_configuration(item, family)?;
            if !tokens.insert(config.token.clone()) {
                return Err(invalid("duplicate audio configuration token").into());
            }
            result.push(config);
        }
        Ok(result)
    }
}

fn selectors(configuration: Option<&str>, profile: Option<&str>) -> Result<String, OnvifError> {
    let mut fields = String::new();
    for (name, value) in [
        ("ConfigurationToken", configuration),
        ("ProfileToken", profile),
    ] {
        if let Some(value) = value {
            token(value)?;
            fields.push_str(&field(name, value));
        }
    }
    Ok(fields)
}

fn parse_configuration(
    node: &QualifiedElement,
    family: AudioFamily,
) -> Result<AudioConfiguration, OnvifError> {
    let id = node
        .attr("", "token")
        .ok_or_else(|| SoapError::missing("configuration token"))?;
    token(id)?;
    let (settings, fields): (_, &[&str]) = match family {
        AudioFamily::Source => {
            let source_token = text(node, SCHEMA, "SourceToken")?;
            token(&source_token)?;
            (AudioSettings::Source { source_token }, &["SourceToken"])
        }
        AudioFamily::Output => {
            let output_token = text(node, SCHEMA, "OutputToken")?;
            token(&output_token)?;
            let send_primacy = node
                .one(SCHEMA, "SendPrimacy")?
                .map(QualifiedElement::scalar)
                .transpose()?;
            (
                AudioSettings::Output {
                    output_token,
                    send_primacy,
                    level: scalar(node, SCHEMA, "OutputLevel")?,
                },
                &["OutputToken", "SendPrimacy", "OutputLevel"],
            )
        }
        AudioFamily::Decoder => (AudioSettings::Decoder, &[]),
    };
    let has_unmodeled = node.children().any(|item| {
        item.namespace() != SCHEMA
            || !matches!(item.local_name(), "Name" | "UseCount")
                && !fields.contains(&item.local_name())
            || !item.attributes().is_empty()
    }) || node
        .attributes()
        .iter()
        .any(|attr| !attr.namespace().is_empty() || attr.local_name() != "token");
    let unmodeled = if has_unmodeled {
        if node.serialized_len() > 256 * 1024 {
            return Err(invalid("audio configuration extension is too large").into());
        }
        Some(node.clone())
    } else {
        None
    };
    Ok(AudioConfiguration {
        token: id.into(),
        name: text(node, SCHEMA, "Name")?,
        use_count: scalar::<i32>(node, SCHEMA, "UseCount")?
            .try_into()
            .map_err(|_| invalid("invalid audio use count"))?,
        settings,
        unmodeled,
    })
}

fn texts(node: &QualifiedElement, name: &str) -> Result<Vec<String>, SoapError> {
    node.children()
        .filter(|item| item.namespace() == SCHEMA && item.local_name() == name)
        .map(QualifiedElement::scalar)
        .collect()
}

fn decoder_format(
    node: &QualifiedElement,
    encoding: String,
    bitrate: &'static str,
    rate: &'static str,
) -> Result<DecoderFormat, SoapError> {
    let read = |field| {
        texts(required(node, SCHEMA, field)?, "Items")?
            .iter()
            .map(|value| {
                value
                    .trim()
                    .parse::<i32>()
                    .ok()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| invalid("invalid decoder option number"))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(DecoderFormat {
        encoding,
        bitrates: read(bitrate)?,
        sample_rates: read(rate)?,
    })
}

fn configuration_fields(
    settings: &AudioSettings,
    options: &AudioConfigurationOptions,
) -> Result<String, ConfigurationWriteError> {
    use AudioConfigurationOptions as Options;
    let check = |value: &str| token(value).map_err(ConfigurationWriteError::Preflight);
    match (settings, options) {
        (AudioSettings::Source { source_token }, Options::Source { inputs })
            if inputs.contains(source_token) =>
        {
            check(source_token)?;
            Ok(field("tt:SourceToken", source_token))
        }
        (
            AudioSettings::Output {
                output_token,
                send_primacy,
                level,
            },
            Options::Output {
                outputs,
                send_primacies,
                level_range,
            },
        ) if outputs.contains(output_token)
            && (level_range.0..=level_range.1).contains(level)
            && send_primacy.as_ref().is_none_or(|value| {
                send_primacies.contains(value)
                    && value.len() <= 1024
                    && !value.chars().any(char::is_control)
            }) =>
        {
            check(output_token)?;
            let mut fields = field("tt:OutputToken", output_token);
            if let Some(value) = send_primacy {
                fields.push_str(&field("tt:SendPrimacy", value));
            }
            fields.push_str(&field("tt:OutputLevel", &level.to_string()));
            Ok(fields)
        }
        (AudioSettings::Decoder, Options::Decoder(_)) => Ok(String::new()),
        _ => Err(ConfigurationWriteError::invalid(
            "audio settings exceed device options",
        )),
    }
}
