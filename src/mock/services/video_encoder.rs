//! VE1: scoped encoder reads/options and one validated shared-state commit.
use crate::mock::{
    effect::Effect,
    fault::{
        CONFIG_MODIFY, Code, Fault, INVALID_ARG_VAL, MOCK_REQUEST_POLICY, MOCK_UNMODELED_EFFECT,
        NO_CONFIG, NO_PROFILE,
    },
    helpers::soap,
    request::{Node, RequestError},
    state::{DeviceState, SharedState, VideoEncoderState},
};
const TT: &str = "http://www.onvif.org/ver10/schema";
const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";
fn ns(m2: bool) -> &'static str {
    if m2 { M2 } else { M1 }
}
fn prefix(m2: bool) -> &'static str {
    if m2 { "tr2" } else { "trt" }
}
fn response(m2: bool, op: &str, body: &str) -> String {
    let p = prefix(m2);
    soap(
        &format!("xmlns:{p}=\"{}\"", ns(m2)),
        &format!("<{p}:{op}Response>{body}</{p}:{op}Response>"),
    )
}
fn invalid(field: &str) -> String {
    Fault::new(
        Code::Sender,
        &[INVALID_ARG_VAL, CONFIG_MODIFY],
        &format!("Invalid mock encoder setting: {field}"),
    )
    .to_xml()
}
fn missing(token: &str) -> String {
    Fault::new(
        Code::Sender,
        &[INVALID_ARG_VAL, NO_CONFIG],
        &format!("Encoder configuration not found: {token}"),
    )
    .to_xml()
}
fn snapshot() -> String {
    Fault::new(
        Code::Receiver,
        &[MOCK_REQUEST_POLICY],
        "Invalid mock encoder snapshot",
    )
    .to_xml()
}
fn unmodeled() -> String {
    Fault::new(
        Code::Sender,
        &[MOCK_UNMODELED_EFFECT],
        "Unmodeled encoder configuration setting",
    )
    .to_xml()
}
fn shape(node: &Node, namespace: &str, children: &[&str], attrs: &[&str]) -> Result<(), String> {
    if node
        .expanded_attributes()
        .any(|(ns, name, _)| !ns.is_empty() || !attrs.contains(&name))
    {
        return Err(unmodeled());
    }
    node.check_child_sequence(namespace, children)
        .map_err(RequestError::to_fault)?;
    for name in children {
        node.child(namespace, name)
            .map_err(RequestError::to_fault)?;
    }
    Ok(())
}
fn child<'a>(node: &'a Node, namespace: &str, name: &str) -> Result<&'a Node, String> {
    node.child(namespace, name)
        .map_err(RequestError::to_fault)?
        .ok_or_else(|| RequestError::MissingField.to_fault())
}
fn text<'a>(node: &'a Node, name: &str) -> Result<&'a str, String> {
    let value = child(node, TT, name)?;
    if value.expanded_attributes().next().is_some() {
        return Err(unmodeled());
    }
    value.scalar_text().map_err(RequestError::to_fault)
}
fn trim(value: &str) -> &str {
    value.trim_matches([' ', '\t', '\n', '\r'])
}
fn int(value: &str, field: &str) -> Result<i32, String> {
    trim(value).parse().map_err(|_| invalid(field))
}
fn boolean(value: &str, field: &str) -> Result<bool, String> {
    match trim(value) {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(invalid(field)),
    }
}
fn identity(value: &str) -> Result<&str, String> {
    if value.is_empty() || value.chars().count() > 64 {
        Err(RequestError::EmptyField.to_fault())
    } else {
        Ok(value)
    }
}
fn selectors<'a>(
    operation: &'a Node,
    m2: bool,
    fields: &[&str],
) -> Result<(Option<&'a str>, Option<&'a str>), String> {
    shape(operation, ns(m2), fields, &[])?;
    let read = |name: &str| -> Result<Option<&str>, String> {
        operation
            .child(ns(m2), name)
            .map_err(RequestError::to_fault)?
            .map(|n| {
                if n.expanded_attributes().next().is_some() {
                    return Err(unmodeled());
                }
                identity(n.scalar_text().map_err(RequestError::to_fault)?)
            })
            .transpose()
    };
    Ok((read("ConfigurationToken")?, read("ProfileToken")?))
}
fn profile(state: &DeviceState, token: Option<&str>) -> Result<(), String> {
    if let Some(token) = token
        && !state.profiles.profiles.iter().any(|p| p.token == token)
    {
        return Err(Fault::new(
            Code::Sender,
            &[INVALID_ARG_VAL, NO_PROFILE],
            &format!("Profile not found: {token}"),
        )
        .to_xml());
    }
    Ok(())
}

pub(super) fn get(state: &SharedState, operation: &Node, m2: bool, singular: bool) -> String {
    let run = || -> Result<String, String> {
        let fields: &[&str] = if m2 {
            &["ConfigurationToken", "ProfileToken"]
        } else if singular {
            &["ConfigurationToken"]
        } else {
            &[]
        };
        let (token, context) = selectors(operation, m2, fields)?;
        if singular && token.is_none() {
            return Err(RequestError::MissingField.to_fault());
        }
        let state = state.read();
        profile(&state, context)?;
        if let Some(token) = token
            && !state.video_encoders.iter().any(|c| c.token == token)
        {
            return Err(missing(token));
        }
        let tag = format!(
            "{}:{}",
            prefix(m2),
            if singular {
                "Configuration"
            } else {
                "Configurations"
            }
        );
        let body = state
            .video_encoders
            .iter()
            .filter(|c| token.is_none_or(|t| c.token == t))
            .map(|c| {
                if m2 {
                    super::media2::render_video_encoder(c, &tag)
                } else {
                    super::media::render_vec_body(c, &tag)
                }
            })
            .collect::<Result<String, String>>()?;
        Ok(response(
            m2,
            if singular {
                "GetVideoEncoderConfiguration"
            } else {
                "GetVideoEncoderConfigurations"
            },
            &body,
        ))
    };
    run().unwrap_or_else(|f| f)
}

struct Limits {
    high: bool,
    resolutions: Vec<(u32, u32)>,
}
impl Limits {
    fn for_encoder(c: &VideoEncoderState) -> Result<Self, String> {
        if c.resolutions.is_empty()
            || c.resolutions
                .iter()
                .any(|(w, h)| *w == 0 || *h == 0 || *w > i32::MAX as u32 || *h > i32::MAX as u32)
        {
            return Err(snapshot());
        }
        Ok(Self {
            high: c.source_token == "VS_1",
            resolutions: c.resolutions.clone(),
        })
    }
    fn codecs(&self, m2: bool) -> Vec<&'static str> {
        let mut codecs = vec!["JPEG", "H264"];
        if self.high && m2 {
            codecs.push("H265");
        }
        codecs
    }
    fn max_rate(&self, codec: &str) -> u32 {
        if codec == "H265" {
            60
        } else if self.high {
            30
        } else {
            25
        }
    }
    fn max_bitrate(&self, codec: &str) -> u32 {
        if codec == "H265" {
            32768
        } else if self.high {
            16384
        } else {
            8192
        }
    }
    fn gov(&self, codec: &str) -> (u32, u32) {
        if codec == "H265" {
            (1, 600)
        } else if self.high {
            (1, 300)
        } else {
            (2, 150)
        }
    }
    fn profiles(&self, codec: &str) -> &'static [&'static str] {
        if codec == "H265" {
            &["Main", "Main10"]
        } else {
            &["Baseline", "Main", "High"]
        }
    }
    fn rates(&self, codec: &str) -> Vec<f32> {
        let mut rates: Vec<_> = (1..=self.max_rate(codec)).map(|n| n as f32).collect();
        rates.push(12.5);
        if self.max_rate(codec) >= 30 {
            rates.push(29.97);
        }
        rates.sort_by(f32::total_cmp);
        rates
    }
}
fn resolutions(list: &[(u32, u32)]) -> String {
    list.iter().map(|(w,h)| format!("<tt:ResolutionsAvailable><tt:Width>{w}</tt:Width><tt:Height>{h}</tt:Height></tt:ResolutionsAvailable>")).collect()
}
fn range(name: &str, min: u32, max: u32) -> String {
    format!("<tt:{name}><tt:Min>{min}</tt:Min><tt:Max>{max}</tt:Max></tt:{name}>")
}
pub(super) fn options(state: &SharedState, operation: &Node, m2: bool) -> String {
    let run = || -> Result<String, String> {
        let (token, context) = selectors(operation, m2, &["ConfigurationToken", "ProfileToken"])?;
        let state = state.read();
        profile(&state, context)?;
        let limits = if let Some(token) = token {
            Limits::for_encoder(
                state
                    .video_encoders
                    .iter()
                    .find(|c| c.token == token)
                    .ok_or_else(|| missing(token))?,
            )?
        } else {
            let mut all = Limits {
                high: false,
                resolutions: vec![],
            };
            for c in &state.video_encoders {
                let one = Limits::for_encoder(c)?;
                all.high |= one.high;
                for resolution in one.resolutions {
                    if !all.resolutions.contains(&resolution) {
                        all.resolutions.push(resolution);
                    }
                }
            }
            if all.resolutions.is_empty() {
                return Err(snapshot());
            }
            all
        };
        let full = resolutions(&limits.resolutions);
        let quality = range("QualityRange", 0, 10);
        let body = if m2 {
            limits.codecs(true).iter().map(|codec| {
                let rates = limits.rates(codec).iter().map(ToString::to_string).collect::<Vec<_>>().join(" ");
                let attrs = if *codec == "JPEG" { String::new() } else {
                    let (min,max) = limits.gov(codec);
                    format!(" GovLengthRange=\"{min} {max}\" ProfilesSupported=\"{}\"", limits.profiles(codec).join(" "))
                };
                format!("<tr2:Options FrameRatesSupported=\"{rates}\"{attrs}><tt:Encoding>{codec}</tt:Encoding>{quality}{full}{}</tr2:Options>", range("BitrateRange",64,limits.max_bitrate(codec)))
            }).collect::<String>()
        } else {
            // Keep a deliberately smaller shallow view to test Extension precedence.
            let shallow = if limits.resolutions.len() > 1 {
                resolutions(&limits.resolutions[1..])
            } else {
                full.clone()
            };
            let block = |codec: &str, res: &str, extended: bool| {
                let (min, max) = limits.gov(codec);
                let gov = if codec == "H264" {
                    range("GovLengthRange", min, max)
                } else {
                    String::new()
                };
                let profiles = if codec == "H264" {
                    limits
                        .profiles(codec)
                        .iter()
                        .map(|p| {
                            format!("<tt:H264ProfilesSupported>{p}</tt:H264ProfilesSupported>")
                        })
                        .collect()
                } else {
                    String::new()
                };
                let bitrate = if extended {
                    range("BitrateRange", 64, limits.max_bitrate(codec))
                } else {
                    String::new()
                };
                format!(
                    "<tt:{codec}>{res}{gov}{}{}{profiles}{bitrate}</tt:{codec}>",
                    range("FrameRateRange", 1, limits.max_rate(codec)),
                    range("EncodingIntervalRange", 1, 1)
                )
            };
            format!(
                "<trt:Options>{quality}{}{}<tt:Extension>{}{}</tt:Extension></trt:Options>",
                block("JPEG", &shallow, false),
                block("H264", &shallow, false),
                block("JPEG", &full, true),
                block("H264", &full, true)
            )
        };
        Ok(response(m2, "GetVideoEncoderConfigurationOptions", &body))
    };
    run().unwrap_or_else(|f| f)
}

struct Candidate<'a> {
    token: &'a str,
    name: &'a str,
    codec: &'a str,
    width: u32,
    height: u32,
    quality: f32,
    rate: Option<(f32, i32)>,
    gov: Option<u32>,
    profile: Option<&'a str>,
}
fn multicast(node: &Node) -> Result<(), String> {
    shape(node, TT, &["Address", "Port", "TTL", "AutoStart"], &[])?;
    let address = child(node, TT, "Address")?;
    shape(address, TT, &["Type", "IPv4Address"], &[])?;
    if text(address, "Type")? != "IPv4"
        || text(address, "IPv4Address")? != "0.0.0.0"
        || int(text(node, "Port")?, "Multicast/Port")? != 0
        || int(text(node, "TTL")?, "Multicast/TTL")? != 1
        || boolean(text(node, "AutoStart")?, "Multicast/AutoStart")?
    {
        return Err(unmodeled());
    }
    Ok(())
}
fn candidate(operation: &Node, m2: bool) -> Result<Candidate<'_>, String> {
    shape(
        operation,
        ns(m2),
        if m2 {
            &["Configuration"]
        } else {
            &["Configuration", "ForcePersistence"]
        },
        &[],
    )?;
    if !m2 {
        let persistence = child(operation, M1, "ForcePersistence")?;
        if persistence.expanded_attributes().next().is_some() {
            return Err(unmodeled());
        }
        boolean(
            persistence.scalar_text().map_err(RequestError::to_fault)?,
            "ForcePersistence",
        )?;
    }
    let config = child(operation, ns(m2), "Configuration")?;
    shape(
        config,
        TT,
        if m2 {
            &[
                "Name",
                "UseCount",
                "Encoding",
                "Resolution",
                "RateControl",
                "Multicast",
                "Quality",
            ]
        } else {
            &[
                "Name",
                "UseCount",
                "Encoding",
                "Resolution",
                "Quality",
                "RateControl",
                "H264",
                "Multicast",
                "SessionTimeout",
            ]
        },
        if m2 {
            &[
                "token",
                "GovLength",
                "Profile",
                "GuaranteedFrameRate",
                "Signed",
            ]
        } else {
            &["token", "GuaranteedFrameRate"]
        },
    )?;
    let token = identity(
        config
            .attribute("", "token")
            .ok_or_else(|| RequestError::MissingField.to_fault())?,
    )?;
    let name = text(config, "Name")?;
    if name.chars().count() > 64 {
        return Err(invalid("Name"));
    }
    int(text(config, "UseCount")?, "UseCount")?;
    let codec = trim(text(config, "Encoding")?);
    for attr in ["GuaranteedFrameRate", "Signed"] {
        if let Some(value) = config.attribute("", attr)
            && boolean(value, attr)?
        {
            return Err(unmodeled());
        }
    }
    let resolution = child(config, TT, "Resolution")?;
    shape(resolution, TT, &["Width", "Height"], &[])?;
    let positive = |value: &str, field: &str| -> Result<u32, String> {
        u32::try_from(int(value, field)?)
            .ok()
            .filter(|v| *v > 0)
            .ok_or_else(|| invalid(field))
    };
    let width = positive(text(resolution, "Width")?, "Resolution/Width")?;
    let height = positive(text(resolution, "Height")?, "Resolution/Height")?;
    let quality = trim(text(config, "Quality")?)
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| invalid("Quality"))?;
    let rate = super::video_rate::candidate(operation, m2)?;
    if let Some(node) = config
        .child(TT, "RateControl")
        .map_err(RequestError::to_fault)?
    {
        if node
            .expanded_attributes()
            .any(|(ns, name, _)| !m2 || !ns.is_empty() || name != "ConstantBitRate")
        {
            return Err(unmodeled());
        }
        if let Some(value) = node.attribute("", "ConstantBitRate")
            && boolean(value, "ConstantBitRate")?
        {
            return Err(unmodeled());
        }
        for (_, _, scalar) in node.element_children() {
            if scalar.expanded_attributes().next().is_some() {
                return Err(unmodeled());
            }
        }
        if !m2 && int(text(node, "EncodingInterval")?, "EncodingInterval")? != 1 {
            return Err(unmodeled());
        }
    }
    if let Some(node) = config
        .child(TT, "Multicast")
        .map_err(RequestError::to_fault)?
    {
        multicast(node)?;
    } else if !m2 {
        return Err(RequestError::MissingField.to_fault());
    }
    if !m2
        && !matches!(
            trim(text(config, "SessionTimeout")?),
            "PT0S" | "PT0.0S" | "P0D"
        )
    {
        return Err(unmodeled());
    }
    let (gov, profile) = if m2 {
        (
            config
                .attribute("", "GovLength")
                .map(|v| positive(v, "GovLength"))
                .transpose()?,
            config.attribute("", "Profile"),
        )
    } else if let Some(h264) = config.child(TT, "H264").map_err(RequestError::to_fault)? {
        if codec != "H264" {
            return Err(invalid("H264"));
        }
        shape(h264, TT, &["GovLength", "H264Profile"], &[])?;
        (
            Some(positive(text(h264, "GovLength")?, "GovLength")?),
            Some(trim(text(h264, "H264Profile")?)),
        )
    } else {
        (None, None)
    };
    Ok(Candidate {
        token,
        name,
        codec,
        width,
        height,
        quality,
        rate,
        gov,
        profile,
    })
}
pub(super) fn set(
    state: &SharedState,
    operation: &Node,
    m2: bool,
    effect: &mut Option<Effect>,
) -> String {
    let c = match candidate(operation, m2) {
        Ok(c) => c,
        Err(f) => return f,
    };
    let result = state.modify_returning_if(
        |state| -> Result<(), String> {
            let original = state
                .video_encoders
                .iter_mut()
                .find(|ve| ve.token == c.token)
                .ok_or_else(|| missing(c.token))?;
            let limits = Limits::for_encoder(original)?;
            if !limits.codecs(m2).contains(&c.codec) {
                return Err(invalid("Encoding"));
            }
            if !limits.resolutions.contains(&(c.width, c.height)) {
                return Err(invalid("Resolution"));
            }
            let mut updated = original.clone();
            updated.name = c.name.into();
            updated.encoding = c.codec.into();
            updated.width = c.width;
            updated.height = c.height;
            updated.quality = c.quality.clamp(0.0, 10.0);
            if c.codec == "JPEG" {
                if c.gov.is_some() || c.profile.is_some() {
                    return Err(unmodeled());
                }
                updated.gov_length = 1;
                updated.profile.clear();
            } else {
                let same = original.encoding == c.codec;
                let gov = c.gov.unwrap_or(if same {
                    original.gov_length
                } else {
                    limits.gov(c.codec).0
                });
                let profile = c
                    .profile
                    .unwrap_or(if same { &original.profile } else { "Main" });
                let (min, max) = limits.gov(c.codec);
                if !(min..=max).contains(&gov) {
                    return Err(invalid("GovLength"));
                }
                if !limits.profiles(c.codec).contains(&profile) {
                    return Err(invalid("Profile"));
                }
                updated.gov_length = gov;
                updated.profile = profile.into();
            }
            if let Some((fps, bitrate)) = c.rate {
                updated.frame_rate_limit = if m2 {
                    let fps = fps.clamp(1.0, limits.max_rate(c.codec) as f32);
                    limits
                        .rates(c.codec)
                        .into_iter()
                        .min_by(|a, b| (fps - *a).abs().total_cmp(&(fps - *b).abs()))
                        .ok_or_else(snapshot)?
                } else {
                    fps.clamp(1.0, limits.max_rate(c.codec) as f32)
                };
                updated.bitrate_limit =
                    bitrate.clamp(64, limits.max_bitrate(c.codec) as i32) as u32;
            }
            *original = updated;
            Ok(())
        },
        Result::is_ok,
    );
    match result {
        Ok(()) => {
            *effect = Some(Effect::VideoEncoderCommitted);
            response(m2, "SetVideoEncoderConfiguration", "")
        }
        Err(f) => f,
    }
}
pub(super) fn instances(state: &SharedState, operation: &Node) -> String {
    let run = || -> Result<String, String> {
        let (token, _) = selectors(operation, true, &["ConfigurationToken"])?;
        let token = token.ok_or_else(|| RequestError::MissingField.to_fault())?;
        let state = state.read();
        let config = state
            .video_source_configs
            .iter()
            .find(|c| c.token == token)
            .ok_or_else(|| missing(token))?;
        if !state
            .video_sources
            .iter()
            .any(|s| s.token == config.source_token)
            || state.video_source_configs.len() > super::media::PROFILE_LIMIT
        {
            return Err(snapshot());
        }
        let high = config.source_token == "VS_1";
        let total = (if high { 4 } else { 2 })
            .min(super::media::PROFILE_LIMIT / state.video_source_configs.len());
        let codecs = if high {
            vec!["JPEG", "H264", "H265"]
        } else {
            vec!["JPEG", "H264"]
        };
        let body: String = codecs.iter().map(|codec|format!("<tr2:Codec><tr2:Encoding>{codec}</tr2:Encoding><tr2:Number>{}</tr2:Number></tr2:Codec>",total.min(2))).collect();
        Ok(response(
            true,
            "GetVideoEncoderInstances",
            &format!("<tr2:Info>{body}<tr2:Total>{total}</tr2:Total></tr2:Info>"),
        ))
    };
    run().unwrap_or_else(|f| f)
}
