use super::xml_str;
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── PtzPreset ─────────────────────────────────────────────────────────────────

/// A named PTZ preset position returned by `GetPresets`.
#[derive(Debug, Clone)]
pub struct PtzPreset {
    /// Opaque preset identifier; pass to `ptz_goto_preset`.
    pub token: String,
    /// Human-readable preset name.
    pub name: String,
    /// Stored pan (x) and tilt (y) position, range `[-1.0, 1.0]`.
    /// `None` if the preset has no stored position.
    pub pan_tilt: Option<(f32, f32)>,
    /// Stored zoom position, range `[0.0, 1.0]`.
    /// `None` if the preset has no stored zoom.
    pub zoom: Option<f32>,
}

impl PtzPreset {
    /// Parse all `<Preset>` children from a `GetPresetsResponse` node.
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        Ok(resp
            .children_named("Preset")
            .map(|p| Self {
                token: p.attr("token").unwrap_or("").to_string(),
                name: xml_str(p, "Name").unwrap_or_default(),
                pan_tilt: p.path(&["PTZPosition", "PanTilt"]).and_then(|n| {
                    let x = n.attr("x")?.parse().ok()?;
                    let y = n.attr("y")?.parse().ok()?;
                    Some((x, y))
                }),
                zoom: p
                    .path(&["PTZPosition", "Zoom"])
                    .and_then(|n| n.attr("x")?.parse().ok()),
            })
            .collect())
    }
}

// ── PtzStatus ─────────────────────────────────────────────────────────────────

/// Current PTZ position and movement state returned by `GetStatus`.
#[derive(Debug, Clone)]
pub struct PtzStatus {
    /// Current pan position in the normalised range `[-1.0, 1.0]`.
    /// `None` if the device did not report a position.
    pub pan: Option<f32>,
    /// Current tilt position in the normalised range `[-1.0, 1.0]`.
    /// `None` if the device did not report a position.
    pub tilt: Option<f32>,
    /// Current zoom position in the normalised range `[0.0, 1.0]`.
    /// `None` if the device did not report a position.
    pub zoom: Option<f32>,
    /// Pan/tilt movement state (e.g. `"IDLE"`, `"MOVING"`, `"UNKNOWN"`).
    pub pan_tilt_status: String,
    /// Zoom movement state (e.g. `"IDLE"`, `"MOVING"`, `"UNKNOWN"`).
    pub zoom_status: String,
}

impl PtzStatus {
    /// Parse from a `GetStatusResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let status = resp
            .child("PTZStatus")
            .ok_or_else(|| SoapError::missing("PTZStatus"))?;

        let (pan, tilt) = status
            .path(&["Position", "PanTilt"])
            .and_then(|n| {
                let x = n.attr("x")?.parse().ok()?;
                let y = n.attr("y")?.parse().ok()?;
                Some((Some(x), Some(y)))
            })
            .unwrap_or((None, None));

        let zoom = status
            .path(&["Position", "Zoom"])
            .and_then(|n| n.attr("x")?.parse().ok());

        Ok(Self {
            pan,
            tilt,
            zoom,
            pan_tilt_status: status
                .path(&["MoveStatus", "PanTilt"])
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
            zoom_status: status
                .path(&["MoveStatus", "Zoom"])
                .map(|n| n.text().to_string())
                .unwrap_or_default(),
        })
    }
}
