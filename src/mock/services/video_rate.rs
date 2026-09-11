//! Rate-only migration boundary. Other encoder fields remain VE1 work.
use crate::mock::{
    fault::{Code, Fault, INVALID_ARGS, MOCK_REQUEST_POLICY},
    request::{Node, RequestError},
    state::VideoEncoderState,
};

const TT: &str = "http://www.onvif.org/ver10/schema";

pub(super) fn candidate(operation: &Node, media2: bool) -> Result<Option<(f32, u32)>, String> {
    let ns = if media2 {
        "http://www.onvif.org/ver20/media/wsdl"
    } else {
        "http://www.onvif.org/ver10/media/wsdl"
    };
    let config = operation
        .child(ns, "Configuration")
        .map_err(RequestError::to_fault)?;
    let Some(config) = config else {
        return Ok(None);
    };
    // Do not let a wrong-namespace RateControl masquerade as omission.
    if config
        .element_children()
        .any(|(ns, name, _)| name == "RateControl" && ns != TT)
    {
        return Err(RequestError::OperationIdentity.to_fault());
    }
    let Some(rate) = config
        .child(TT, "RateControl")
        .map_err(RequestError::to_fault)?
    else {
        return Ok(None);
    };
    rate.check_child_sequence(
        TT,
        if media2 {
            &["FrameRateLimit", "BitrateLimit"]
        } else {
            &["FrameRateLimit", "EncodingInterval", "BitrateLimit"]
        },
    )
    .map_err(RequestError::to_fault)?;
    let text = rate
        .required_child_text(TT, "FrameRateLimit")
        .map_err(RequestError::to_fault)?;
    let value = text.trim_matches([' ', '\t', '\r', '\n']);
    let fps = if media2 {
        value
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 0.0)
    } else {
        // A Media1 integer must remain exact in the shared float storage.
        value.parse::<i32>().ok().filter(|v| *v >= 0).and_then(|v| {
            let fps = v as f32;
            (f64::from(fps) == f64::from(v)).then_some(fps)
        })
    }
    .ok_or_else(|| {
        Fault::new(
            Code::Sender,
            &[INVALID_ARGS],
            "Invalid encoder FrameRateLimit",
        )
        .to_xml()
    })?;
    let bitrate = rate
        .required_child_text(TT, "BitrateLimit")
        .map_err(RequestError::to_fault)?
        .trim_matches([' ', '\t', '\r', '\n'])
        .parse::<u32>()
        .map_err(|_| {
            Fault::new(
                Code::Sender,
                &[INVALID_ARGS],
                "Invalid encoder BitrateLimit",
            )
            .to_xml()
        })?;
    Ok(Some((fps, bitrate)))
}

pub(super) fn view(encoder: &VideoEncoderState, media2: bool) -> Result<(), String> {
    let rate = encoder.frame_rate_limit;
    if !rate.is_finite() || rate < 0.0 {
        return Err(Fault::new(
            Code::Receiver,
            &[MOCK_REQUEST_POLICY],
            "Invalid mock encoder frame rate",
        )
        .to_xml());
    }
    if !media2 && (rate.fract() != 0.0 || f64::from(rate) > f64::from(i32::MAX)) {
        return Err(Fault::new(
            Code::Receiver,
            &[MOCK_REQUEST_POLICY],
            "Encoder frame rate cannot be represented by Media1; use Media2",
        )
        .to_xml());
    }
    Ok(())
}
