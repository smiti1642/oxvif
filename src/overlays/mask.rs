use std::{fmt::Write, str::FromStr};

use crate::{
    OnvifError,
    scoped_rpc::{MEDIA2, SCHEMA, field, required, scalar, text, token},
    soap::{
        SoapError,
        qualified::{QualifiedElement, invalid},
    },
    types::xml_escape,
};

use super::{
    Media2Overlays, OverlayWrite, OverlayWriteError, boolean, entries, preflight_error, shape,
};

const MAX_POINTS: usize = 64;
const DEFAULT_COLORSPACE: &str = "http://www.onvif.org/ver10/colorspace/YCbCr";

/// A point in normalized ONVIF coordinates relative to the configured video window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaskPoint {
    /// Horizontal coordinate in [-1, 1].
    pub x: f32,
    /// Vertical coordinate in [-1, 1].
    pub y: f32,
}

/// Three channels in a device-supported colorspace.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskColor {
    /// Channel values X, Y and Z.
    pub channels: [f32; 3],
    /// Optional URI, with the standard YCbCr default.
    pub colorspace: Option<String>,
}

/// Privacy mask effect. Unknown device values can be observed but cannot be written.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MaskKind {
    /// Solid color.
    Color,
    /// Pixelated region.
    Pixelated,
    /// Blurred region.
    Blurred,
    /// An unmodeled effect from the device.
    Other(String),
}

impl MaskKind {
    fn from_wire(value: String) -> Self {
        match value.as_str() {
            "Color" => Self::Color,
            "Pixelated" => Self::Pixelated,
            "Blurred" => Self::Blurred,
            _ => Self::Other(value),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Color => "Color",
            Self::Pixelated => "Pixelated",
            Self::Blurred => "Blurred",
            Self::Other(value) => value,
        }
    }
}

/// Settings that can be changed through Media2.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskSettings {
    /// Open polygon in vertex order. The last point connects to the first.
    pub polygon: Vec<MaskPoint>,
    /// Requested mask effect.
    pub kind: MaskKind,
    /// Solid fill color, required for Color writes.
    pub color: Option<MaskColor>,
    /// Whether the mask currently covers the region.
    pub enabled: bool,
}

/// A mask and its device-assigned identity.
#[derive(Clone, Debug, PartialEq)]
pub struct Mask {
    /// Device-assigned mask token.
    pub token: String,
    /// Owning video-source configuration token.
    pub configuration_token: String,
    /// Settings reported by the device.
    pub settings: MaskSettings,
}

/// A color range per channel in an advertised colorspace.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskColorRange {
    /// Inclusive channel limits in X, Y, Z order.
    pub channels: [(f32, f32); 3],
    /// Colorspace URI.
    pub colorspace: String,
}

/// Device-advertised color choice.
#[derive(Clone, Debug, PartialEq)]
pub enum MaskColorOptions {
    /// A list of discrete colors.
    List(Vec<MaskColor>),
    /// Per-colorspace channel ranges.
    Ranges(Vec<MaskColorRange>),
}

/// Privacy-mask options for one video-source configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskOptions {
    /// Maximum masks on this configuration.
    pub max_masks: u32,
    /// Maximum vertices on one mask.
    pub max_points: u32,
    /// Advertised effects.
    pub kinds: Vec<MaskKind>,
    /// Permitted solid colors.
    pub colors: MaskColorOptions,
    /// Device restricts geometry to four-corner rectangles.
    pub rectangle_only: Option<bool>,
    /// Setting a color can update other masks sharing the video source.
    pub single_color_only: Option<bool>,
}

impl Media2Overlays {
    /// Read limits, geometry restrictions and supported effects.
    pub async fn mask_options(&self) -> Result<MaskOptions, OnvifError> {
        let response = self
            .service
            .call(
                "GetMaskOptions",
                "GetMaskOptionsResponse",
                &self.configuration_field(),
            )
            .await?;
        let options = required(&response, MEDIA2, "Options")?;
        let colors = required(options, MEDIA2, "Color")?;
        let mut list = Vec::new();
        let mut ranges = Vec::new();
        for item in colors.children() {
            match (item.namespace(), item.local_name()) {
                (SCHEMA, "ColorList") => list.push(parse_color(item)?),
                (SCHEMA, "ColorspaceRange") => ranges.push(parse_range(item)?),
                _ => return Err(invalid("unmodeled mask color option").into()),
            }
        }
        let colors = match (list.is_empty(), ranges.is_empty()) {
            (false, true) => MaskColorOptions::List(list),
            (true, false) => MaskColorOptions::Ranges(ranges),
            _ => return Err(invalid("expected one mask color option family").into()),
        };
        let kinds = options
            .children()
            .filter(|item| item.namespace() == MEDIA2 && item.local_name() == "Types")
            .map(|item| item.scalar().map(MaskKind::from_wire))
            .collect::<Result<Vec<_>, _>>()?;
        if kinds.is_empty() {
            return Err(SoapError::missing("mask Types").into());
        }
        Ok(MaskOptions {
            max_masks: scalar(options, MEDIA2, "MaxMasks")?,
            max_points: scalar(options, MEDIA2, "MaxPoints")?,
            kinds,
            colors,
            rectangle_only: options.attr("", "RectangleOnly").map(boolean).transpose()?,
            single_color_only: options
                .attr("", "SingleColorOnly")
                .map(boolean)
                .transpose()?,
        })
    }

    /// List all masks on this configuration.
    pub async fn masks(&self) -> Result<Vec<Mask>, OnvifError> {
        let response = self
            .service
            .call("GetMasks", "GetMasksResponse", &self.configuration_field())
            .await?;
        entries(
            &response,
            "Masks",
            &self.configuration,
            parse_mask,
            |mask| (&mask.token, &mask.configuration_token),
        )
    }

    /// Read a single mask on this configuration.
    pub async fn mask(&self, mask_token: &str) -> Result<Mask, OnvifError> {
        token(mask_token)?;
        let body = field("Token", mask_token) + &self.configuration_field();
        let response = self
            .service
            .call("GetMasks", "GetMasksResponse", &body)
            .await?;
        let mut result = entries(
            &response,
            "Masks",
            &self.configuration,
            parse_mask,
            |mask| (&mask.token, &mask.configuration_token),
        )?;
        if result.len() != 1 || result[0].token != mask_token {
            return Err(invalid("expected a single matching mask").into());
        }
        Ok(result.remove(0))
    }

    /// Create a mask and independently read back the assigned token.
    pub async fn create_mask(
        &self,
        settings: &MaskSettings,
    ) -> Result<OverlayWrite<Mask>, OverlayWriteError> {
        let options = self
            .mask_options()
            .await
            .map_err(OverlayWriteError::Preflight)?;
        validate(settings, &options)?;
        let existing = self.masks().await.map_err(OverlayWriteError::Preflight)?;
        if existing.len() >= options.max_masks as usize {
            return Err(preflight_error("mask configuration quota exceeded"));
        }
        let response = self
            .write(
                "CreateMask",
                "CreateMaskResponse",
                &mask_body("Mask", None, self.configuration_token(), settings),
            )
            .await?;
        let token = Self::created_token(&response, "Token")?;
        let observed = self.mask(&token).await;
        Ok(OverlayWrite { token, observed })
    }

    /// Replace a mask on this configuration. SingleColorOnly devices can also
    /// update the color of other masks sharing its video source.
    pub async fn set_mask(
        &self,
        mask_token: &str,
        settings: &MaskSettings,
    ) -> Result<OverlayWrite<Mask>, OverlayWriteError> {
        token(mask_token).map_err(OverlayWriteError::Preflight)?;
        let options = self
            .mask_options()
            .await
            .map_err(OverlayWriteError::Preflight)?;
        validate(settings, &options)?;
        let existing = self
            .mask(mask_token)
            .await
            .map_err(OverlayWriteError::Preflight)?;
        if matches!(existing.settings.kind, MaskKind::Other(_)) {
            return Err(preflight_error("existing mask effect is unmodeled"));
        }
        self.write(
            "SetMask",
            "SetMaskResponse",
            &mask_body(
                "Mask",
                Some(mask_token),
                self.configuration_token(),
                settings,
            ),
        )
        .await?;
        let observed = self.mask(mask_token).await;
        Ok(OverlayWrite {
            token: mask_token.into(),
            observed,
        })
    }

    /// Delete a mask after verifying the configuration and read back its absence.
    pub async fn delete_mask(
        &self,
        mask_token: &str,
    ) -> Result<OverlayWrite<bool>, OverlayWriteError> {
        self.mask(mask_token)
            .await
            .map_err(OverlayWriteError::Preflight)?;
        self.write(
            "DeleteMask",
            "DeleteMaskResponse",
            &field("Token", mask_token),
        )
        .await?;
        let observed = self
            .masks()
            .await
            .map(|items| !items.iter().any(|item| item.token == mask_token));
        Ok(OverlayWrite {
            token: mask_token.into(),
            observed,
        })
    }
}

pub(crate) fn validate(
    settings: &MaskSettings,
    options: &MaskOptions,
) -> Result<(), OverlayWriteError> {
    let points = &settings.polygon;
    if points.len() > options.max_points as usize
        || !valid_polygon(points)
        || (options.rectangle_only == Some(true) && !rectangle(points))
    {
        return Err(preflight_error("invalid or unsupported mask geometry"));
    }
    if matches!(settings.kind, MaskKind::Other(_)) || !options.kinds.contains(&settings.kind) {
        return Err(preflight_error("unsupported mask effect"));
    }
    match (&settings.kind, &settings.color) {
        (MaskKind::Color, Some(color)) if color_allowed(color, &options.colors) => Ok(()),
        (MaskKind::Blurred | MaskKind::Pixelated, None) => Ok(()),
        _ => Err(preflight_error("invalid or unsupported mask color")),
    }
}

fn color_allowed(color: &MaskColor, options: &MaskColorOptions) -> bool {
    let space = color.colorspace.as_deref().unwrap_or(DEFAULT_COLORSPACE);
    match options {
        MaskColorOptions::List(colors) => colors.iter().any(|option| {
            option.colorspace.as_deref().unwrap_or(DEFAULT_COLORSPACE) == space
                && color.channels == option.channels
        }),
        MaskColorOptions::Ranges(ranges) => ranges.iter().any(|option| {
            option.colorspace == space
                && option
                    .channels
                    .iter()
                    .zip(color.channels)
                    .all(|(&(min, max), value)| (min..=max).contains(&value))
        }),
    }
}

fn cross(a: MaskPoint, b: MaskPoint, c: MaskPoint) -> f64 {
    (f64::from(b.x) - f64::from(a.x)) * (f64::from(c.y) - f64::from(a.y))
        - (f64::from(b.y) - f64::from(a.y)) * (f64::from(c.x) - f64::from(a.x))
}

fn intersects(a: MaskPoint, b: MaskPoint, c: MaskPoint, d: MaskPoint) -> bool {
    let ab_min = [a.x.min(b.x), a.y.min(b.y)];
    let ab_max = [a.x.max(b.x), a.y.max(b.y)];
    let cd_min = [c.x.min(d.x), c.y.min(d.y)];
    let cd_max = [c.x.max(d.x), c.y.max(d.y)];
    (0..2).all(|i| ab_min[i] <= cd_max[i] && cd_min[i] <= ab_max[i])
        && cross(a, b, c) * cross(a, b, d) <= 0.0
        && cross(c, d, a) * cross(c, d, b) <= 0.0
}

fn valid_polygon(points: &[MaskPoint]) -> bool {
    let n = points.len();
    if !(3..=MAX_POINTS).contains(&n)
        || points.iter().enumerate().any(|(i, p)| {
            !(-1.0..=1.0).contains(&p.x) || !(-1.0..=1.0).contains(&p.y) || points[..i].contains(p)
        })
    {
        return false;
    }
    let mut area = 0.0;
    for i in 0..n {
        let next = (i + 1) % n;
        let a = points[i];
        let b = points[next];
        area += f64::from(a.x) * f64::from(b.y) - f64::from(a.y) * f64::from(b.x);
        for j in i + 2..n {
            let after = (j + 1) % n;
            if after != i && intersects(a, b, points[j], points[after]) {
                return false;
            }
        }
    }
    area != 0.0
}

fn rectangle(points: &[MaskPoint]) -> bool {
    points.len() == 4
        && (0..4).all(|i| {
            let a = points[i];
            let b = points[(i + 1) % 4];
            let c = points[(i + 2) % 4];
            (a.x == b.x && b.y == c.y) || (a.y == b.y && b.x == c.x)
        })
}

pub(crate) fn mask_body(
    wrapper: &str,
    mask_token: Option<&str>,
    config: &str,
    settings: &MaskSettings,
) -> String {
    let attr = mask_token
        .map(|token| format!(" token=\"{}\"", xml_escape(token)))
        .unwrap_or_default();
    let mut result = format!(
        "<{wrapper}{attr}>{}<Polygon>",
        field("ConfigurationToken", config)
    );
    for point in &settings.polygon {
        write!(result, "<tt:Point x=\"{}\" y=\"{}\"/>", point.x, point.y).unwrap();
    }
    result.push_str("</Polygon>");
    result.push_str(&field("Type", settings.kind.as_str()));
    if let Some(color) = &settings.color {
        let [x, y, z] = color.channels;
        let space = color
            .colorspace
            .as_deref()
            .map(|value| format!(" Colorspace=\"{}\"", xml_escape(value)))
            .unwrap_or_default();
        write!(result, "<Color X=\"{x}\" Y=\"{y}\" Z=\"{z}\"{space}/>").unwrap();
    }
    write!(
        result,
        "<Enabled>{}</Enabled></{wrapper}>",
        settings.enabled
    )
    .unwrap();
    result
}

fn attribute<T: FromStr>(node: &QualifiedElement, name: &'static str) -> Result<T, SoapError> {
    node.attr("", name)
        .and_then(|value| value.trim().parse().ok())
        .ok_or_else(|| invalid(name))
}

fn parse_color(node: &QualifiedElement) -> Result<MaskColor, SoapError> {
    shape(node, SCHEMA, &[], &["X", "Y", "Z", "Colorspace"])?;
    let channels = [
        attribute::<f32>(node, "X")?,
        attribute(node, "Y")?,
        attribute(node, "Z")?,
    ];
    if !channels.iter().all(|v| v.is_finite()) {
        return Err(invalid("nonfinite mask color"));
    }
    Ok(MaskColor {
        channels,
        colorspace: node.attr("", "Colorspace").map(str::to_owned),
    })
}

fn parse_range(node: &QualifiedElement) -> Result<MaskColorRange, SoapError> {
    let range = |name| {
        let element = required(node, SCHEMA, name)?;
        let min = scalar::<f32>(element, SCHEMA, "Min")?;
        let max = scalar::<f32>(element, SCHEMA, "Max")?;
        if !min.is_finite() || !max.is_finite() || min > max {
            return Err(invalid("invalid mask color range"));
        }
        Ok((min, max))
    };
    Ok(MaskColorRange {
        channels: [range("X")?, range("Y")?, range("Z")?],
        colorspace: text(node, SCHEMA, "Colorspace")?,
    })
}

pub(crate) fn parse_settings(node: &QualifiedElement) -> Result<MaskSettings, SoapError> {
    shape(
        node,
        MEDIA2,
        &["ConfigurationToken", "Polygon", "Type", "Color", "Enabled"],
        &["token"],
    )?;
    let geometry = required(node, MEDIA2, "Polygon")?;
    shape(geometry, SCHEMA, &["Point"], &[])?;
    let polygon = geometry
        .children()
        .take(MAX_POINTS + 1)
        .map(|node| {
            shape(node, SCHEMA, &[], &["x", "y"])?;
            Ok::<_, SoapError>(MaskPoint {
                x: attribute(node, "x")?,
                y: attribute(node, "y")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !valid_polygon(&polygon) {
        return Err(invalid("invalid mask polygon"));
    }
    Ok(MaskSettings {
        polygon,
        kind: MaskKind::from_wire(text(node, MEDIA2, "Type")?),
        color: node.one(MEDIA2, "Color")?.map(parse_color).transpose()?,
        enabled: boolean(&text(node, MEDIA2, "Enabled")?)?,
    })
}

fn parse_mask(node: &QualifiedElement) -> Result<Mask, OnvifError> {
    let mask_token = node
        .attr("", "token")
        .ok_or_else(|| SoapError::missing("mask token"))?;
    token(mask_token)?;
    let configuration_token = text(node, MEDIA2, "ConfigurationToken")?;
    token(&configuration_token)?;
    Ok(Mask {
        token: mask_token.into(),
        configuration_token,
        settings: parse_settings(node)?,
    })
}
