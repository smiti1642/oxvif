use crate::{
    OnvifError,
    scoped_rpc::{MEDIA2, SCHEMA, field, required, text, token},
    soap::{
        SoapError,
        qualified::{QualifiedElement, invalid},
    },
    types::{OsdColor, OsdConfiguration, OsdOptions, OsdPosition, OsdTextString, xml_escape},
};

use super::{
    Media2Overlays, OverlayWrite, OverlayWriteError, boolean, entries, preflight_error, shape,
};

impl Media2Overlays {
    /// Read OSD limits and supported types for the bound configuration.
    pub async fn osd_options(&self) -> Result<OsdOptions, OnvifError> {
        let response = self
            .service
            .call(
                "GetOSDOptions",
                "GetOSDOptionsResponse",
                &self.configuration_field(),
            )
            .await?;
        let options = required(&response, MEDIA2, "OSDOptions")?;
        required(options, SCHEMA, "MaximumNumberOfOSDs")?
            .attr("", "Total")
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or_else(|| invalid("invalid OSD quota"))?;
        OsdOptions::from_xml(&response.legacy_projection(&[MEDIA2, SCHEMA]))
    }

    /// List OSDs on this configuration. Images are readable.
    pub async fn osds(&self) -> Result<Vec<OsdConfiguration>, OnvifError> {
        let response = self
            .service
            .call("GetOSDs", "GetOSDsResponse", &self.configuration_field())
            .await?;
        entries(&response, "OSDs", &self.configuration, parse_osd, |osd| {
            (&osd.token, &osd.video_source_config_token)
        })
    }

    /// Read one OSD by token on this configuration.
    pub async fn osd(&self, osd_token: &str) -> Result<OsdConfiguration, OnvifError> {
        token(osd_token)?;
        let body = field("OSDToken", osd_token) + &self.configuration_field();
        let response = self
            .service
            .call("GetOSDs", "GetOSDsResponse", &body)
            .await?;
        let mut result = entries(&response, "OSDs", &self.configuration, parse_osd, |osd| {
            (&osd.token, &osd.video_source_config_token)
        })?;
        if result.len() != 1 || result[0].token != osd_token {
            return Err(invalid("expected a single matching OSD").into());
        }
        Ok(result.remove(0))
    }

    /// Create a text OSD, then read its assigned token back.
    pub async fn create_osd(
        &self,
        osd: &OsdConfiguration,
    ) -> Result<OverlayWrite<OsdConfiguration>, OverlayWriteError> {
        if !osd.token.is_empty() {
            return Err(preflight_error("create requires an empty OSD token"));
        }
        let options = self
            .osd_options()
            .await
            .map_err(OverlayWriteError::Preflight)?;
        self.validate_osd(osd, &options)?;
        let existing = self.osds().await.map_err(OverlayWriteError::Preflight)?;
        quota(osd, &options, &existing)?;
        let response = self
            .write("CreateOSD", "CreateOSDResponse", &body(osd))
            .await?;
        let token = Self::created_token(&response, "OSDToken")?;
        let observed = self.osd(&token).await;
        Ok(OverlayWrite { token, observed })
    }

    /// Replace a text OSD on this configuration. Unknown settings cause a refusal.
    pub async fn set_osd(
        &self,
        osd: &OsdConfiguration,
    ) -> Result<OverlayWrite<OsdConfiguration>, OverlayWriteError> {
        token(&osd.token).map_err(OverlayWriteError::Preflight)?;
        let options = self
            .osd_options()
            .await
            .map_err(OverlayWriteError::Preflight)?;
        self.validate_osd(osd, &options)?;
        let existing = self.osds().await.map_err(OverlayWriteError::Preflight)?;
        if !existing
            .iter()
            .any(|entry| entry.token == osd.token && entry.type_ == "Text")
        {
            return Err(preflight_error(
                "target text OSD is absent from this configuration",
            ));
        }
        quota(osd, &options, &existing)?;
        self.write("SetOSD", "SetOSDResponse", &body(osd)).await?;
        let observed = self.osd(&osd.token).await;
        Ok(OverlayWrite {
            token: osd.token.clone(),
            observed,
        })
    }

    /// Delete a selected OSD after verifying that it belongs to this configuration.
    pub async fn delete_osd(
        &self,
        osd_token: &str,
    ) -> Result<OverlayWrite<bool>, OverlayWriteError> {
        self.osd(osd_token)
            .await
            .map_err(OverlayWriteError::Preflight)?;
        self.write(
            "DeleteOSD",
            "DeleteOSDResponse",
            &field("OSDToken", osd_token),
        )
        .await?;
        let observed = self
            .osds()
            .await
            .map(|items| !items.iter().any(|item| item.token == osd_token));
        Ok(OverlayWrite {
            token: osd_token.into(),
            observed,
        })
    }

    fn validate_osd(
        &self,
        osd: &OsdConfiguration,
        options: &OsdOptions,
    ) -> Result<(), OverlayWriteError> {
        if self.configuration_token() != osd.video_source_config_token
            || osd.type_ != "Text"
            || osd.image_path.is_some()
            || !options.types.iter().any(|kind| kind == "Text")
        {
            return Err(preflight_error(
                "unsupported OSD type or source configuration",
            ));
        }
        let position = &osd.position;
        let coordinates = match (position.x, position.y) {
            (Some(x), Some(y)) => {
                position.type_ == "Custom" && [x, y].iter().all(|n| (-1.0..=1.0).contains(n))
            }
            (None, None) => position.type_ != "Custom",
            _ => false,
        };
        if !coordinates || !options.position_types.contains(&position.type_) {
            return Err(preflight_error("unsupported OSD position"));
        }
        let text = osd
            .text_string
            .as_ref()
            .ok_or_else(|| preflight_error("text OSD requires text settings"))?;
        let fields = (
            text.plain_text.is_some(),
            text.date_format.is_some(),
            text.time_format.is_some(),
        );
        let expected = match text.type_.as_str() {
            "Plain" => (true, false, false),
            "Date" => (false, true, false),
            "Time" => (false, false, true),
            "DateAndTime" => (false, true, true),
            _ => return Err(preflight_error("unmodeled OSD text type")),
        };
        if fields != expected
            || !options.text_types.contains(&text.type_)
            || !choice(text.date_format.as_deref(), &options.date_formats)
            || !choice(text.time_format.as_deref(), &options.time_formats)
            || text.font_size.is_some_and(|size| {
                size > i32::MAX as u32
                    || options
                        .font_size_range
                        .is_some_and(|(min, max)| !(min..=max).contains(&size))
            })
            || text
                .plain_text
                .as_ref()
                .is_some_and(|value| value.len() > 4096 || !xml_text(value))
            || !text
                .font_color
                .iter()
                .chain(text.background_color.iter())
                .all(valid_color)
        {
            return Err(preflight_error(
                "OSD text settings exceed modeled or advertised options",
            ));
        }
        Ok(())
    }
}

fn choice(value: Option<&str>, allowed: &[String]) -> bool {
    value.is_none_or(|value| {
        value.len() <= 256
            && xml_text(value)
            && (allowed.is_empty() || allowed.iter().any(|option| option == value))
    })
}

fn xml_text(value: &str) -> bool {
    value
        .chars()
        .all(|ch| !ch.is_control() || matches!(ch, '\t' | '\n' | '\r'))
}

fn valid_color(color: &OsdColor) -> bool {
    [color.x, color.y, color.z].iter().all(|v| v.is_finite())
        && color.transparent.is_none_or(|v| {
            v.fract() == 0.0 && (i32::MIN as f64..=i32::MAX as f64).contains(&f64::from(v))
        })
        && color
            .colorspace
            .as_ref()
            .is_none_or(|value| value.len() <= 256 && xml_text(value))
}

fn quota(
    osd: &OsdConfiguration,
    options: &OsdOptions,
    existing: &[OsdConfiguration],
) -> Result<(), OverlayWriteError> {
    let others = || existing.iter().filter(|entry| entry.token != osd.token);
    let text = osd.text_string.as_ref().expect("validated text settings");
    if others().count() >= options.max_osd as usize
        || options
            .max_per_text_type
            .get(&text.type_)
            .is_some_and(|limit| {
                others()
                    .filter(|entry| {
                        entry
                            .text_string
                            .as_ref()
                            .is_some_and(|other| other.type_ == text.type_)
                    })
                    .count()
                    >= *limit as usize
            })
    {
        return Err(preflight_error("OSD configuration quota exceeded"));
    }
    Ok(())
}

fn body(osd: &OsdConfiguration) -> String {
    format!(
        "<OSD token=\"{}\">{}</OSD>",
        xml_escape(&osd.token),
        osd.xml_contents()
    )
}

fn optional(node: &QualifiedElement, name: &'static str) -> Result<Option<String>, SoapError> {
    node.one(SCHEMA, name)?
        .map(QualifiedElement::scalar)
        .transpose()
}

fn number(value: Option<&str>) -> Result<f32, SoapError> {
    value
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .ok_or_else(|| invalid("invalid OSD number"))
}

fn color(node: &QualifiedElement) -> Result<OsdColor, SoapError> {
    shape(node, SCHEMA, &["Color"], &["Transparent"])?;
    let c = required(node, SCHEMA, "Color")?;
    shape(c, SCHEMA, &[], &["X", "Y", "Z", "Colorspace"])?;
    Ok(OsdColor {
        x: number(c.attr("", "X"))?,
        y: number(c.attr("", "Y"))?,
        z: number(c.attr("", "Z"))?,
        colorspace: c.attr("", "Colorspace").map(str::to_owned),
        transparent: node
            .attr("", "Transparent")
            .map(|v| {
                v.trim()
                    .parse::<i32>()
                    .map(|v| v as f32)
                    .map_err(|_| invalid("invalid OSD transparency"))
            })
            .transpose()?,
    })
}

fn parse_osd(node: &QualifiedElement) -> Result<OsdConfiguration, OnvifError> {
    shape(
        node,
        SCHEMA,
        &[
            "VideoSourceConfigurationToken",
            "Type",
            "Position",
            "TextString",
            "Image",
        ],
        &["token"],
    )?;
    let osd_token = node
        .attr("", "token")
        .ok_or_else(|| SoapError::missing("OSD token"))?;
    token(osd_token)?;
    let vsc = text(node, SCHEMA, "VideoSourceConfigurationToken")?;
    token(&vsc)?;
    let position = required(node, SCHEMA, "Position")?;
    shape(position, SCHEMA, &["Type", "Pos"], &[])?;
    let coordinates = position
        .one(SCHEMA, "Pos")?
        .map(|p| {
            shape(p, SCHEMA, &[], &["x", "y"])?;
            Ok::<_, SoapError>((number(p.attr("", "x"))?, number(p.attr("", "y"))?))
        })
        .transpose()?;
    let text_string = node
        .one(SCHEMA, "TextString")?
        .map(parse_text)
        .transpose()?;
    let image_path = node
        .one(SCHEMA, "Image")?
        .map(|image| {
            shape(image, SCHEMA, &["ImgPath"], &[])?;
            text(image, SCHEMA, "ImgPath")
        })
        .transpose()?;
    Ok(OsdConfiguration {
        token: osd_token.into(),
        video_source_config_token: vsc,
        type_: text(node, SCHEMA, "Type")?,
        position: OsdPosition {
            type_: text(position, SCHEMA, "Type")?,
            x: coordinates.map(|p| p.0),
            y: coordinates.map(|p| p.1),
        },
        text_string,
        image_path,
    })
}

fn parse_text(node: &QualifiedElement) -> Result<OsdTextString, SoapError> {
    shape(
        node,
        SCHEMA,
        &[
            "Type",
            "PlainText",
            "DateFormat",
            "TimeFormat",
            "FontSize",
            "FontColor",
            "BackgroundColor",
        ],
        &["IsPersistentText"],
    )?;
    Ok(OsdTextString {
        type_: text(node, SCHEMA, "Type")?,
        plain_text: optional(node, "PlainText")?,
        date_format: optional(node, "DateFormat")?,
        time_format: optional(node, "TimeFormat")?,
        font_size: optional(node, "FontSize")?
            .map(|value| {
                value
                    .trim()
                    .parse()
                    .map_err(|_| invalid("invalid OSD font size"))
            })
            .transpose()?,
        font_color: node.one(SCHEMA, "FontColor")?.map(color).transpose()?,
        background_color: node
            .one(SCHEMA, "BackgroundColor")?
            .map(color)
            .transpose()?,
        is_persistent_text: node.attr("", "IsPersistentText").map(boolean).transpose()?,
    })
}
