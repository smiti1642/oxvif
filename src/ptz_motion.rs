//! Typed PTZ movement and validation against reported configuration options.

use std::{fmt::Write, time::Duration};

use crate::{
    OnvifClient, OnvifError, OnvifSession, PtzConfiguration, PtzConfigurationOptions,
    soap::{SoapError, XmlNode, find_response, parse_soap_body},
    types::xml_escape,
};

/// Pan and tilt coordinates or rates, in an optional explicit space.
#[derive(Debug, Clone)]
pub struct PanTiltVector {
    /// Pan coordinate or rate.
    pub x: f32,
    /// Tilt coordinate or rate.
    pub y: f32,
    /// Space URI. Omit to use the configuration default.
    pub space: Option<String>,
}

/// Zoom coordinate or rate, in an optional explicit space.
#[derive(Debug, Clone)]
pub struct ZoomVector {
    /// Zoom coordinate or rate.
    pub x: f32,
    /// Space URI. Omit to use the configuration default.
    pub space: Option<String>,
}

/// Optional PTZ axis groups. Omitted groups are absent on the wire.
#[derive(Debug, Clone, Default)]
pub struct PtzVector {
    /// Pan and tilt as a single ONVIF vector.
    pub pan_tilt: Option<PanTiltVector>,
    /// Zoom vector.
    pub zoom: Option<ZoomVector>,
}

/// Axis groups to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtzStopAxes {
    /// Stop pan/tilt and zoom.
    All,
    /// Stop pan and tilt.
    PanTilt,
    /// Stop zoom.
    Zoom,
}

/// One explicit PTZ movement request.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PtzMotion {
    /// Move to a position.
    Absolute {
        /// Requested position.
        position: PtzVector,
        /// Optional speed override.
        speed: Option<PtzVector>,
    },
    /// Move by a relative displacement.
    Relative {
        /// Requested displacement.
        translation: PtzVector,
        /// Optional speed override.
        speed: Option<PtzVector>,
    },
    /// Move at a velocity until timeout or explicit stop.
    Continuous {
        /// Requested velocity.
        velocity: PtzVector,
        /// Override the device's default motion timeout.
        timeout: Option<Duration>,
    },
    /// Explicitly stop selected axis groups.
    Stop(PtzStopAxes),
}

/// A reported coordinate space. Missing or invalid limits remain unknown.
#[derive(Debug, Clone)]
pub struct ReportedPtzSpace {
    /// Space URI.
    pub uri: String,
    /// Reported X bounds.
    pub x: Option<(f32, f32)>,
    /// Reported Y bounds, when meaningful.
    pub y: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotionKind {
    Absolute,
    Relative,
    Continuous,
    Speed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    PanTilt,
    Zoom,
}

const SPACE_NAMES: &[(MotionKind, Axis, &str)] = &[
    (
        MotionKind::Absolute,
        Axis::PanTilt,
        "AbsolutePanTiltPositionSpace",
    ),
    (
        MotionKind::Absolute,
        Axis::Zoom,
        "AbsoluteZoomPositionSpace",
    ),
    (
        MotionKind::Relative,
        Axis::PanTilt,
        "RelativePanTiltTranslationSpace",
    ),
    (
        MotionKind::Relative,
        Axis::Zoom,
        "RelativeZoomTranslationSpace",
    ),
    (
        MotionKind::Continuous,
        Axis::PanTilt,
        "ContinuousPanTiltVelocitySpace",
    ),
    (
        MotionKind::Continuous,
        Axis::Zoom,
        "ContinuousZoomVelocitySpace",
    ),
    (MotionKind::Speed, Axis::PanTilt, "PanTiltSpeedSpace"),
    (MotionKind::Speed, Axis::Zoom, "ZoomSpeedSpace"),
];

/// Device evidence for validating a particular PTZ configuration.
#[derive(Debug, Clone)]
pub struct PtzMotionCapabilities {
    configuration: PtzConfiguration,
    options: PtzConfigurationOptions,
    spaces: Option<Vec<(MotionKind, Axis, ReportedPtzSpace)>>,
}

impl PtzMotionCapabilities {
    /// Configuration defaults and its physical node identity.
    pub fn configuration(&self) -> &PtzConfiguration {
        &self.configuration
    }
    /// Reported operation timeout bounds in wire form.
    pub fn options(&self) -> &PtzConfigurationOptions {
        &self.options
    }
    /// All reported spaces, labelled by their ONVIF operation/axis element name.
    pub fn reported_spaces(
        &self,
    ) -> Option<impl Iterator<Item = (&'static str, &ReportedPtzSpace)>> {
        self.spaces.as_ref().map(|spaces| {
            spaces.iter().map(|(kind, axis, space)| {
                let name = SPACE_NAMES
                    .iter()
                    .find(|(k, a, _)| k == kind && a == axis)
                    .expect("known PTZ space group")
                    .2;
                (name, space)
            })
        })
    }

    fn default_space(&self, kind: MotionKind, axis: Axis) -> Option<&str> {
        let c = &self.configuration;
        match (kind, axis) {
            (MotionKind::Absolute, Axis::PanTilt) => c.default_abs_pan_tilt_space.as_deref(),
            (MotionKind::Absolute, Axis::Zoom) => c.default_abs_zoom_space.as_deref(),
            (MotionKind::Relative, Axis::PanTilt) => c.default_rel_pan_tilt_space.as_deref(),
            (MotionKind::Relative, Axis::Zoom) => c.default_rel_zoom_space.as_deref(),
            (MotionKind::Continuous, Axis::PanTilt) => c.default_cont_pan_tilt_space.as_deref(),
            (MotionKind::Continuous, Axis::Zoom) => c.default_cont_zoom_space.as_deref(),
            (MotionKind::Speed, _) => None,
        }
    }

    fn validate_axis(
        &self,
        kind: MotionKind,
        axis: Axis,
        x: f32,
        y: Option<f32>,
        uri: Option<&str>,
    ) -> Result<(), OnvifError> {
        let Some(spaces) = &self.spaces else {
            return Ok(());
        };
        let mut matching = spaces
            .iter()
            .filter(|(k, a, _)| *k == kind && *a == axis)
            .map(|(_, _, space)| space);
        let first = matching
            .next()
            .ok_or_else(|| invalid("PTZ operation does not support this axis"))?;
        let space = if let Some(uri) = uri.or_else(|| self.default_space(kind, axis)) {
            std::iter::once(first)
                .chain(matching)
                .find(|s| s.uri == uri)
                .ok_or_else(|| invalid("PTZ space was not reported for this axis"))?
        } else if matching.next().is_none() {
            first
        } else {
            return Ok(());
        };
        if space.x.is_some_and(|(min, max)| x < min || x > max)
            || y.zip(space.y)
                .is_some_and(|(y, (min, max))| y < min || y > max)
        {
            return Err(invalid("PTZ vector is outside reported space bounds"));
        }
        Ok(())
    }
}

fn invalid(message: &'static str) -> OnvifError {
    OnvifError::InvalidArgument(message.into())
}

fn bounds(node: &XmlNode, name: &str) -> Option<(f32, f32)> {
    let range = node.child(name)?;
    let min: f32 = range.child("Min")?.text().parse().ok()?;
    let max: f32 = range.child("Max")?.text().parse().ok()?;
    (min.is_finite() && max.is_finite() && min <= max).then_some((min, max))
}

fn reported_spaces(node: &XmlNode) -> Vec<(MotionKind, Axis, ReportedPtzSpace)> {
    let mut spaces = Vec::new();
    for &(kind, axis, name) in SPACE_NAMES {
        for item in node.children_named(name) {
            let uri = item
                .child("URI")
                .map(|n| n.text().to_owned())
                .unwrap_or_default();
            spaces.push((
                kind,
                axis,
                ReportedPtzSpace {
                    uri,
                    x: bounds(item, "XRange"),
                    y: bounds(item, "YRange"),
                },
            ));
        }
    }
    spaces
}

fn check_vector(
    vector: &PtzVector,
    kind: MotionKind,
    caps: Option<&PtzMotionCapabilities>,
) -> Result<(), OnvifError> {
    if vector.pan_tilt.is_none() && vector.zoom.is_none() {
        return Err(invalid("PTZ vector requires at least one axis group"));
    }
    let axes = [
        vector
            .pan_tilt
            .as_ref()
            .map(|v| (Axis::PanTilt, v.x, Some(v.y), v.space.as_deref())),
        vector
            .zoom
            .as_ref()
            .map(|v| (Axis::Zoom, v.x, None, v.space.as_deref())),
    ];
    for (axis, x, y, space) in axes.into_iter().flatten() {
        if !x.is_finite()
            || y.is_some_and(|v| !v.is_finite())
            || space.is_some_and(|uri| !valid_space_uri(uri))
        {
            return Err(invalid("invalid PTZ coordinate or space URI"));
        }
        if let Some(caps) = caps {
            caps.validate_axis(kind, axis, x, y, space)?;
        }
    }
    Ok(())
}

fn valid_space_uri(uri: &str) -> bool {
    !uri.chars().any(|ch| ch.is_whitespace() || ch.is_control()) && reqwest::Url::parse(uri).is_ok()
}

fn write_vector(out: &mut String, tag: &str, vector: &PtzVector) {
    write!(out, "<tptz:{tag}>").unwrap();
    if let Some(pan_tilt) = &vector.pan_tilt {
        write!(out, "<tt:PanTilt x=\"{}\" y=\"{}\"", pan_tilt.x, pan_tilt.y).unwrap();
        if let Some(space) = &pan_tilt.space {
            write!(out, " space=\"{}\"", xml_escape(space)).unwrap();
        }
        out.push_str("/>");
    }
    if let Some(zoom) = &vector.zoom {
        write!(out, "<tt:Zoom x=\"{}\"", zoom.x).unwrap();
        if let Some(space) = &zoom.space {
            write!(out, " space=\"{}\"", xml_escape(space)).unwrap();
        }
        out.push_str("/>");
    }
    write!(out, "</tptz:{tag}>").unwrap();
}

fn motion_body(
    motion: &PtzMotion,
    caps: Option<&PtzMotionCapabilities>,
) -> Result<(&'static str, &'static str, &'static str, String), OnvifError> {
    let mut payload = String::new();
    let (name, action, response) = match motion {
        PtzMotion::Absolute { position, speed } => {
            check_vector(position, MotionKind::Absolute, caps)?;
            write_vector(&mut payload, "Position", position);
            if let Some(speed) = speed {
                check_vector(speed, MotionKind::Speed, caps)?;
                write_vector(&mut payload, "Speed", speed);
            }
            (
                "AbsoluteMove",
                "http://www.onvif.org/ver20/ptz/wsdl/AbsoluteMove",
                "AbsoluteMoveResponse",
            )
        }
        PtzMotion::Relative { translation, speed } => {
            check_vector(translation, MotionKind::Relative, caps)?;
            write_vector(&mut payload, "Translation", translation);
            if let Some(speed) = speed {
                check_vector(speed, MotionKind::Speed, caps)?;
                write_vector(&mut payload, "Speed", speed);
            }
            (
                "RelativeMove",
                "http://www.onvif.org/ver20/ptz/wsdl/RelativeMove",
                "RelativeMoveResponse",
            )
        }
        PtzMotion::Continuous { velocity, timeout } => {
            check_vector(velocity, MotionKind::Continuous, caps)?;
            write_vector(&mut payload, "Velocity", velocity);
            if let Some(timeout) = timeout {
                if timeout.is_zero() {
                    return Err(invalid("PTZ motion timeout must be positive"));
                }
                if let Some(caps) = caps {
                    let min = caps
                        .options
                        .ptz_timeout_min
                        .as_deref()
                        .and_then(parse_duration);
                    let max = caps
                        .options
                        .ptz_timeout_max
                        .as_deref()
                        .and_then(parse_duration);
                    if min.is_some_and(|min| *timeout < min)
                        || max.is_some_and(|max| *timeout > max)
                    {
                        return Err(invalid("PTZ motion timeout is outside reported bounds"));
                    }
                }
                write!(
                    payload,
                    "<tptz:Timeout>{}</tptz:Timeout>",
                    crate::types::duration_xml(*timeout)
                )
                .unwrap();
            }
            (
                "ContinuousMove",
                "http://www.onvif.org/ver20/ptz/wsdl/ContinuousMove",
                "ContinuousMoveResponse",
            )
        }
        PtzMotion::Stop(axes) => {
            write!(
                payload,
                "<tptz:PanTilt>{}</tptz:PanTilt><tptz:Zoom>{}</tptz:Zoom>",
                *axes != PtzStopAxes::Zoom,
                *axes != PtzStopAxes::PanTilt
            )
            .unwrap();
            (
                "Stop",
                "http://www.onvif.org/ver20/ptz/wsdl/Stop",
                "StopResponse",
            )
        }
    };
    Ok((name, action, response, payload))
}

// Parse fixed-duration units, leaving calendar durations to device validation.
fn parse_duration(value: &str) -> Option<Duration> {
    let mut rest = value.strip_prefix('P')?;
    let mut duration = Duration::ZERO;
    let mut rank = 0;
    let mut time = false;
    while !rest.is_empty() {
        if !time && let Some(value) = rest.strip_prefix('T') {
            time = true;
            rest = value;
            if rest.is_empty() {
                return None;
            }
        }
        let end = rest.find(|ch: char| !ch.is_ascii_digit() && ch != '.')?;
        let (number, unit) = rest.split_at(end);
        let (new_rank, multiplier) = match (time, unit.as_bytes()[0]) {
            (false, b'D') => (1, 86_400),
            (true, b'H') => (2, 3600),
            (true, b'M') => (3, 60),
            (true, b'S') => (4, 1),
            _ => return None,
        };
        if new_rank <= rank {
            return None;
        }
        rank = new_rank;
        let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
        if number.contains('.') && (new_rank != 4 || fraction.is_empty() || fraction.len() > 9) {
            return None;
        }
        let seconds = whole.parse::<u64>().ok()?.checked_mul(multiplier)?;
        let nanos = if fraction.is_empty() {
            0
        } else {
            fraction
                .parse::<u32>()
                .ok()?
                .checked_mul(10_u32.pow((9 - fraction.len()) as u32))?
        };
        duration = duration.checked_add(Duration::new(seconds, nanos))?;
        rest = &unit[1..];
    }
    (rank > 0).then_some(duration)
}

impl OnvifClient {
    /// Read a configuration and its reported operation spaces and timeouts.
    pub async fn ptz_motion_capabilities(
        &self,
        ptz_url: &str,
        configuration_token: &str,
    ) -> Result<PtzMotionCapabilities, OnvifError> {
        let configuration = self
            .ptz_get_configuration(ptz_url, configuration_token)
            .await?;
        if configuration.token != configuration_token {
            return Err(SoapError::InvalidValue {
                field: "PTZ configuration token",
                value: "response mismatch".into(),
            }
            .into());
        }
        let body = format!(
            "<tptz:GetConfigurationOptions><tptz:ConfigurationToken>{}</tptz:ConfigurationToken></tptz:GetConfigurationOptions>",
            xml_escape(configuration_token)
        );
        let xml = self
            .call(
                ptz_url,
                "http://www.onvif.org/ver20/ptz/wsdl/GetConfigurationOptions",
                &body,
            )
            .await?;
        let body = parse_soap_body(&xml)?;
        let resp = find_response(&body, "GetConfigurationOptionsResponse")?;
        let options = PtzConfigurationOptions::from_xml(resp)?;
        let spaces = resp
            .path(&["PTZConfigurationOptions", "Spaces"])
            .map(reported_spaces);
        Ok(PtzMotionCapabilities {
            configuration,
            options,
            spaces,
        })
    }

    /// Send one typed motion request. Supply the capabilities for this profile's
    /// bound configuration to validate reported limits before sending.
    pub async fn ptz_move(
        &self,
        ptz_url: &str,
        profile_token: &str,
        motion: &PtzMotion,
        caps: Option<&PtzMotionCapabilities>,
    ) -> Result<(), OnvifError> {
        if profile_token.is_empty() {
            return Err(invalid("empty PTZ profile token"));
        }
        let (name, action, response, payload) = motion_body(motion, caps)?;
        let body = format!(
            "<tptz:{name}><tptz:ProfileToken>{}</tptz:ProfileToken>{payload}</tptz:{name}>",
            xml_escape(profile_token)
        );
        let xml = self.call(ptz_url, action, &body).await?;
        find_response(&parse_soap_body(&xml)?, response)?;
        Ok(())
    }
}

impl OnvifSession {
    /// Resolve the PTZ endpoint and read the requested configuration's options.
    pub async fn ptz_motion_capabilities(
        &self,
        configuration_token: &str,
    ) -> Result<PtzMotionCapabilities, OnvifError> {
        let url = self
            .capabilities()
            .ptz
            .url
            .as_deref()
            .ok_or_else(|| SoapError::missing("PTZ service URL"))?;
        self.client()
            .ptz_motion_capabilities(url, configuration_token)
            .await
    }

    /// Send one typed motion using the resolved PTZ service.
    pub async fn ptz_move(
        &self,
        profile_token: &str,
        motion: &PtzMotion,
        caps: Option<&PtzMotionCapabilities>,
    ) -> Result<(), OnvifError> {
        let url = self
            .capabilities()
            .ptz
            .url
            .as_deref()
            .ok_or_else(|| SoapError::missing("PTZ service URL"))?;
        self.client()
            .ptz_move(url, profile_token, motion, caps)
            .await
    }
}
