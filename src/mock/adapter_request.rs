//! Private projection onto the existing typed adapter API, not a service validator.

use super::request::{Node, Request};

pub(crate) enum AdapterRequest {
    Identity,
    Stream(String),
    Move(String, [f32; 3]),
}

impl AdapterRequest {
    /// Decline malformed, misrouted or unrepresentable typed calls. The caller
    /// still owns raw fallback; never impose synthetic Fault policy on it.
    pub(crate) fn parse(action: &str, body: &str) -> Option<Self> {
        match action {
            "http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation"
            | "http://www.onvif.org/ver10/media/wsdl/GetStreamUri"
            | "http://www.onvif.org/ver20/media/wsdl/GetStreamUri"
            | "http://www.onvif.org/ver20/ptz/wsdl/ContinuousMove" => {}
            _ => return None,
        }
        let (ns, name) = action.rsplit_once('/')?;
        let request = Request::parse(body).ok()?;
        let operation = request.operation(ns, name).ok()?;
        if name == "GetDeviceInformation" {
            return Some(Self::Identity);
        }
        let profile = operation
            .required_child_text(ns, "ProfileToken")
            .ok()?
            .to_owned();
        if name == "GetStreamUri" {
            return Some(Self::Stream(profile));
        }
        // PtzVector has no optional axes, spaces or timeout. Do not silently
        // replace omitted/invalid values with zeros or discard explicit options.
        if operation.child(ns, "Timeout").ok()?.is_some() {
            return None;
        }
        let velocity = operation.child(ns, "Velocity").ok()??;
        let tt = "http://www.onvif.org/ver10/schema";
        let pan_tilt = velocity.child(tt, "PanTilt").ok()??;
        let zoom = velocity.child(tt, "Zoom").ok()??;
        if pan_tilt.attribute("", "space").is_some() || zoom.attribute("", "space").is_some() {
            return None;
        }
        Some(Self::Move(
            profile,
            [
                coordinate(pan_tilt, "x")?,
                coordinate(pan_tilt, "y")?,
                coordinate(zoom, "x")?,
            ],
        ))
    }
}

fn coordinate(node: &Node, name: &str) -> Option<f32> {
    if !node.scalar_text().ok()?.trim().is_empty() {
        return None;
    }
    let value = node.attribute("", name)?.trim().parse::<f32>().ok()?;
    value.is_finite().then_some(value)
}
