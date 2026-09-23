//! Bounded synthetic OSD candidates and atomic source-associated mutations.

use crate::mock::{
    effect::Effect,
    fault::{
        ACTION, CONFIG_MODIFY, Code, Fault, INVALID_ARG_VAL, MAX_OSDS, MOCK_REQUEST_POLICY,
        MOCK_UNMODELED_EFFECT, NO_CONFIG,
    },
    helpers::soap,
    request::{Node, RequestError},
    state::{
        DeviceState, OSD_QUOTA_DATE, OSD_QUOTA_DATE_AND_TIME, OSD_QUOTA_PLAIN, OSD_QUOTA_TIME,
        OSD_QUOTA_TOTAL, OsdColorEntry, OsdEntry, OsdTextEntry, SharedState,
    },
};

const M: &str = "http://www.onvif.org/ver10/media/wsdl";
const T: &str = "http://www.onvif.org/ver10/schema";

fn response(op: &str, body: &str) -> String {
    soap(
        &format!("xmlns:trt=\"{M}\""),
        &format!("<trt:{op}Response>{body}</trt:{op}Response>"),
    )
}
fn invalid(field: &str) -> String {
    Fault::new(
        Code::Sender,
        &[INVALID_ARG_VAL, CONFIG_MODIFY],
        &format!("Invalid mock OSD setting: {field}"),
    )
    .to_xml()
}
fn missing(token: &str) -> String {
    Fault::new(
        Code::Sender,
        &[INVALID_ARG_VAL, NO_CONFIG],
        &format!("OSD configuration not found: {token}"),
    )
    .to_xml()
}
fn snapshot() -> String {
    Fault::new(
        Code::Receiver,
        &[MOCK_REQUEST_POLICY],
        "Invalid mock OSD snapshot",
    )
    .to_xml()
}
fn unsupported() -> String {
    Fault::new(
        Code::Sender,
        &[MOCK_UNMODELED_EFFECT],
        "Unmodeled OSD setting",
    )
    .to_xml()
}
fn shape(node: &Node, ns: &str, fields: &[&str], attrs: &[&str]) -> Result<(), String> {
    if node
        .element_children()
        .any(|(uri, name, _)| uri != ns || !fields.contains(&name))
        || node
            .expanded_attributes()
            .any(|(uri, name, _)| !uri.is_empty() || !attrs.contains(&name))
    {
        return Err(unsupported());
    }
    node.check_child_sequence(ns, fields)
        .map_err(RequestError::to_fault)
}
fn child<'a>(node: &'a Node, ns: &str, name: &str) -> Result<&'a Node, String> {
    node.child(ns, name)
        .map_err(RequestError::to_fault)?
        .ok_or_else(|| RequestError::MissingField.to_fault())
}
fn optional(node: &Node, ns: &str, name: &str) -> Result<Option<String>, String> {
    node.child(ns, name)
        .map_err(RequestError::to_fault)?
        .map(|n| {
            if n.expanded_attributes().next().is_some() {
                return Err(unsupported());
            }
            n.scalar_text()
                .map(str::to_owned)
                .map_err(RequestError::to_fault)
        })
        .transpose()
}
fn text(node: &Node, ns: &str, name: &str) -> Result<String, String> {
    optional(node, ns, name)?.ok_or_else(|| RequestError::MissingField.to_fault())
}
fn identity(value: &str) -> Result<(), String> {
    if value.is_empty() || value.chars().count() > 64 {
        return Err(RequestError::EmptyField.to_fault());
    }
    Ok(())
}
fn number(value: &str, field: &str) -> Result<f32, String> {
    let value: f32 = value.trim().parse().map_err(|_| invalid(field))?;
    if !value.is_finite() {
        return Err(invalid(field));
    }
    Ok(value)
}
fn source(state: &DeviceState, token: &str) -> Result<(), String> {
    match state
        .video_source_configs
        .iter()
        .filter(|c| c.token == token)
        .count()
    {
        1 => Ok(()),
        0 => Err(missing(token)),
        _ => Err(snapshot()),
    }
}
fn selector(operation: &Node, field: &str, required: bool) -> Result<Option<String>, String> {
    shape(operation, M, &[field], &[])?;
    let token = optional(operation, M, field)?;
    if required && token.is_none() {
        return Err(RequestError::MissingField.to_fault());
    }
    if let Some(token) = &token {
        identity(token)?;
    }
    Ok(token)
}

pub(crate) fn list(state: &SharedState, operation: &Node) -> String {
    let run = || {
        let token = selector(operation, "ConfigurationToken", false)?;
        let s = state.read();
        if let Some(token) = &token {
            source(&s, token)?;
        }
        let body = s
            .osd
            .osds
            .iter()
            .filter(|o| {
                token
                    .as_ref()
                    .is_none_or(|t| *t == o.video_source_config_token)
            })
            .map(super::media::render_osd_entry)
            .collect::<String>();
        Ok(response("GetOSDs", &body))
    };
    run().unwrap_or_else(|e: String| e)
}
pub(crate) fn options(state: &SharedState, operation: &Node) -> String {
    let run = || {
        let token = selector(operation, "ConfigurationToken", true)?
            .ok_or_else(|| RequestError::MissingField.to_fault())?;
        source(&state.read(), &token)?;
        Ok(super::media::resp_osd_options())
    };
    run().unwrap_or_else(|e: String| e)
}

fn color(node: &Node) -> Result<OsdColorEntry, String> {
    shape(node, T, &["Color"], &["Transparent"])?;
    let c = child(node, T, "Color")?;
    shape(c, T, &[], &["X", "Y", "Z", "Colorspace"])?;
    let channel = |name| {
        number(
            c.attribute("", name)
                .ok_or_else(|| RequestError::MissingField.to_fault())?,
            name,
        )
    };
    let transparent = node
        .attribute("", "Transparent")
        .map(|v| {
            // The existing public field is f32; accept only the synthetic
            // integer levels we can represent exactly, with integer lexical form.
            let n: u8 = v.trim().parse().map_err(|_| invalid("Transparent"))?;
            Ok::<f32, String>(f32::from(n))
        })
        .transpose()?;
    Ok(OsdColorEntry {
        x: channel("X")?,
        y: channel("Y")?,
        z: channel("Z")?,
        colorspace: c.attribute("", "Colorspace").map(str::to_owned),
        transparent,
    })
}

fn candidate(operation: &Node, set: bool) -> Result<OsdEntry, String> {
    shape(operation, M, &["OSD"], &[])?;
    let o = child(operation, M, "OSD")?;
    shape(
        o,
        T,
        &[
            "VideoSourceConfigurationToken",
            "Type",
            "Position",
            "TextString",
            "Image",
        ],
        &["token"],
    )?;
    let token = o.attribute("", "token").unwrap_or_default().to_owned();
    if set {
        identity(&token)?;
    }
    let vsc = text(o, T, "VideoSourceConfigurationToken")?;
    identity(&vsc)?;
    let ty = text(o, T, "Type")?;
    let position = child(o, T, "Position")?;
    shape(position, T, &["Type", "Pos"], &[])?;
    let position_type = text(position, T, "Type")?;
    if ![
        "UpperLeft",
        "UpperRight",
        "LowerLeft",
        "LowerRight",
        "Custom",
    ]
    .contains(&position_type.as_str())
    {
        return Err(invalid("Position/Type"));
    }
    let pos = position.child(T, "Pos").map_err(RequestError::to_fault)?;
    let (position_x, position_y) = if let Some(pos) = pos {
        shape(pos, T, &[], &["x", "y"])?;
        let coord = |name| {
            let n = number(
                pos.attribute("", name)
                    .ok_or_else(|| RequestError::MissingField.to_fault())?,
                name,
            )?;
            if !(-1.0..=1.0).contains(&n) {
                return Err(invalid("Position/Pos"));
            }
            Ok(n)
        };
        (Some(coord("x")?), Some(coord("y")?))
    } else {
        (None, None)
    };
    if position_type == "Custom" && pos.is_none() {
        return Err(invalid("Position/Pos"));
    }
    let ts = o.child(T, "TextString").map_err(RequestError::to_fault)?;
    let img = o.child(T, "Image").map_err(RequestError::to_fault)?;
    let (text, image_path) = match (ty.as_str(), ts, img) {
        ("Text", Some(ts), None) => {
            shape(
                ts,
                T,
                &[
                    "Type",
                    "DateFormat",
                    "TimeFormat",
                    "FontSize",
                    "FontColor",
                    "PlainText",
                ],
                &["IsPersistentText"],
            )?;
            match ts.attribute("", "IsPersistentText").map(str::trim) {
                None | Some("true" | "1") => {}
                Some("false" | "0") => return Err(unsupported()),
                _ => return Err(invalid("IsPersistentText")),
            }
            let text_type = text(ts, T, "Type")?;
            if !["Plain", "Date", "Time", "DateAndTime"].contains(&text_type.as_str()) {
                return Err(invalid("TextString/Type"));
            }
            let font_size = optional(ts, T, "FontSize")?
                .map(|s| {
                    let n: u32 = s.trim().parse().map_err(|_| invalid("FontSize"))?;
                    if !(8..=72).contains(&n) {
                        return Err(invalid("FontSize"));
                    }
                    Ok(n)
                })
                .transpose()?;
            let date_format = optional(ts, T, "DateFormat")?;
            let time_format = optional(ts, T, "TimeFormat")?;
            if date_format
                .as_ref()
                .is_some_and(|s| !["MM/dd/yyyy", "yyyy-MM-dd", "dd.MM.yyyy"].contains(&s.as_str()))
            {
                return Err(invalid("DateFormat"));
            }
            if time_format
                .as_ref()
                .is_some_and(|s| !["HH:mm:ss", "hh:mm:ss tt"].contains(&s.as_str()))
            {
                return Err(invalid("TimeFormat"));
            }
            let date_format = date_format.or_else(|| {
                matches!(text_type.as_str(), "Date" | "DateAndTime").then(|| "yyyy-MM-dd".into())
            });
            let time_format = time_format.or_else(|| {
                matches!(text_type.as_str(), "Time" | "DateAndTime").then(|| "HH:mm:ss".into())
            });
            (
                Some(OsdTextEntry {
                    text_type,
                    plain_text: optional(ts, T, "PlainText")?,
                    date_format,
                    time_format,
                    font_size,
                    font_color: ts
                        .child(T, "FontColor")
                        .map_err(RequestError::to_fault)?
                        .map(color)
                        .transpose()?,
                }),
                None,
            )
        }
        ("Image", None, Some(img)) => {
            shape(img, T, &["ImgPath"], &[])?;
            let path = text(img, T, "ImgPath")?;
            if path.is_empty() {
                return Err(invalid("Image/ImgPath"));
            }
            (None, Some(path))
        }
        _ => return Err(invalid("Type/payload")),
    };
    Ok(OsdEntry {
        token,
        video_source_config_token: vsc,
        osd_type: ty,
        position_type,
        position_x,
        position_y,
        text,
        image_path,
    })
}

fn capacity(
    s: &DeviceState,
    entry: &OsdEntry,
    exclude: Option<usize>,
    create: bool,
) -> Result<(), String> {
    let others = || {
        s.osd
            .osds
            .iter()
            .enumerate()
            .filter(|(i, o)| {
                Some(*i) != exclude
                    && o.video_source_config_token == entry.video_source_config_token
            })
            .map(|(_, o)| o)
    };
    let text_full = entry.text.as_ref().is_some_and(|text| {
        let limit = match text.text_type.as_str() {
            "Plain" => OSD_QUOTA_PLAIN,
            "Date" => OSD_QUOTA_DATE,
            "Time" => OSD_QUOTA_TIME,
            _ => OSD_QUOTA_DATE_AND_TIME,
        };
        others()
            .filter(|o| {
                o.text
                    .as_ref()
                    .is_some_and(|t| t.text_type == text.text_type)
            })
            .count()
            >= limit as usize
    });
    if others().count() >= OSD_QUOTA_TOTAL as usize || text_full {
        return Err(if create {
            Fault::new(
                Code::Receiver,
                &[ACTION, MAX_OSDS],
                "OSD source capacity exceeded",
            )
            .to_xml()
        } else {
            invalid("source capacity")
        });
    }
    Ok(())
}
fn index(s: &DeviceState, token: &str) -> Result<usize, String> {
    let mut found = s
        .osd
        .osds
        .iter()
        .enumerate()
        .filter(|(_, o)| o.token == token);
    let first = found.next().map(|(i, _)| i).ok_or_else(|| missing(token))?;
    if found.next().is_some() {
        return Err(snapshot());
    }
    Ok(first)
}
pub(crate) fn create(state: &SharedState, operation: &Node, effect: &mut Option<Effect>) -> String {
    let mut run = || {
        let mut entry = candidate(operation, false)?;
        let token = state.modify_returning_if(
            |s| {
                source(s, &entry.video_source_config_token)?;
                capacity(s, &entry, None, true)?;
                let mut id = s.osd.next_token_id;
                // At most one collision per live entry. No counter changes until commit.
                for _ in 0..=s.osd.osds.len() {
                    let next = id.checked_add(1).ok_or_else(snapshot)?;
                    let token = format!("OSD_{id}");
                    if !s.osd.osds.iter().any(|o| o.token == token) {
                        entry.token = token.clone();
                        s.osd.osds.push(entry);
                        s.osd.next_token_id = next;
                        return Ok(token);
                    }
                    id = next;
                }
                Err(snapshot())
            },
            Result::is_ok,
        )?;
        *effect = Some(Effect::OsdCommitted);
        Ok(response(
            "CreateOSD",
            &format!("<trt:OSDToken>{token}</trt:OSDToken>"),
        ))
    };
    run().unwrap_or_else(|e: String| e)
}
pub(crate) fn set(state: &SharedState, operation: &Node, effect: &mut Option<Effect>) -> String {
    let mut run = || {
        let entry = candidate(operation, true)?;
        state.modify_returning_if(
            |s| {
                let i = index(s, &entry.token)?;
                source(s, &entry.video_source_config_token)?;
                if s.osd.osds[i].video_source_config_token != entry.video_source_config_token {
                    return Err(invalid("immutable source binding"));
                }
                capacity(s, &entry, Some(i), false)?;
                s.osd.osds[i] = entry;
                Ok(())
            },
            Result::is_ok,
        )?;
        *effect = Some(Effect::OsdCommitted);
        Ok(response("SetOSD", ""))
    };
    run().unwrap_or_else(|e: String| e)
}
pub(crate) fn delete(state: &SharedState, operation: &Node, effect: &mut Option<Effect>) -> String {
    let mut run = || {
        let token = selector(operation, "OSDToken", true)?
            .ok_or_else(|| RequestError::MissingField.to_fault())?;
        state.modify_returning_if(
            |s| -> Result<(), String> {
                let i = index(s, &token)?;
                s.osd.osds.remove(i);
                Ok(())
            },
            Result::is_ok,
        )?;
        *effect = Some(Effect::OsdCommitted);
        Ok(response("DeleteOSD", ""))
    };
    run().unwrap_or_else(|e: String| e)
}
