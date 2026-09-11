//! The modeled source configuration contract shared by both Media services.

use crate::mock::{
    effect::Effect,
    fault::{
        CONFIG_MODIFY, Code, Fault, INVALID_ARG_VAL, INVALID_ARGS, MOCK_REQUEST_POLICY,
        MOCK_UNMODELED_EFFECT, NO_CONFIG, NO_PROFILE,
    },
    helpers::soap,
    request::{Node, RequestError},
    state::{DeviceState, SharedState, VideoSourceConfigEntry},
};
use crate::types::xml_escape;

const TT: &str = "http://www.onvif.org/ver10/schema";
const M1: &str = "http://www.onvif.org/ver10/media/wsdl";
const M2: &str = "http://www.onvif.org/ver20/media/wsdl";

fn ns(media2: bool) -> &'static str {
    if media2 { M2 } else { M1 }
}
fn prefix(media2: bool) -> &'static str {
    if media2 { "tr2" } else { "trt" }
}
fn response(media2: bool, op: &str, fields: &str) -> String {
    let p = prefix(media2);
    soap(
        &format!("xmlns:{p}=\"{}\"", ns(media2)),
        &format!("<{p}:{op}Response>{fields}</{p}:{op}Response>"),
    )
}

fn invalid(path: &str) -> String {
    Fault::new(
        Code::Sender,
        &[INVALID_ARG_VAL, CONFIG_MODIFY],
        &format!("Invalid mock source setting: {path}"),
    )
    .to_xml()
}

fn unknown_config(token: &str) -> String {
    Fault::new(
        Code::Sender,
        &[INVALID_ARG_VAL, NO_CONFIG],
        &format!("Source configuration not found: {token}"),
    )
    .to_xml()
}

fn invalid_snapshot() -> String {
    Fault::new(
        Code::Receiver,
        &[MOCK_REQUEST_POLICY],
        "Invalid mock source snapshot",
    )
    .to_xml()
}

fn modeled(node: &Node, children: &[&str], attributes: &[&str]) -> Result<(), String> {
    if node
        .element_children()
        .any(|(ns, name, _)| ns != TT || !children.contains(&name))
        || node
            .expanded_attributes()
            .any(|(ns, name, _)| !ns.is_empty() || !attributes.contains(&name))
    {
        return Err(Fault::new(
            Code::Sender,
            &[MOCK_UNMODELED_EFFECT],
            "Unmodeled source configuration setting",
        )
        .to_xml());
    }
    node.check_child_sequence(TT, children)
        .map_err(RequestError::to_fault)
}

fn int(value: &str, path: &str) -> Result<i32, String> {
    value
        .trim_matches([' ', '\t', '\r', '\n'])
        .parse()
        .map_err(|_| invalid(path))
}

fn child<'a>(node: &'a Node, ns: &str, name: &str) -> Result<&'a Node, String> {
    node.child(ns, name)
        .map_err(RequestError::to_fault)?
        .ok_or_else(|| RequestError::MissingField.to_fault())
}

fn text<'a>(node: &'a Node, ns: &str, name: &str) -> Result<&'a str, String> {
    child(node, ns, name)?
        .scalar_text()
        .map_err(RequestError::to_fault)
}

fn identity(value: &str) -> Result<&str, String> {
    if value.is_empty() {
        return Err(RequestError::EmptyField.to_fault());
    }
    if value.chars().count() > 64 {
        return Err(Fault::new(Code::Sender, &[INVALID_ARGS], "Invalid Args").to_xml());
    }
    Ok(value)
}

pub(super) fn empty(operation: &Node, media2: bool) -> Result<(), String> {
    operation
        .check_child_sequence(ns(media2), &[])
        .map_err(RequestError::to_fault)
}

fn selectors(
    operation: &Node,
    media2: bool,
    singular: bool,
) -> Result<(Option<&str>, Option<&str>), String> {
    let ns = ns(media2);
    operation
        .check_child_sequence(
            ns,
            if singular {
                &["ConfigurationToken"]
            } else {
                &["ConfigurationToken", "ProfileToken"]
            },
        )
        .map_err(RequestError::to_fault)?;
    let config = operation
        .optional_child_text(ns, "ConfigurationToken")
        .map_err(RequestError::to_fault)?
        .map(identity)
        .transpose()?;
    let profile = operation
        .optional_child_text(ns, "ProfileToken")
        .map_err(RequestError::to_fault)?
        .map(identity)
        .transpose()?;
    if singular && config.is_none() {
        return Err(RequestError::MissingField.to_fault());
    }
    Ok((config, profile))
}

fn profile_exists(state: &DeviceState, token: Option<&str>) -> Result<(), String> {
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

/// All modeled source configurations can be reassigned to every profile. This
/// does not model physical encoder-routing conflicts or stream availability.
pub(super) fn get(state: &SharedState, operation: &Node, media2: bool) -> String {
    let result = || -> Result<String, String> {
        let (want, profile) = selectors(operation, media2, !media2)?;
        let state = state.read();
        profile_exists(&state, profile)?;
        if let Some(want) = want
            && !state.video_source_configs.iter().any(|c| c.token == want)
        {
            return Err(unknown_config(want));
        }
        let tag = format!(
            "{}:{}",
            prefix(media2),
            if media2 {
                "Configurations"
            } else {
                "Configuration"
            }
        );
        let body = state
            .video_source_configs
            .iter()
            .filter(|c| want.is_none_or(|t| t == c.token))
            .map(|c| super::media::render_vsc_body(c, &tag))
            .collect::<String>();
        Ok(response(
            media2,
            if media2 {
                "GetVideoSourceConfigurations"
            } else {
                "GetVideoSourceConfiguration"
            },
            &body,
        ))
    };
    result().unwrap_or_else(|fault| fault)
}

pub(super) fn options(state: &SharedState, operation: &Node, media2: bool) -> String {
    let result = || -> Result<String, String> {
        let (want, profile) = selectors(operation, media2, false)?;
        let state = state.read();
        profile_exists(&state, profile)?;
        let source = want
            .map(|want| {
                state
                    .video_source_configs
                    .iter()
                    .find(|c| c.token == want)
                    .map(|c| c.source_token.as_str())
                    .ok_or_else(|| unknown_config(want))
            })
            .transpose()?;
        let sensors: Vec<_> = state
            .video_sources
            .iter()
            .filter(|s| source.is_none_or(|token| token == s.token))
            .collect();
        let width = sensors
            .iter()
            .map(|s| s.width)
            .min()
            .filter(|v| *v > 0)
            .ok_or_else(invalid_snapshot)?;
        let height = sensors
            .iter()
            .map(|s| s.height)
            .min()
            .filter(|v| *v > 0)
            .ok_or_else(invalid_snapshot)?;
        let tokens = sensors
            .iter()
            .map(|s| {
                format!(
                    "<tt:VideoSourceTokensAvailable>{}</tt:VideoSourceTokensAvailable>",
                    xml_escape(&s.token)
                )
            })
            .collect::<String>();
        let p = prefix(media2);
        let limit = super::media::PROFILE_LIMIT;
        Ok(response(
            media2,
            "GetVideoSourceConfigurationOptions",
            &format!(
                "<{p}:Options MaximumNumberOfProfiles=\"{limit}\"><tt:BoundsRange><tt:XRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:XRange><tt:YRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:YRange><tt:WidthRange><tt:Min>1</tt:Min><tt:Max>{width}</tt:Max></tt:WidthRange><tt:HeightRange><tt:Min>1</tt:Min><tt:Max>{height}</tt:Max></tt:HeightRange></tt:BoundsRange>{tokens}</{p}:Options>"
            ),
        ))
    };
    result().unwrap_or_else(|fault| fault)
}

struct Candidate<'a> {
    token: &'a str,
    name: &'a str,
    source: &'a str,
    width: u32,
    height: u32,
}

fn candidate(operation: &Node, media2: bool) -> Result<Candidate<'_>, String> {
    let ns = ns(media2);
    operation
        .check_child_sequence(
            ns,
            if media2 {
                &["Configuration"]
            } else {
                &["Configuration", "ForcePersistence"]
            },
        )
        .map_err(RequestError::to_fault)?;
    if !media2 {
        let value = text(operation, ns, "ForcePersistence")?;
        if !matches!(
            value.trim_matches([' ', '\t', '\r', '\n']),
            "true" | "false" | "1" | "0"
        ) {
            return Err(invalid("ForcePersistence"));
        }
    }
    let config = child(operation, ns, "Configuration")?;
    modeled(
        config,
        &["Name", "UseCount", "SourceToken", "Bounds", "Extension"],
        &["token", "ViewMode"],
    )?;
    let token = identity(
        config
            .attribute("", "token")
            .ok_or_else(|| RequestError::MissingField.to_fault())?,
    )?;
    let name = text(config, TT, "Name")?;
    if name.chars().count() > 64 {
        return Err(invalid("Name"));
    }
    int(text(config, TT, "UseCount")?, "UseCount")?;
    let source = identity(text(config, TT, "SourceToken")?)?;
    let bounds = child(config, TT, "Bounds")?;
    modeled(bounds, &[], &["x", "y", "width", "height"])?;
    let bound = |field: &str| -> Result<i32, String> {
        int(
            bounds
                .attribute("", field)
                .ok_or_else(|| RequestError::MissingField.to_fault())?,
            &format!("Bounds/@{field}"),
        )
    };
    if bound("x")? != 0 {
        return Err(invalid("Bounds/@x (only zero origin is modeled)"));
    }
    if bound("y")? != 0 {
        return Err(invalid("Bounds/@y (only zero origin is modeled)"));
    }
    let width = u32::try_from(bound("width")?)
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| invalid("Bounds/@width"))?;
    let height = u32::try_from(bound("height")?)
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| invalid("Bounds/@height"))?;
    if let Some(extension) = config
        .child(TT, "Extension")
        .map_err(RequestError::to_fault)?
    {
        modeled(extension, &[], &[])?;
    }
    Ok(Candidate {
        token,
        name,
        source,
        width,
        height,
    })
}

pub(super) fn set(
    state: &SharedState,
    operation: &Node,
    media2: bool,
    effect: &mut Option<Effect>,
) -> String {
    let candidate = match candidate(operation, media2) {
        Ok(c) => c,
        Err(fault) => return fault,
    };
    let committed = state.modify_returning_if(
        |state| -> Result<(), String> {
            let index = state
                .video_source_configs
                .iter()
                .position(|c| c.token == candidate.token)
                .ok_or_else(|| unknown_config(candidate.token))?;
            let sensor = state
                .video_sources
                .iter()
                .find(|s| s.token == candidate.source)
                .ok_or_else(|| invalid("SourceToken"))?;
            if sensor.width == 0 || sensor.height == 0 {
                return Err(invalid_snapshot());
            }
            let updated = VideoSourceConfigEntry {
                token: candidate.token.to_owned(),
                name: candidate.name.to_owned(),
                source_token: candidate.source.to_owned(),
                use_count: state.video_source_configs[index].use_count,
                width: candidate.width.min(sensor.width),
                height: candidate.height.min(sensor.height),
            };
            state.video_source_configs[index] = updated;
            Ok(())
        },
        Result::is_ok,
    );
    match committed {
        Ok(()) => {
            *effect = Some(Effect::VideoSourceChanged);
            response(media2, "SetVideoSourceConfiguration", "")
        }
        Err(fault) => fault,
    }
}
