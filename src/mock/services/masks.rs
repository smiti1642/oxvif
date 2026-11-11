//! Bounded per-instance Media2 privacy masks.

use crate::{
    mock::{
        effect::Effect,
        fault::{CONFIG_MODIFY, Code, Fault, INVALID_ARG_VAL, NO_CONFIG},
        helpers::soap,
        request::{Node, RequestError},
        state::SharedState,
    },
    overlays::{
        Mask, MaskColorOptions, MaskColorRange, MaskKind, MaskOptions, MaskSettings, mask_body,
        parse_mask_settings, validate_mask,
    },
    scoped_rpc::{MEDIA2, required, text},
    soap::qualified::{self, SOAP},
};

use super::osd::{identity, optional, shape, source};

const MAX_MASKS: usize = 4;
const MAX_POINTS: usize = 8;
const MAX_TOTAL_MASKS: usize = 256;
const COLORSPACE: &str = "http://www.onvif.org/ver10/colorspace/YCbCr";

#[derive(Default)]
pub(crate) struct MaskState {
    items: Vec<Mask>,
    next: u64,
}

pub(crate) fn handle(
    op: &str,
    state: &SharedState,
    body: &str,
    operation: &Node,
    effect: &mut Option<Effect>,
) -> String {
    let result = match op {
        "GetMasks" => list(state, operation),
        "GetMaskOptions" => options(state, operation),
        "CreateMask" | "SetMask" => change(state, body, op == "CreateMask"),
        "DeleteMask" => delete(state, operation),
        _ => unreachable!("fixed mask operation"),
    };
    match result {
        Ok(fields) => {
            if matches!(op, "CreateMask" | "SetMask" | "DeleteMask") {
                *effect = Some(Effect::MaskCommitted);
            }
            soap(
                "",
                &format!("<{op}Response xmlns=\"{MEDIA2}\">{fields}</{op}Response>"),
            )
        }
        Err(fault) => fault,
    }
}

fn list(state: &SharedState, operation: &Node) -> Result<String, String> {
    shape(operation, MEDIA2, &["Token", "ConfigurationToken"], &[])?;
    let config = optional(operation, MEDIA2, "ConfigurationToken")?;
    let token = optional(operation, MEDIA2, "Token")?;
    let configured = state.read();
    if let Some(config) = &config {
        identity(config)?;
        source(&configured, config)?;
    }
    let state = state.masks.lock().unwrap();
    let entries = state.items.iter().filter(|item| {
        config
            .as_ref()
            .is_none_or(|config| item.configuration_token == *config)
            && token.as_ref().is_none_or(|token| item.token == *token)
    });
    let payload = entries
        .map(|item| {
            mask_body(
                "Masks",
                Some(&item.token),
                &item.configuration_token,
                &item.settings,
            )
        })
        .collect::<String>();
    if token.is_some() && payload.is_empty() {
        return Err(missing());
    }
    Ok(payload)
}

fn options(state: &SharedState, operation: &Node) -> Result<String, String> {
    shape(operation, MEDIA2, &["ConfigurationToken"], &[])?;
    let config = optional(operation, MEDIA2, "ConfigurationToken")?
        .ok_or_else(|| RequestError::MissingField.to_fault())?;
    identity(&config)?;
    source(&state.read(), &config)?;
    let channel =
        |name: &str| format!("<tt:{name}><tt:Min>0</tt:Min><tt:Max>255</tt:Max></tt:{name}>");
    Ok(format!(
        "<Options RectangleOnly=\"false\" SingleColorOnly=\"false\">\
        <MaxMasks>{MAX_MASKS}</MaxMasks><MaxPoints>{MAX_POINTS}</MaxPoints>\
        <Types>Color</Types><Types>Pixelated</Types><Types>Blurred</Types>\
        <Color><tt:ColorspaceRange>{}{}{}<tt:Colorspace>{COLORSPACE}</tt:Colorspace></tt:ColorspaceRange></Color></Options>",
        channel("X"),
        channel("Y"),
        channel("Z")
    ))
}

fn supported() -> MaskOptions {
    MaskOptions {
        max_masks: MAX_MASKS as u32,
        max_points: MAX_POINTS as u32,
        kinds: vec![MaskKind::Color, MaskKind::Pixelated, MaskKind::Blurred],
        colors: MaskColorOptions::Ranges(vec![MaskColorRange {
            channels: [(0.0, 255.0); 3],
            colorspace: COLORSPACE.into(),
        }]),
        rectangle_only: Some(false),
        single_color_only: Some(false),
    }
}

fn candidate(body: &str, create: bool) -> Result<(String, String, MaskSettings), String> {
    let parse = || {
        let root = qualified::parse(body)?;
        let node = required(&root, SOAP, "Body")?;
        let operation = required(node, MEDIA2, if create { "CreateMask" } else { "SetMask" })?;
        crate::overlays::shape(operation, MEDIA2, &["Mask"], &[])?;
        let node = required(operation, MEDIA2, "Mask")?;
        let config = text(node, MEDIA2, "ConfigurationToken")?;
        let token = node.attr("", "token").unwrap_or_default().to_owned();
        Ok::<_, crate::soap::SoapError>((config, token, parse_mask_settings(node)?))
    };
    let (config, token, settings) = parse().map_err(|_| invalid("invalid mask request"))?;
    identity(&config)?;
    if !create {
        identity(&token)?;
    }
    validate_mask(&settings, &supported()).map_err(|_| invalid("unsupported mask settings"))?;
    Ok((config, token, settings))
}

fn change(state: &SharedState, body: &str, create: bool) -> Result<String, String> {
    let (config, token, settings) = candidate(body, create)?;
    let configured = state.read();
    source(&configured, &config)?;
    let mut masks = state.masks.lock().unwrap();
    if create {
        if masks.items.len() >= MAX_TOTAL_MASKS
            || masks
                .items
                .iter()
                .filter(|item| item.configuration_token == config)
                .count()
                >= MAX_MASKS
        {
            return Err(invalid("mask quota exceeded"));
        }
        let next = masks
            .next
            .checked_add(1)
            .ok_or_else(|| invalid("mask token limit"))?;
        let token = format!("MASK_{next}");
        masks.items.push(Mask {
            token: token.clone(),
            configuration_token: config,
            settings,
        });
        masks.next = next;
        Ok(format!("<Token>{token}</Token>"))
    } else {
        let mask = masks
            .items
            .iter_mut()
            .find(|item| item.token == token && item.configuration_token == config)
            .ok_or_else(missing)?;
        mask.settings = settings;
        Ok(String::new())
    }
}

fn delete(state: &SharedState, operation: &Node) -> Result<String, String> {
    shape(operation, MEDIA2, &["Token"], &[])?;
    let token = optional(operation, MEDIA2, "Token")?
        .ok_or_else(|| RequestError::MissingField.to_fault())?;
    identity(&token)?;
    let mut masks = state.masks.lock().unwrap();
    let index = masks
        .items
        .iter()
        .position(|item| item.token == token)
        .ok_or_else(missing)?;
    masks.items.remove(index);
    Ok(String::new())
}

fn invalid(reason: &str) -> String {
    Fault::new(Code::Sender, &[INVALID_ARG_VAL, CONFIG_MODIFY], reason).to_xml()
}

fn missing() -> String {
    Fault::new(
        Code::Sender,
        &[INVALID_ARG_VAL, NO_CONFIG],
        "unknown mask token",
    )
    .to_xml()
}
