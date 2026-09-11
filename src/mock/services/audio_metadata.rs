//! AM1: scoped audio/metadata views and atomic, options-checked candidates.
use crate::mock::{
    effect::Effect,
    fault::{
        CONFIG_MODIFY, Code, Fault, INVALID_ARG_VAL, MOCK_REQUEST_POLICY, MOCK_UNMODELED_EFFECT,
        NO_CONFIG, NO_PROFILE,
    },
    helpers::soap,
    request::{Node, RequestError},
    state::{
        AudioEncoderEntry, AudioOptionEntry, DeviceState, MetadataEntry, MulticastEntry,
        SharedState,
    },
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
        &format!("Invalid mock audio/metadata setting: {field}"),
    )
    .to_xml()
}
fn missing(token: &str) -> String {
    Fault::new(
        Code::Sender,
        &[INVALID_ARG_VAL, NO_CONFIG],
        &format!("Configuration not found: {token}"),
    )
    .to_xml()
}
fn snapshot() -> String {
    Fault::new(
        Code::Receiver,
        &[MOCK_REQUEST_POLICY],
        "Invalid mock audio/metadata snapshot",
    )
    .to_xml()
}
fn unmodeled() -> String {
    Fault::new(
        Code::Sender,
        &[MOCK_UNMODELED_EFFECT],
        "Unmodeled audio/metadata configuration setting",
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

fn entity(token: &str, name: &str, count: u32) -> Result<(), String> {
    if identity(token).is_err() || name.chars().count() > 64 || count > i32::MAX as u32 {
        return Err(snapshot());
    }
    Ok(())
}
fn mc_valid(m: &MulticastEntry) -> bool {
    m.address
        .parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_multicast() || ip.is_unspecified())
        && m.port <= 65535
        && m.ttl <= 255
        && !m.auto_start
}
fn mc_render(m: &MulticastEntry) -> Result<String, String> {
    if !mc_valid(m) {
        return Err(snapshot());
    }
    let (kind, tag) = if m.address.contains(':') {
        ("IPv6", "IPv6Address")
    } else {
        ("IPv4", "IPv4Address")
    };
    Ok(format!(
        "<tt:Multicast><tt:Address><tt:Type>{kind}</tt:Type><tt:{tag}>{}</tt:{tag}></tt:Address><tt:Port>{}</tt:Port><tt:TTL>{}</tt:TTL><tt:AutoStart>false</tt:AutoStart></tt:Multicast>",
        crate::types::xml_escape(&m.address),
        m.port,
        m.ttl
    ))
}
fn mc_parse(n: &Node) -> Result<MulticastEntry, String> {
    shape(n, TT, &["Address", "Port", "TTL", "AutoStart"], &[])?;
    let a = child(n, TT, "Address")?;
    shape(a, TT, &["Type", "IPv4Address", "IPv6Address"], &[])?;
    let kind = trim(text(a, "Type")?);
    let tag = match kind {
        "IPv4" => "IPv4Address",
        "IPv6" => "IPv6Address",
        _ => return Err(invalid("Multicast/Address/Type")),
    };
    let other = if kind == "IPv4" {
        "IPv6Address"
    } else {
        "IPv4Address"
    };
    if a.child(TT, other)
        .map_err(RequestError::to_fault)?
        .is_some()
    {
        return Err(invalid("Multicast/Address"));
    }
    let address = trim(text(a, tag)?).to_owned();
    let ip = address
        .parse::<std::net::IpAddr>()
        .map_err(|_| invalid("Multicast/Address"))?;
    if ip.is_ipv4() != (kind == "IPv4") {
        return Err(invalid("Multicast/Address"));
    }
    let port = positive_or_zero(text(n, "Port")?, "Multicast/Port")?;
    let ttl = positive_or_zero(text(n, "TTL")?, "Multicast/TTL")?;
    // Readonly indication: validate input but never start persistent RTP.
    boolean(text(n, "AutoStart")?, "Multicast/AutoStart")?;
    let m = MulticastEntry {
        address,
        port,
        ttl,
        auto_start: false,
    };
    if !mc_valid(&m) {
        return Err(invalid("Multicast"));
    }
    Ok(m)
}
fn positive_or_zero(s: &str, field: &str) -> Result<u32, String> {
    u32::try_from(int(s, field)?).map_err(|_| invalid(field))
}
fn duration(s: &str) -> Result<String, String> {
    let s = trim(s);
    if !crate::types::valid_metadata_duration(s) {
        return Err(invalid("SessionTimeout"));
    }
    Ok(s.to_owned())
}
fn codec(m2: bool, name: &str, bitrate: u32) -> Result<String, String> {
    match (m2, name) {
        (false, "G711" | "AAC" | "G726") => Ok(name.into()),
        (true, "G711") => Ok("PCMU".into()),
        (true, "AAC") => Ok("MP4A-LATM".into()),
        (true, "G726") if [16, 24, 32, 40].contains(&bitrate) => Ok("G726".into()),
        _ => Err(snapshot()),
    }
}
fn options_valid(c: &AudioEncoderEntry) -> Result<(), String> {
    if c.options.is_empty() {
        return Err(snapshot());
    }
    for o in &c.options {
        if o.bitrates.is_empty()
            || o.sample_rates.is_empty()
            || o.bitrates
                .iter()
                .chain(&o.sample_rates)
                .any(|v| *v == 0 || *v > i32::MAX as u32)
        {
            return Err(snapshot());
        }
        for b in &o.bitrates {
            codec(true, &o.encoding, *b)?;
            codec(false, &o.encoding, *b)?;
        }
    }
    Ok(())
}
pub(super) fn audio_render(c: &AudioEncoderEntry, q: &str, m2: bool) -> Result<String, String> {
    entity(&c.token, &c.name, c.use_count)?;
    options_valid(c)?;
    if !c.options.iter().any(|o| {
        o.encoding == c.encoding
            && o.bitrates.contains(&c.bitrate)
            && o.sample_rates.contains(&c.sample_rate)
    }) {
        return Err(snapshot());
    }
    let multicast = c.multicast.as_ref().map(mc_render).transpose()?;
    if !m2 && multicast.is_none() {
        return Err(snapshot());
    }
    let multicast = multicast.unwrap_or_default();
    let timeout = if m2 {
        String::new()
    } else {
        let t = c.session_timeout.as_deref().ok_or_else(snapshot)?;
        duration(t).map_err(|_| snapshot())?;
        format!(
            "<tt:SessionTimeout>{}</tt:SessionTimeout>",
            crate::types::xml_escape(t)
        )
    };
    let rates = format!(
        "<tt:Bitrate>{}</tt:Bitrate><tt:SampleRate>{}</tt:SampleRate>",
        c.bitrate, c.sample_rate
    );
    let tail = if m2 {
        format!("{multicast}{rates}")
    } else {
        format!("{rates}{multicast}{timeout}")
    };
    Ok(format!(
        "<{q} token=\"{}\"><tt:Name>{}</tt:Name><tt:UseCount>{}</tt:UseCount><tt:Encoding>{}</tt:Encoding>{tail}</{q}>",
        crate::types::xml_escape(&c.token),
        crate::types::xml_escape(&c.name),
        c.use_count,
        codec(m2, &c.encoding, c.bitrate)?
    ))
}
fn metadata_render(c: &MetadataEntry, q: &str) -> Result<String, String> {
    entity(&c.token, &c.name, c.use_count)?;
    duration(&c.session_timeout).map_err(|_| snapshot())?;
    if (c.ptz_status || c.ptz_position) && !(c.pan_tilt_status_supported || c.zoom_status_supported)
    {
        return Err(snapshot());
    }
    Ok(format!(
        "<{q} token=\"{}\"><tt:Name>{}</tt:Name><tt:UseCount>{}</tt:UseCount><tt:PTZStatus><tt:Status>{}</tt:Status><tt:Position>{}</tt:Position></tt:PTZStatus><tt:Analytics>{}</tt:Analytics>{}<tt:SessionTimeout>{}</tt:SessionTimeout></{q}>",
        crate::types::xml_escape(&c.token),
        crate::types::xml_escape(&c.name),
        c.use_count,
        c.ptz_status,
        c.ptz_position,
        c.analytics,
        mc_render(&c.multicast)?,
        crate::types::xml_escape(&c.session_timeout)
    ))
}
pub(super) fn get(state: &SharedState, operation: &Node, m2: bool, op: &str) -> String {
    let run = || -> Result<String, String> {
        let fields: &[&str] = if m2 {
            &["ConfigurationToken", "ProfileToken"]
        } else if op == "GetAudioEncoderConfiguration" {
            &["ConfigurationToken"]
        } else {
            &[]
        };
        let (token, context) = selectors(operation, m2, fields)?;
        if op == "GetAudioEncoderConfiguration" && token.is_none() {
            return Err(RequestError::MissingField.to_fault());
        }
        let s = state.read();
        profile(&s, context)?;
        let q = format!(
            "{}:{}",
            prefix(m2),
            if op == "GetAudioEncoderConfiguration" {
                "Configuration"
            } else {
                "Configurations"
            }
        );
        let mut entries: Vec<(&str, String)> = vec![];
        match op {
            "GetAudioSources" => {
                for c in &s.audio_sources {
                    entity(&c.token, "", 0)?;
                    if c.channels == 0 || c.channels > i32::MAX as u32 {
                        return Err(snapshot());
                    }
                    entries.push((&c.token, format!("<trt:AudioSources token=\"{}\"><tt:Channels>{}</tt:Channels></trt:AudioSources>",crate::types::xml_escape(&c.token),c.channels)));
                }
            }
            "GetAudioSourceConfigurations" => {
                for c in &s.audio_source_configs {
                    entity(&c.token, &c.name, c.use_count)?;
                    if !s
                        .audio_sources
                        .iter()
                        .any(|source| source.token == c.source_token)
                    {
                        return Err(snapshot());
                    }
                    entries.push((&c.token, super::media::render_audio_source_config(c, &q)));
                }
            }
            "GetAudioEncoderConfiguration" | "GetAudioEncoderConfigurations" => {
                for c in &s.audio_encoders {
                    entries.push((&c.token, audio_render(c, &q, m2)?));
                }
            }
            "GetMetadataConfigurations" => {
                for c in &s.metadata {
                    entries.push((&c.token, metadata_render(c, &q)?));
                }
            }
            "GetAudioOutputConfigurations" => {
                for c in &s.audio_outputs {
                    entity(&c.token, &c.name, c.use_count)?;
                    if identity(&c.output_token).is_err() || c.output_level > i32::MAX as u32 {
                        return Err(snapshot());
                    }
                    entries.push((&c.token,format!("<{q} token=\"{}\"><tt:Name>{}</tt:Name><tt:UseCount>{}</tt:UseCount><tt:OutputToken>{}</tt:OutputToken><tt:OutputLevel>{}</tt:OutputLevel></{q}>",crate::types::xml_escape(&c.token),crate::types::xml_escape(&c.name),c.use_count,crate::types::xml_escape(&c.output_token),c.output_level)));
                }
            }
            "GetAudioDecoderConfigurations" => {
                for c in &s.audio_decoders {
                    entity(&c.token, &c.name, c.use_count)?;
                    entries.push((&c.token,format!("<{q} token=\"{}\"><tt:Name>{}</tt:Name><tt:UseCount>{}</tt:UseCount></{q}>",crate::types::xml_escape(&c.token),crate::types::xml_escape(&c.name),c.use_count)));
                }
            }
            _ => return Err(unmodeled()),
        }
        if let Some(t) = token
            && !entries.iter().any(|(key, _)| *key == t)
        {
            return Err(missing(t));
        }
        let body: String = entries
            .into_iter()
            .filter(|(key, _)| token.is_none_or(|t| t == *key))
            .map(|(_, xml)| xml)
            .collect();
        Ok(response(m2, op, &body))
    };
    run().unwrap_or_else(|f| f)
}
pub(super) fn options(state: &SharedState, operation: &Node, m2: bool, metadata: bool) -> String {
    let run = || -> Result<String, String> {
        let (token, context) = selectors(operation, m2, &["ConfigurationToken", "ProfileToken"])?;
        let s = state.read();
        profile(&s, context)?;
        if metadata {
            if let Some(t) = token
                && !s.metadata.iter().any(|c| c.token == t)
            {
                return Err(missing(t));
            }
            let mut pan = false;
            let mut zoom = false;
            for c in s
                .metadata
                .iter()
                .filter(|c| token.is_none_or(|t| c.token == t))
            {
                metadata_render(c, "tr2:Configurations")?;
                pan |= c.pan_tilt_status_supported;
                zoom |= c.zoom_status_supported;
            }
            Ok(response(
                true,
                "GetMetadataConfigurationOptions",
                &format!(
                    "<tr2:Options><tt:PTZStatusFilterOptions><tt:PanTiltStatusSupported>{pan}</tt:PanTiltStatusSupported><tt:ZoomStatusSupported>{zoom}</tt:ZoomStatusSupported></tt:PTZStatusFilterOptions></tr2:Options>"
                ),
            ))
        } else {
            if let Some(t) = token
                && !s.audio_encoders.iter().any(|c| c.token == t)
            {
                return Err(missing(t));
            }
            let mut rows = vec![];
            for c in s
                .audio_encoders
                .iter()
                .filter(|c| token.is_none_or(|t| c.token == t))
            {
                options_valid(c)?;
                for o in &c.options {
                    let row = AudioOptionEntry {
                        encoding: codec(m2, &o.encoding, o.bitrates[0])?,
                        bitrates: o.bitrates.clone(),
                        sample_rates: o.sample_rates.clone(),
                    };
                    let xml = super::media::render_audio_option(
                        &row,
                        if m2 { "tr2:Options" } else { "tt:Options" },
                    );
                    if !rows.contains(&xml) {
                        rows.push(xml);
                    }
                }
            }
            let body = rows.concat();
            Ok(response(
                m2,
                "GetAudioEncoderConfigurationOptions",
                &if m2 {
                    body
                } else {
                    format!("<trt:Options>{body}</trt:Options>")
                },
            ))
        }
    };
    run().unwrap_or_else(|f| f)
}
struct Candidate<'a> {
    token: &'a str,
    name: &'a str,
    config: &'a Node,
}
fn candidate<'a>(operation: &'a Node, m2: bool, fields: &[&str]) -> Result<Candidate<'a>, String> {
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
        let n = child(operation, M1, "ForcePersistence")?;
        if n.expanded_attributes().next().is_some() {
            return Err(unmodeled());
        }
        boolean(
            n.scalar_text().map_err(RequestError::to_fault)?,
            "ForcePersistence",
        )?;
    }
    let config = child(operation, ns(m2), "Configuration")?;
    shape(config, TT, fields, &["token"])?;
    let token = identity(
        config
            .attribute("", "token")
            .ok_or_else(|| RequestError::MissingField.to_fault())?,
    )?;
    let name = text(config, "Name")?;
    if name.chars().count() > 64 {
        return Err(invalid("Name"));
    }
    positive_or_zero(text(config, "UseCount")?, "UseCount")?;
    Ok(Candidate {
        token,
        name,
        config,
    })
}
pub(super) fn set_audio(
    state: &SharedState,
    operation: &Node,
    m2: bool,
    effect: &mut Option<Effect>,
) -> String {
    let run = || -> Result<(), String> {
        let c = candidate(
            operation,
            m2,
            if m2 {
                &[
                    "Name",
                    "UseCount",
                    "Encoding",
                    "Multicast",
                    "Bitrate",
                    "SampleRate",
                ]
            } else {
                &[
                    "Name",
                    "UseCount",
                    "Encoding",
                    "Bitrate",
                    "SampleRate",
                    "Multicast",
                    "SessionTimeout",
                ]
            },
        )?;
        let encoding = trim(text(c.config, "Encoding")?);
        let bitrate = positive_or_zero(text(c.config, "Bitrate")?, "Bitrate")?;
        let rate = positive_or_zero(text(c.config, "SampleRate")?, "SampleRate")?;
        let multicast = c
            .config
            .child(TT, "Multicast")
            .map_err(RequestError::to_fault)?
            .map(mc_parse)
            .transpose()?;
        let timeout = if m2 {
            None
        } else {
            Some(duration(text(c.config, "SessionTimeout")?)?)
        };
        if !m2 && multicast.is_none() {
            return Err(RequestError::MissingField.to_fault());
        }
        state.modify_returning_if(
            |s| -> Result<(), String> {
                let current = s
                    .audio_encoders
                    .iter_mut()
                    .find(|e| e.token == c.token)
                    .ok_or_else(|| missing(c.token))?;
                options_valid(current)?;
                let option = current
                    .options
                    .iter()
                    .find(|o| {
                        o.bitrates.contains(&bitrate)
                            && o.sample_rates.contains(&rate)
                            && codec(m2, &o.encoding, bitrate).is_ok_and(|name| name == encoding)
                    })
                    .ok_or_else(|| invalid("Encoding/Bitrate/SampleRate"))?;
                let mut next = current.clone();
                next.name = c.name.into();
                next.encoding = option.encoding.clone();
                next.bitrate = bitrate;
                next.sample_rate = rate;
                if let Some(m) = multicast {
                    next.multicast = Some(m);
                }
                if let Some(t) = timeout {
                    next.session_timeout = Some(t);
                }
                audio_render(&next, "trt:Configuration", false)?;
                *current = next;
                Ok(())
            },
            Result::is_ok,
        )
    };
    match run() {
        Ok(()) => {
            *effect = Some(Effect::AudioEncoderCommitted);
            response(m2, "SetAudioEncoderConfiguration", "")
        }
        Err(f) => f,
    }
}
pub(super) fn set_metadata(
    state: &SharedState,
    operation: &Node,
    effect: &mut Option<Effect>,
) -> String {
    let run = || -> Result<(), String> {
        let c = candidate(
            operation,
            true,
            &[
                "Name",
                "UseCount",
                "PTZStatus",
                "Analytics",
                "Multicast",
                "SessionTimeout",
            ],
        )?;
        let ptz = c
            .config
            .child(TT, "PTZStatus")
            .map_err(RequestError::to_fault)?
            .map(|n| {
                shape(n, TT, &["Status", "Position"], &[])?;
                Ok::<_, String>((
                    boolean(text(n, "Status")?, "PTZStatus/Status")?,
                    boolean(text(n, "Position")?, "PTZStatus/Position")?,
                ))
            })
            .transpose()?;
        let analytics = c
            .config
            .child(TT, "Analytics")
            .map_err(RequestError::to_fault)?
            .map(|_| boolean(text(c.config, "Analytics")?, "Analytics"))
            .transpose()?;
        let multicast = mc_parse(child(c.config, TT, "Multicast")?)?;
        // Required but deprecated in Media2: validate the lexical form; do not change stored lifetime.
        duration(text(c.config, "SessionTimeout")?)?;
        state.modify_returning_if(
            |s| -> Result<(), String> {
                let current = s
                    .metadata
                    .iter_mut()
                    .find(|e| e.token == c.token)
                    .ok_or_else(|| missing(c.token))?;
                let mut next = current.clone();
                next.name = c.name.into();
                next.multicast = multicast;
                if let Some(a) = analytics {
                    next.analytics = a;
                }
                if let Some((status, position)) = ptz {
                    if (status || position)
                        && !(next.pan_tilt_status_supported || next.zoom_status_supported)
                    {
                        return Err(invalid("PTZStatus"));
                    }
                    next.ptz_status = status;
                    next.ptz_position = position;
                }
                metadata_render(&next, "tr2:Configurations")?;
                *current = next;
                Ok(())
            },
            Result::is_ok,
        )
    };
    match run() {
        Ok(()) => {
            *effect = Some(Effect::MetadataCommitted);
            response(true, "SetMetadataConfiguration", "")
        }
        Err(f) => f,
    }
}
