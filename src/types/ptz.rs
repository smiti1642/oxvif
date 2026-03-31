use super::xml_str;
use crate::error::OnvifError;
use crate::soap::XmlNode;

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
