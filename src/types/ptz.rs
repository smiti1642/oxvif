use super::{IntRange, PtzSpaceRange, PtzSpeed, parse_space_range, xml_escape, xml_str};
use crate::error::OnvifError;
use crate::soap::{SoapError, XmlNode};

// ── PtzPreset ─────────────────────────────────────────────────────────────────

/// A named PTZ preset position returned by `GetPresets`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
        resp.children_named("Preset")
            .map(|p| {
                let token = p
                    .attr("token")
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| SoapError::missing("Preset/@token"))?
                    .to_string();
                Ok(Self {
                    token,
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
            })
            .collect()
    }
}

// ── PtzStatus ─────────────────────────────────────────────────────────────────

/// Current PTZ position and movement state returned by `GetStatus`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// UTC timestamp of this status snapshot, if reported by the device.
    pub utc_time: Option<String>,
    /// Human-readable error description from `PTZStatus/Error`, if present.
    pub error: Option<String>,
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
            utc_time: status.child("UtcTime").map(|n| n.text().to_string()),
            error: status.child("Error").map(|n| n.text().to_string()),
        })
    }
}

// ── Preset tours ──────────────────────────────────────────────────────────────
//
// A preset tour is a stored sequence of stops the camera walks on its own — a
// guard tour. `tt:PresetTour` is the stored definition, and the same type is
// what `ModifyPresetTour` writes back, so these types are both read and
// written and every string interpolated into `to_xml_body` is escaped.
//
// Two shapes here are easy to get wrong and are called out where they appear:
// `PresetDetail` is an `xs:choice` (modelled as a Rust enum so more than one
// variant cannot be serialised), and `Direction` is a single value inside
// `StartingCondition` but a repeated list inside `StartingConditionOptions`.

/// State of a preset tour, from `tt:PTZPresetTourState`.
///
/// The schema is a string restriction whose last member is `Extended`, so the
/// set is open in practice. An unrecognised value becomes
/// [`Unknown`](Self::Unknown) rather than an error — a vendor string must not
/// turn `GetPresetTours` into an `Err`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PtzPresetTourState {
    /// The tour exists but is not running.
    #[default]
    Idle,
    /// The tour is running.
    Touring,
    /// The tour is running but suspended.
    Paused,
    /// The schema's own `Extended` member.
    Extended,
    /// A value the schema does not define. Carries the device's string.
    Unknown(String),
}

impl PtzPresetTourState {
    fn parse(s: &str) -> Self {
        match s {
            "Idle" => Self::Idle,
            "Touring" => Self::Touring,
            "Paused" => Self::Paused,
            "Extended" => Self::Extended,
            other => Self::Unknown(other.to_string()),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "Idle",
            Self::Touring => "Touring",
            Self::Paused => "Paused",
            Self::Extended => "Extended",
            Self::Unknown(s) => s,
        }
    }
}

/// Direction a preset tour walks its stops, from `tt:PTZPresetTourDirection`.
///
/// Open in the same way as [`PtzPresetTourState`] — unknown values are
/// carried, not rejected.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PtzPresetTourDirection {
    /// Walk the stops in list order.
    #[default]
    Forward,
    /// Walk the stops in reverse list order.
    Backward,
    /// The schema's own `Extended` member.
    Extended,
    /// A value the schema does not define. Carries the device's string.
    Unknown(String),
}

impl PtzPresetTourDirection {
    fn parse(s: &str) -> Self {
        match s {
            "Forward" => Self::Forward,
            "Backward" => Self::Backward,
            "Extended" => Self::Extended,
            other => Self::Unknown(other.to_string()),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Forward => "Forward",
            Self::Backward => "Backward",
            Self::Extended => "Extended",
            Self::Unknown(s) => s,
        }
    }
}

/// Where one tour stop points the camera, from `tt:PTZPresetTourPresetDetail`.
///
/// The schema member is an **`xs:choice`** — exactly one of these, never two.
/// Modelling it as a Rust enum is what makes the invalid state
/// unrepresentable: a struct with three `Option` fields would let
/// [`ptz_modify_preset_tour`](crate::OnvifClient::ptz_modify_preset_tour) send
/// a schema-invalid body that a device rejects with an unhelpful fault.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum PtzPresetTourPresetDetail {
    /// Go to a stored preset, by token.
    PresetToken(String),
    /// Go to the configured home position.
    ///
    /// The schema types this member `xs:boolean`, but a `false` there has no
    /// defined meaning — the choice being *present* is the instruction. Any
    /// `<Home>` element parses to this variant.
    Home,
    /// Go to an explicit position rather than a stored one.
    Position {
        /// Pan (x) and tilt (y), normalised range `[-1.0, 1.0]`.
        pan_tilt: Option<(f32, f32)>,
        /// Zoom, normalised range `[0.0, 1.0]`.
        zoom: Option<f32>,
    },
}

impl PtzPresetTourPresetDetail {
    fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        // Parse order matches the schema's choice order. `TypeExtension` is
        // deliberately unmodelled — it carries vendor detail this crate has no
        // way to interpret.
        if let Some(t) = node.child("PresetToken") {
            return Ok(Self::PresetToken(t.text().to_string()));
        }
        if node.child("Home").is_some() {
            return Ok(Self::Home);
        }
        if let Some(p) = node.child("PTZPosition") {
            return Ok(Self::Position {
                pan_tilt: p.child("PanTilt").and_then(|n| {
                    let x = n.attr("x")?.parse().ok()?;
                    let y = n.attr("y")?.parse().ok()?;
                    Some((x, y))
                }),
                zoom: p.child("Zoom").and_then(|n| n.attr("x")?.parse().ok()),
            });
        }
        Err(SoapError::missing("PTZPresetTourPresetDetail").into())
    }

    fn to_xml_body(&self) -> String {
        match self {
            Self::PresetToken(t) => {
                format!("<tt:PresetToken>{}</tt:PresetToken>", xml_escape(t))
            }
            Self::Home => "<tt:Home>true</tt:Home>".to_string(),
            Self::Position { pan_tilt, zoom } => {
                let pt = pan_tilt
                    .map(|(x, y)| format!("<tt:PanTilt x=\"{x}\" y=\"{y}\"/>"))
                    .unwrap_or_default();
                let z = zoom
                    .map(|x| format!("<tt:Zoom x=\"{x}\"/>"))
                    .unwrap_or_default();
                format!("<tt:PTZPosition>{pt}{z}</tt:PTZPosition>")
            }
        }
    }
}

/// One stop on a preset tour, from `tt:PTZPresetTourSpot`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct PtzPresetTourSpot {
    /// Where this stop points the camera. Required by the schema.
    pub preset_detail: PtzPresetTourPresetDetail,
    /// Speed to move at. `None` leaves it to the configuration default.
    pub speed: Option<PtzSpeed>,
    /// How long to dwell here, as an ISO 8601 duration (e.g. `"PT10S"`).
    pub stay_time: Option<String>,
}

fn parse_ptz_speed(node: &XmlNode) -> PtzSpeed {
    PtzSpeed {
        pan_tilt: node.child("PanTilt").and_then(|n| {
            let x = n.attr("x")?.parse().ok()?;
            let y = n.attr("y")?.parse().ok()?;
            Some((x, y))
        }),
        zoom: node.child("Zoom").and_then(|n| n.attr("x")?.parse().ok()),
    }
}

fn speed_xml(s: &PtzSpeed) -> String {
    let pt = s
        .pan_tilt
        .map(|(x, y)| format!("<tt:PanTilt x=\"{x}\" y=\"{y}\"/>"))
        .unwrap_or_default();
    let z = s
        .zoom
        .map(|x| format!("<tt:Zoom x=\"{x}\"/>"))
        .unwrap_or_default();
    format!("<tt:Speed>{pt}{z}</tt:Speed>")
}

impl PtzPresetTourSpot {
    fn from_xml(node: &XmlNode) -> Result<Self, OnvifError> {
        let detail = node
            .child("PresetDetail")
            .ok_or_else(|| SoapError::missing("PTZPresetTourSpot/PresetDetail"))?;
        Ok(Self {
            preset_detail: PtzPresetTourPresetDetail::from_xml(detail)?,
            speed: node.child("Speed").map(parse_ptz_speed),
            stay_time: xml_str(node, "StayTime"),
        })
    }

    fn to_xml_body(&self) -> String {
        let speed = self.speed.as_ref().map(speed_xml).unwrap_or_default();
        let stay = self
            .stay_time
            .as_deref()
            .map(|d| format!("<tt:StayTime>{}</tt:StayTime>", xml_escape(d)))
            .unwrap_or_default();
        format!(
            "<tt:TourSpot><tt:PresetDetail>{}</tt:PresetDetail>{speed}{stay}</tt:TourSpot>",
            self.preset_detail.to_xml_body()
        )
    }
}

/// Whether a tour runs on its own and how, from
/// `tt:PTZPresetTourStartingCondition`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct PtzPresetTourStartingCondition {
    /// Visit stops in random order rather than list order. Carried on the
    /// element as an **attribute**, not a child element.
    pub random_preset_order: Option<bool>,
    /// How many times to repeat the tour. `None` means the device did not say.
    pub recurring_time: Option<u32>,
    /// How long to keep touring, as an ISO 8601 duration.
    pub recurring_duration: Option<String>,
    /// Direction to walk the stops.
    ///
    /// **A single value here**, unlike
    /// [`PtzPresetTourStartingConditionOptions::directions`], which lists what
    /// the device supports. Same element name, different cardinality.
    pub direction: Option<PtzPresetTourDirection>,
}

impl PtzPresetTourStartingCondition {
    fn from_xml(node: &XmlNode) -> Self {
        Self {
            random_preset_order: node
                .attr("RandomPresetOrder")
                .map(|v| v == "true" || v == "1"),
            recurring_time: node
                .child("RecurringTime")
                .and_then(|n| n.text().parse().ok()),
            recurring_duration: xml_str(node, "RecurringDuration"),
            direction: node
                .child("Direction")
                .map(|n| PtzPresetTourDirection::parse(n.text())),
        }
    }

    fn to_xml_body(&self) -> String {
        let random = self
            .random_preset_order
            .map(|v| format!(" RandomPresetOrder=\"{v}\""))
            .unwrap_or_default();
        let time = self
            .recurring_time
            .map(|v| format!("<tt:RecurringTime>{v}</tt:RecurringTime>"))
            .unwrap_or_default();
        let duration = self
            .recurring_duration
            .as_deref()
            .map(|d| {
                format!(
                    "<tt:RecurringDuration>{}</tt:RecurringDuration>",
                    xml_escape(d)
                )
            })
            .unwrap_or_default();
        let direction = self
            .direction
            .as_ref()
            .map(|d| format!("<tt:Direction>{}</tt:Direction>", xml_escape(d.as_str())))
            .unwrap_or_default();
        format!("<tt:StartingCondition{random}>{time}{duration}{direction}</tt:StartingCondition>")
    }
}

/// Runtime state of a preset tour, from `tt:PTZPresetTourStatus`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct PtzPresetTourStatus {
    /// Whether the tour is idle, running or paused.
    pub state: PtzPresetTourState,
    /// The stop the camera is currently at, while touring.
    pub current_tour_spot: Option<PtzPresetTourSpot>,
}

/// A stored preset tour, from `tt:PresetTour`.
///
/// Read by [`ptz_get_preset_tours`](crate::OnvifClient::ptz_get_preset_tours)
/// and written back by
/// [`ptz_modify_preset_tour`](crate::OnvifClient::ptz_modify_preset_tour).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct PtzPresetTour {
    /// Opaque tour identifier.
    ///
    /// **Optional in the schema**, unlike `tt:PTZPreset/@token` — a device may
    /// legitimately return a tour with no token, so a missing one is not a
    /// parse error here. It is still required on the way *in* to every
    /// operation that names an existing tour.
    pub token: Option<String>,
    /// Human-readable tour name.
    pub name: Option<String>,
    /// Runtime state. Required by the schema.
    pub status: PtzPresetTourStatus,
    /// Whether the tour starts on its own. Required by the schema.
    pub auto_start: bool,
    /// Repetition and direction. Required by the schema.
    pub starting_condition: PtzPresetTourStartingCondition,
    /// The stops, in order. Empty is schema-valid — a tour can exist before
    /// its stops are added.
    pub tour_spots: Vec<PtzPresetTourSpot>,
}

impl PtzPresetTour {
    fn from_node(node: &XmlNode) -> Result<Self, OnvifError> {
        // `Status`, `AutoStart` and `StartingCondition` are the three
        // `minOccurs="1"` members. `@token` is deliberately not one of them.
        let status_node = node
            .child("Status")
            .ok_or_else(|| SoapError::missing("PresetTour/Status"))?;
        let auto_start = node
            .child("AutoStart")
            .ok_or_else(|| SoapError::missing("PresetTour/AutoStart"))?;
        let condition = node
            .child("StartingCondition")
            .ok_or_else(|| SoapError::missing("PresetTour/StartingCondition"))?;

        let current_tour_spot = match status_node.child("CurrentTourSpot") {
            Some(n) => Some(PtzPresetTourSpot::from_xml(n)?),
            None => None,
        };

        Ok(Self {
            token: node
                .attr("token")
                .filter(|t| !t.is_empty())
                .map(str::to_string),
            name: xml_str(node, "Name"),
            status: PtzPresetTourStatus {
                state: status_node
                    .child("State")
                    .map(|n| PtzPresetTourState::parse(n.text()))
                    .unwrap_or_default(),
                current_tour_spot,
            },
            auto_start: auto_start.text() == "true" || auto_start.text() == "1",
            starting_condition: PtzPresetTourStartingCondition::from_xml(condition),
            tour_spots: node
                .children_named("TourSpot")
                .map(PtzPresetTourSpot::from_xml)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    /// Parse every `<PresetTour>` child of a `GetPresetToursResponse`.
    pub(crate) fn vec_from_xml(resp: &XmlNode) -> Result<Vec<Self>, OnvifError> {
        resp.children_named("PresetTour")
            .map(Self::from_node)
            .collect()
    }

    /// Parse the single `<PresetTour>` of a `GetPresetTourResponse`.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let node = resp
            .child("PresetTour")
            .ok_or_else(|| SoapError::missing("PresetTour"))?;
        Self::from_node(node)
    }

    /// Serialise as the `<tptz:PresetTour>` body of a `ModifyPresetTour`.
    pub(crate) fn to_xml_body(&self) -> String {
        let token = self
            .token
            .as_deref()
            .map(|t| format!(" token=\"{}\"", xml_escape(t)))
            .unwrap_or_default();
        let name = self
            .name
            .as_deref()
            .map(|n| format!("<tt:Name>{}</tt:Name>", xml_escape(n)))
            .unwrap_or_default();
        let spots: String = self.tour_spots.iter().map(|s| s.to_xml_body()).collect();
        format!(
            "<tptz:PresetTour{token}>\
               {name}\
               <tt:Status><tt:State>{state}</tt:State></tt:Status>\
               <tt:AutoStart>{auto}</tt:AutoStart>\
               {condition}\
               {spots}\
             </tptz:PresetTour>",
            state = xml_escape(self.status.state.as_str()),
            auto = self.auto_start,
            condition = self.starting_condition.to_xml_body(),
        )
    }
}

// ── Preset tour options ───────────────────────────────────────────────────────

/// What starting conditions a device will accept, from
/// `tt:PTZPresetTourStartingConditionOptions`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct PtzPresetTourStartingConditionOptions {
    /// Accepted range for `RecurringTime`.
    pub recurring_time: Option<IntRange>,
    /// Accepted `(min, max)` for `RecurringDuration`, as ISO 8601 durations.
    pub recurring_duration: Option<(String, String)>,
    /// Every direction the device supports.
    ///
    /// **A list here**, where [`PtzPresetTourStartingCondition::direction`] is
    /// a single value. Same element name, different cardinality — this is the
    /// asymmetry to watch when reading the schema.
    pub directions: Vec<PtzPresetTourDirection>,
}

/// What a device will accept as a stop target, from
/// `tt:PTZPresetTourPresetDetailOptions`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct PtzPresetTourPresetDetailOptions {
    /// Preset tokens that may be used as a stop.
    pub preset_tokens: Vec<String>,
    /// Whether the home position may be used as a stop.
    pub home: Option<bool>,
    /// Pan/tilt space and limits for an explicit-position stop.
    pub pan_tilt_position_space: Option<PtzSpaceRange>,
    /// Zoom space and limits for an explicit-position stop.
    pub zoom_position_space: Option<PtzSpaceRange>,
}

/// What a device will accept for a tour stop, from
/// `tt:PTZPresetTourSpotOptions`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct PtzPresetTourSpotOptions {
    /// Accepted stop targets. Required by the schema.
    pub preset_detail: PtzPresetTourPresetDetailOptions,
    /// Accepted `(min, max)` dwell time, as ISO 8601 durations. Required by
    /// the schema.
    pub stay_time: (String, String),
}

/// What preset tours a device will accept, returned by `GetPresetTourOptions`.
///
/// Worth reading before building a tour for
/// [`ptz_modify_preset_tour`](crate::OnvifClient::ptz_modify_preset_tour) — a
/// stop outside these bounds comes back as a fault rather than being clamped.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct PtzPresetTourOptions {
    /// Whether the device supports tours that start on their own.
    pub auto_start: bool,
    /// Accepted repetition and direction.
    pub starting_condition: PtzPresetTourStartingConditionOptions,
    /// Accepted stop shapes and dwell times.
    pub tour_spot: PtzPresetTourSpotOptions,
}

/// Read a `tt:DurationRange` (`Min`/`Max` of `xs:duration`) as a string pair.
fn duration_range(node: &XmlNode) -> Option<(String, String)> {
    let min = node.child("Min")?.text().to_string();
    let max = node.child("Max")?.text().to_string();
    Some((min, max))
}

impl PtzPresetTourOptions {
    /// Parse from a `GetPresetTourOptionsResponse` node.
    pub(crate) fn from_xml(resp: &XmlNode) -> Result<Self, OnvifError> {
        let opts = resp
            .child("Options")
            .ok_or_else(|| SoapError::missing("Options"))?;
        // All three are `minOccurs="1"` on `tt:PTZPresetTourOptions`.
        let auto_start = opts
            .child("AutoStart")
            .ok_or_else(|| SoapError::missing("Options/AutoStart"))?;
        let condition = opts
            .child("StartingCondition")
            .ok_or_else(|| SoapError::missing("Options/StartingCondition"))?;
        let spot = opts
            .child("TourSpot")
            .ok_or_else(|| SoapError::missing("Options/TourSpot"))?;

        let detail = spot
            .child("PresetDetail")
            .ok_or_else(|| SoapError::missing("Options/TourSpot/PresetDetail"))?;
        let stay_time = spot
            .child("StayTime")
            .and_then(duration_range)
            .ok_or_else(|| SoapError::missing("Options/TourSpot/StayTime"))?;

        Ok(Self {
            auto_start: auto_start.text() == "true" || auto_start.text() == "1",
            starting_condition: PtzPresetTourStartingConditionOptions {
                recurring_time: condition.child("RecurringTime").map(|r| IntRange {
                    min: r
                        .child("Min")
                        .and_then(|n| n.text().parse().ok())
                        .unwrap_or(0),
                    max: r
                        .child("Max")
                        .and_then(|n| n.text().parse().ok())
                        .unwrap_or(0),
                }),
                recurring_duration: condition
                    .child("RecurringDuration")
                    .and_then(duration_range),
                // `[0..*]` here — the cardinality asymmetry with
                // `PtzPresetTourStartingCondition::direction`.
                directions: condition
                    .children_named("Direction")
                    .map(|n| PtzPresetTourDirection::parse(n.text()))
                    .collect(),
            },
            tour_spot: PtzPresetTourSpotOptions {
                preset_detail: PtzPresetTourPresetDetailOptions {
                    preset_tokens: detail
                        .children_named("PresetToken")
                        .map(|n| n.text().to_string())
                        .collect(),
                    home: detail
                        .child("Home")
                        .map(|n| n.text() == "true" || n.text() == "1"),
                    pan_tilt_position_space: detail
                        .child("PanTiltPositionSpace")
                        .map(parse_space_range),
                    zoom_position_space: detail.child("ZoomPositionSpace").map(parse_space_range),
                },
                stay_time,
            },
        })
    }
}

// ── PtzPresetTourOperation ────────────────────────────────────────────────────

/// What to do to a preset tour, for
/// [`ptz_operate_preset_tour`](crate::OnvifClient::ptz_operate_preset_tour).
///
/// Unlike the response enums in this module this one is **closed**: the client
/// chooses the value, so there is no vendor string that has to survive.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtzPresetTourOperation {
    /// Begin touring.
    Start,
    /// Stop touring and return to idle.
    Stop,
    /// Suspend touring without resetting position in the sequence.
    Pause,
    /// The schema's `Extended` member.
    Extended,
}

impl PtzPresetTourOperation {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Pause => "Pause",
            Self::Extended => "Extended",
        }
    }
}
