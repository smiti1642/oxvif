use crate::mock::helpers::{resp_soap_fault, soap};
use crate::mock::state::{
    OSD_QUOTA_DATE, OSD_QUOTA_DATE_AND_TIME, OSD_QUOTA_PLAIN, OSD_QUOTA_TIME, OSD_QUOTA_TOTAL,
    OsdColorEntry, OsdEntry, OsdTextEntry, ProfileEntry, SharedState,
};
use crate::mock::xml_parse::{extract_all_tags, extract_attr, extract_tag};

pub fn resp_profiles(state: &SharedState) -> String {
    let snapshot = state.read().profiles.profiles.clone();
    let items: String = snapshot
        .iter()
        .map(|p| render_profile(p, "Profiles"))
        .collect();
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!("<trt:GetProfilesResponse>{items}</trt:GetProfilesResponse>"),
    )
}

pub fn resp_profile(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "GetProfile").unwrap_or_default();
    let want = extract_tag(&inner, "ProfileToken").unwrap_or_default();
    let snapshot = state.read().profiles.profiles.clone();
    match snapshot.iter().find(|p| p.token == want) {
        Some(p) => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            &format!(
                "<trt:GetProfileResponse>{}</trt:GetProfileResponse>",
                render_profile(p, "Profile")
            ),
        ),
        None => resp_soap_fault("ter:NoProfile", &format!("Profile not found: {want}")),
    }
}

pub fn resp_stream_uri() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetStreamUriResponse>
          <trt:MediaUri>
            <tt:Uri>rtsp://127.0.0.1:554/mock/stream</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetStreamUriResponse>"#,
    )
}

pub fn resp_snapshot_uri(base: &str) -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            r#"<trt:GetSnapshotUriResponse>
          <trt:MediaUri>
            <tt:Uri>{base}/mock/snapshot.jpg</tt:Uri>
            <tt:InvalidAfterConnect>false</tt:InvalidAfterConnect>
            <tt:InvalidAfterReboot>false</tt:InvalidAfterReboot>
            <tt:Timeout>PT0S</tt:Timeout>
          </trt:MediaUri>
        </trt:GetSnapshotUriResponse>"#
        ),
    )
}

pub fn handle_create_profile(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "CreateProfile").unwrap_or_default();
    let name = extract_tag(&inner, "Name").unwrap_or_else(|| "Profile".to_string());
    // Caller may supply an explicit token (rare — most cameras assign).
    // If supplied, honour it verbatim; otherwise generate one.
    let supplied_token = extract_tag(&inner, "Token");

    // Reject duplicate token if caller supplied one that already exists.
    if let Some(t) = supplied_token.as_ref()
        && state.read().profiles.profiles.iter().any(|p| &p.token == t)
    {
        return resp_soap_fault(
            "ter:ProfileExists",
            &format!("Profile token already in use: {t}"),
        );
    }

    let entry = state.modify_returning(|s| {
        let token = supplied_token.unwrap_or_else(|| {
            let id = s.profiles.next_token_id;
            s.profiles.next_token_id += 1;
            format!("Profile_{id}")
        });
        let entry = ProfileEntry {
            token: token.clone(),
            name: name.clone(),
            fixed: false,
            video_source_config_token: None,
            video_encoder_config_token: None,
            audio_source_config_token: None,
            audio_encoder_config_token: None,
        };
        eprintln!("    [STATE] profile created: {token} ({name})");
        s.profiles.profiles.push(entry.clone());
        entry
    });

    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:CreateProfileResponse>{}</trt:CreateProfileResponse>",
            render_profile(&entry, "Profile")
        ),
    )
}

pub fn handle_delete_profile(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "DeleteProfile").unwrap_or_default();
    let token = extract_tag(&inner, "ProfileToken").unwrap_or_default();
    if token.is_empty() {
        return resp_soap_fault("ter:InvalidArgs", "ProfileToken missing");
    }

    let outcome = state.modify_returning(|s| {
        let Some(idx) = s.profiles.profiles.iter().position(|p| p.token == token) else {
            return DeleteOutcome::NotFound;
        };
        if s.profiles.profiles[idx].fixed {
            return DeleteOutcome::Fixed;
        }
        s.profiles.profiles.remove(idx);
        eprintln!("    [STATE] profile deleted: {token}");
        DeleteOutcome::Deleted
    });

    match outcome {
        DeleteOutcome::Deleted => soap(
            r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
            "<trt:DeleteProfileResponse/>",
        ),
        DeleteOutcome::NotFound => {
            resp_soap_fault("ter:NoProfile", &format!("Profile not found: {token}"))
        }
        DeleteOutcome::Fixed => resp_soap_fault(
            "ter:DeletionOfFixedProfile",
            &format!("Cannot delete fixed profile: {token}"),
        ),
    }
}

enum DeleteOutcome {
    Deleted,
    NotFound,
    /// Per ONVIF spec, fixed profiles can't be removed.
    Fixed,
}

// ── Profile render helpers ──────────────────────────────────────────────────
//
// Profiles are rendered with full nested configuration objects (the
// shape real cameras use). The configuration details for VSC_1, VEC_1,
// VEC_2 are hardcoded here rather than stored in state — only the
// *attachment* (which token a profile is bound to) is mutable, which
// matches what `CreateProfile` / `AddVideoEncoderConfiguration` etc.
// actually mutate on a real camera.

fn render_profile(p: &ProfileEntry, tag: &str) -> String {
    let vsc = p
        .video_source_config_token
        .as_deref()
        .map(render_vsc_inline)
        .unwrap_or_default();
    let vec = p
        .video_encoder_config_token
        .as_deref()
        .map(render_vec_inline)
        .unwrap_or_default();
    let asc = p
        .audio_source_config_token
        .as_deref()
        .map(render_asc_inline)
        .unwrap_or_default();
    let aec = p
        .audio_encoder_config_token
        .as_deref()
        .map(render_aec_inline)
        .unwrap_or_default();
    format!(
        r#"<trt:{tag} token="{token}" fixed="{fixed}">
          <tt:Name>{name}</tt:Name>
          {vsc}{vec}{asc}{aec}
        </trt:{tag}>"#,
        token = p.token,
        fixed = p.fixed,
        name = p.name,
    )
}

fn render_vsc_inline(token: &str) -> String {
    match token {
        "VSC_1" => r#"<tt:VideoSourceConfiguration token="VSC_1">
          <tt:Name>VSConfig1</tt:Name>
          <tt:UseCount>2</tt:UseCount>
          <tt:SourceToken>VS_1</tt:SourceToken>
          <tt:Bounds x="0" y="0" width="1920" height="1080"/>
        </tt:VideoSourceConfiguration>"#
            .to_string(),
        _ => String::new(),
    }
}

fn render_vec_inline(token: &str) -> String {
    match token {
        "VEC_1" => r#"<tt:VideoEncoderConfiguration token="VEC_1">
          <tt:Name>H264</tt:Name>
          <tt:UseCount>1</tt:UseCount>
          <tt:Encoding>H264</tt:Encoding>
          <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
          <tt:RateControl><tt:FrameRateLimit>30</tt:FrameRateLimit><tt:BitrateLimit>4096</tt:BitrateLimit></tt:RateControl>
        </tt:VideoEncoderConfiguration>"#
            .to_string(),
        "VEC_2" => r#"<tt:VideoEncoderConfiguration token="VEC_2">
          <tt:Name>H264_sub</tt:Name>
          <tt:UseCount>1</tt:UseCount>
          <tt:Encoding>H264</tt:Encoding>
          <tt:Resolution><tt:Width>640</tt:Width><tt:Height>480</tt:Height></tt:Resolution>
          <tt:RateControl><tt:FrameRateLimit>15</tt:FrameRateLimit><tt:BitrateLimit>1024</tt:BitrateLimit></tt:RateControl>
        </tt:VideoEncoderConfiguration>"#
            .to_string(),
        _ => String::new(),
    }
}

fn render_asc_inline(token: &str) -> String {
    match token {
        "ASC_1" => r#"<tt:AudioSourceConfiguration token="ASC_1">
          <tt:Name>AudioSourceConfig1</tt:Name>
          <tt:UseCount>1</tt:UseCount>
          <tt:SourceToken>AudioSource_1</tt:SourceToken>
        </tt:AudioSourceConfiguration>"#
            .to_string(),
        _ => String::new(),
    }
}

fn render_aec_inline(token: &str) -> String {
    match token {
        "AEC_1" => r#"<tt:AudioEncoderConfiguration token="AEC_1">
          <tt:Name>AudioEncoder</tt:Name>
          <tt:UseCount>1</tt:UseCount>
          <tt:Encoding>G711</tt:Encoding>
          <tt:Bitrate>64</tt:Bitrate>
          <tt:SampleRate>8</tt:SampleRate>
        </tt:AudioEncoderConfiguration>"#
            .to_string(),
        _ => String::new(),
    }
}

pub fn resp_video_sources() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourcesResponse>
          <trt:VideoSources token="VS_1">
            <tt:Framerate>25</tt:Framerate>
            <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
          </trt:VideoSources>
        </trt:GetVideoSourcesResponse>"#,
    )
}

pub fn resp_video_source_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourceConfigurationsResponse>
          <trt:Configurations token="VSC_1">
            <tt:Name>VSConfig1</tt:Name>
            <tt:UseCount>2</tt:UseCount>
            <tt:SourceToken>VS_1</tt:SourceToken>
            <tt:Bounds x="0" y="0" width="1920" height="1080"/>
          </trt:Configurations>
        </trt:GetVideoSourceConfigurationsResponse>"#,
    )
}

pub fn resp_video_encoder_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoEncoderConfigurationsResponse>
          <trt:Configurations token="VEC_1">
            <tt:Name>MainStream</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>H264</tt:Encoding>
            <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
            <tt:Quality>5</tt:Quality>
            <tt:RateControl>
              <tt:FrameRateLimit>25</tt:FrameRateLimit>
              <tt:EncodingInterval>1</tt:EncodingInterval>
              <tt:BitrateLimit>4096</tt:BitrateLimit>
            </tt:RateControl>
            <tt:H264>
              <tt:GovLength>25</tt:GovLength>
              <tt:H264Profile>Main</tt:H264Profile>
            </tt:H264>
          </trt:Configurations>
          <trt:Configurations token="VEC_2">
            <tt:Name>SubStream</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>JPEG</tt:Encoding>
            <tt:Resolution><tt:Width>640</tt:Width><tt:Height>480</tt:Height></tt:Resolution>
            <tt:Quality>3</tt:Quality>
          </trt:Configurations>
        </trt:GetVideoEncoderConfigurationsResponse>"#,
    )
}

pub fn resp_audio_sources() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioSourcesResponse>
          <trt:AudioSources token="AudioSource_1">
            <tt:Channels>1</tt:Channels>
          </trt:AudioSources>
        </trt:GetAudioSourcesResponse>"#,
    )
}

pub fn resp_audio_encoder_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationsResponse>
          <trt:Configurations token="AEC_1">
            <tt:Name>AudioEncoder</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>G711</tt:Encoding>
            <tt:Bitrate>64</tt:Bitrate>
            <tt:SampleRate>8</tt:SampleRate>
          </trt:Configurations>
        </trt:GetAudioEncoderConfigurationsResponse>"#,
    )
}

pub fn resp_osds(state: &SharedState, body: &str) -> String {
    // Optional <ConfigurationToken> filter — only return OSDs attached
    // to that VSC. Real cameras vary on whether they apply this filter
    // strictly; we honour it when present, return all when absent.
    let inner = extract_tag(body, "GetOSDs").unwrap_or_default();
    let filter = extract_tag(&inner, "ConfigurationToken");

    let snapshot = state.read().osd.osds.clone();
    let items: String = snapshot
        .iter()
        .filter(|o| {
            filter
                .as_deref()
                .is_none_or(|t| o.video_source_config_token == t)
        })
        .map(render_osd_entry)
        .collect();

    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!("<trt:GetOSDsResponse>{items}</trt:GetOSDsResponse>"),
    )
}

pub fn resp_video_source_configuration() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourceConfigurationResponse>
          <trt:Configuration token="VSC_1">
            <tt:Name>VSConfig1</tt:Name>
            <tt:UseCount>2</tt:UseCount>
            <tt:SourceToken>VS_1</tt:SourceToken>
            <tt:Bounds x="0" y="0" width="1920" height="1080"/>
          </trt:Configuration>
        </trt:GetVideoSourceConfigurationResponse>"#,
    )
}

pub fn resp_video_source_configuration_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoSourceConfigurationOptionsResponse>
          <trt:Options>
            <tt:MaximumNumberOfProfiles>5</tt:MaximumNumberOfProfiles>
            <tt:BoundsRange>
              <tt:XRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:XRange>
              <tt:YRange><tt:Min>0</tt:Min><tt:Max>0</tt:Max></tt:YRange>
              <tt:WidthRange><tt:Min>160</tt:Min><tt:Max>1920</tt:Max></tt:WidthRange>
              <tt:HeightRange><tt:Min>90</tt:Min><tt:Max>1080</tt:Max></tt:HeightRange>
            </tt:BoundsRange>
            <tt:VideoSourceTokensAvailable>VS_1</tt:VideoSourceTokensAvailable>
          </trt:Options>
        </trt:GetVideoSourceConfigurationOptionsResponse>"#,
    )
}

pub fn resp_video_encoder_configuration() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoEncoderConfigurationResponse>
          <trt:Configuration token="VEC_1">
            <tt:Name>MainStream</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>H264</tt:Encoding>
            <tt:Resolution><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:Resolution>
            <tt:Quality>5</tt:Quality>
            <tt:RateControl>
              <tt:FrameRateLimit>25</tt:FrameRateLimit>
              <tt:EncodingInterval>1</tt:EncodingInterval>
              <tt:BitrateLimit>4096</tt:BitrateLimit>
            </tt:RateControl>
            <tt:H264>
              <tt:GovLength>25</tt:GovLength>
              <tt:H264Profile>Main</tt:H264Profile>
            </tt:H264>
          </trt:Configuration>
        </trt:GetVideoEncoderConfigurationResponse>"#,
    )
}

pub fn resp_video_encoder_configuration_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetVideoEncoderConfigurationOptionsResponse>
          <trt:Options>
            <tt:QualityRange><tt:Min>0</tt:Min><tt:Max>10</tt:Max></tt:QualityRange>
            <tt:H264>
              <tt:ResolutionsAvailable><tt:Width>1920</tt:Width><tt:Height>1080</tt:Height></tt:ResolutionsAvailable>
              <tt:ResolutionsAvailable><tt:Width>1280</tt:Width><tt:Height>720</tt:Height></tt:ResolutionsAvailable>
              <tt:GovLengthRange><tt:Min>1</tt:Min><tt:Max>300</tt:Max></tt:GovLengthRange>
              <tt:FrameRateRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:FrameRateRange>
              <tt:EncodingIntervalRange><tt:Min>1</tt:Min><tt:Max>30</tt:Max></tt:EncodingIntervalRange>
              <tt:BitrateRange><tt:Min>64</tt:Min><tt:Max>16384</tt:Max></tt:BitrateRange>
              <tt:H264ProfilesSupported>Baseline</tt:H264ProfilesSupported>
              <tt:H264ProfilesSupported>Main</tt:H264ProfilesSupported>
              <tt:H264ProfilesSupported>High</tt:H264ProfilesSupported>
            </tt:H264>
          </trt:Options>
        </trt:GetVideoEncoderConfigurationOptionsResponse>"#,
    )
}

pub fn resp_osd(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "GetOSD").unwrap_or_default();
    let want = extract_tag(&inner, "OSDToken").unwrap_or_default();

    let snapshot = state.read().osd.osds.clone();
    match snapshot.iter().find(|o| o.token == want) {
        Some(entry) => {
            let body = format!(
                "<trt:GetOSDResponse>{}</trt:GetOSDResponse>",
                render_osd_entry(entry)
            )
            .replace("<trt:OSDs ", "<trt:OSDConfiguration ")
            .replace("</trt:OSDs>", "</trt:OSDConfiguration>");
            soap(
                r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
                &body,
            )
        }
        None => resp_soap_fault("ter:InvalidArgs", &format!("OSD not found: {want}")),
    }
}

pub fn handle_create_osd(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "OSD").unwrap_or_default();
    let parsed = match parse_osd_payload(&inner) {
        Ok(p) => p,
        Err(e) => return resp_soap_fault("ter:InvalidArgs", &e),
    };

    // Quota enforcement — match what GetOSDOptions advertises so the
    // mock surfaces "DateAndTime full" the same way Genetec does.
    if let Some(text) = parsed.text.as_ref() {
        let snapshot = state.read().osd.osds.clone();
        let used_total = snapshot.len() as u32;
        let used_for_type = snapshot
            .iter()
            .filter(|o| {
                o.text
                    .as_ref()
                    .is_some_and(|t| t.text_type == text.text_type)
            })
            .count() as u32;
        let limit = match text.text_type.as_str() {
            "Plain" => OSD_QUOTA_PLAIN,
            "Date" => OSD_QUOTA_DATE,
            "Time" => OSD_QUOTA_TIME,
            "DateAndTime" => OSD_QUOTA_DATE_AND_TIME,
            _ => OSD_QUOTA_TOTAL,
        };
        if used_for_type >= limit {
            return resp_soap_fault(
                "ter:InvalidArgs",
                &format!(
                    "Per-type OSD quota exceeded: {}={used_for_type}/{limit}",
                    text.text_type
                ),
            );
        }
        if used_total >= OSD_QUOTA_TOTAL {
            return resp_soap_fault(
                "ter:InvalidArgs",
                &format!("Total OSD quota exceeded: {used_total}/{OSD_QUOTA_TOTAL}"),
            );
        }
    }

    let token = state.modify_returning(|s| {
        let id = s.osd.next_token_id;
        s.osd.next_token_id += 1;
        let token = format!("OSD_{id}");
        let mut entry = parsed;
        entry.token = token.clone();
        eprintln!(
            "    [STATE] OSD created: {token} (vsc={}, type={})",
            entry.video_source_config_token, entry.osd_type
        );
        s.osd.osds.push(entry);
        token
    });

    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            "<trt:CreateOSDResponse><trt:OSDToken>{token}</trt:OSDToken></trt:CreateOSDResponse>"
        ),
    )
}

pub fn handle_set_osd(state: &SharedState, body: &str) -> String {
    // Token sits on the outer `<trt:OSD token="...">` tag, not inside
    // its body — so pull from `body`, not the extracted inner.
    let token = extract_attr(body, "OSD", "token").unwrap_or_default();
    let inner = extract_tag(body, "OSD").unwrap_or_default();
    if token.is_empty() {
        return resp_soap_fault("ter:InvalidArgs", "OSD token missing");
    }
    let parsed = match parse_osd_payload(&inner) {
        Ok(p) => p,
        Err(e) => return resp_soap_fault("ter:InvalidArgs", &e),
    };

    let updated = state.modify_returning(|s| {
        if let Some(existing) = s.osd.osds.iter_mut().find(|o| o.token == token) {
            // Token + vsc are immutable on Set; everything else is replaced.
            let vsc = existing.video_source_config_token.clone();
            *existing = OsdEntry {
                token: token.clone(),
                video_source_config_token: vsc,
                ..parsed
            };
            eprintln!("    [STATE] OSD updated: {token}");
            true
        } else {
            false
        }
    });

    if !updated {
        return resp_soap_fault("ter:InvalidArgs", &format!("OSD not found: {token}"));
    }
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        "<trt:SetOSDResponse/>",
    )
}

pub fn handle_delete_osd(state: &SharedState, body: &str) -> String {
    let inner = extract_tag(body, "DeleteOSD").unwrap_or_default();
    let token = extract_tag(&inner, "OSDToken").unwrap_or_default();

    let removed = state.modify_returning(|s| {
        let before = s.osd.osds.len();
        s.osd.osds.retain(|o| o.token != token);
        let removed = before > s.osd.osds.len();
        if removed {
            eprintln!("    [STATE] OSD deleted: {token}");
        }
        removed
    });

    if !removed {
        return resp_soap_fault("ter:InvalidArgs", &format!("OSD not found: {token}"));
    }
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        "<trt:DeleteOSDResponse/>",
    )
}

/// `GetOSDOptions` advertises per-text-type quotas via XML attributes
/// on `<MaximumNumberOfOSDs>`. This is the Genetec/late-Hikvision shape;
/// `oxvif::OnvifSession::get_osd_options` parses the attributes (the
/// strict `OnvifClient` ignores them, by design).
pub fn resp_osd_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        &format!(
            r#"<trt:GetOSDOptionsResponse>
          <trt:OSDOptions>
            <tt:MaximumNumberOfOSDs Total="{OSD_QUOTA_TOTAL}" Plain="{OSD_QUOTA_PLAIN}" Date="{OSD_QUOTA_DATE}" Time="{OSD_QUOTA_TIME}" DateAndTime="{OSD_QUOTA_DATE_AND_TIME}"/>
            <tt:Type>Text</tt:Type>
            <tt:Type>Image</tt:Type>
            <tt:PositionOption>
              <tt:Type>UpperLeft</tt:Type>
              <tt:Type>UpperRight</tt:Type>
              <tt:Type>LowerLeft</tt:Type>
              <tt:Type>LowerRight</tt:Type>
              <tt:Type>Custom</tt:Type>
            </tt:PositionOption>
            <tt:TextOption>
              <tt:Type>Plain</tt:Type>
              <tt:Type>Date</tt:Type>
              <tt:Type>Time</tt:Type>
              <tt:Type>DateAndTime</tt:Type>
              <tt:DateFormat>MM/dd/yyyy</tt:DateFormat>
              <tt:DateFormat>yyyy-MM-dd</tt:DateFormat>
              <tt:DateFormat>dd.MM.yyyy</tt:DateFormat>
              <tt:TimeFormat>HH:mm:ss</tt:TimeFormat>
              <tt:TimeFormat>hh:mm:ss tt</tt:TimeFormat>
              <tt:FontSizeRange>
                <tt:Min>8</tt:Min>
                <tt:Max>72</tt:Max>
              </tt:FontSizeRange>
            </tt:TextOption>
          </trt:OSDOptions>
        </trt:GetOSDOptionsResponse>"#
        ),
    )
}

// ── OSD render / parse helpers ──────────────────────────────────────────────

fn render_osd_entry(o: &OsdEntry) -> String {
    let pos_xy = match (o.position_x, o.position_y) {
        (Some(x), Some(y)) => format!(r#"<tt:Pos x="{x}" y="{y}"/>"#),
        _ => String::new(),
    };
    let text_el = o.text.as_ref().map(render_osd_text).unwrap_or_default();
    let image_el = o
        .image_path
        .as_deref()
        .map(|p| format!("<tt:ImgPath>{p}</tt:ImgPath>"))
        .unwrap_or_default();
    format!(
        r#"<trt:OSDs token="{token}">
          <tt:VideoSourceConfigurationToken>{vsc}</tt:VideoSourceConfigurationToken>
          <tt:Type>{ty}</tt:Type>
          <tt:Position>
            <tt:Type>{pos_type}</tt:Type>
            {pos_xy}
          </tt:Position>
          {text_el}{image_el}
        </trt:OSDs>"#,
        token = o.token,
        vsc = o.video_source_config_token,
        ty = o.osd_type,
        pos_type = o.position_type,
    )
}

fn render_osd_text(t: &OsdTextEntry) -> String {
    let plain = t
        .plain_text
        .as_deref()
        .map(|s| format!("<tt:PlainText>{s}</tt:PlainText>"))
        .unwrap_or_default();
    let date = t
        .date_format
        .as_deref()
        .map(|s| format!("<tt:DateFormat>{s}</tt:DateFormat>"))
        .unwrap_or_default();
    let time = t
        .time_format
        .as_deref()
        .map(|s| format!("<tt:TimeFormat>{s}</tt:TimeFormat>"))
        .unwrap_or_default();
    let font = t
        .font_size
        .map(|n| format!("<tt:FontSize>{n}</tt:FontSize>"))
        .unwrap_or_default();
    let color = t
        .font_color
        .as_ref()
        .map(|c| {
            let cs = c
                .colorspace
                .as_deref()
                .map(|s| format!(r#" Colorspace="{s}""#))
                .unwrap_or_default();
            let trans = c
                .transparent
                .map(|v| format!("<tt:Transparent>{v}</tt:Transparent>"))
                .unwrap_or_default();
            format!(
                r#"<tt:FontColor><tt:Color X="{x}" Y="{y}" Z="{z}"{cs}/>{trans}</tt:FontColor>"#,
                x = c.x,
                y = c.y,
                z = c.z,
            )
        })
        .unwrap_or_default();
    format!(
        r#"<tt:TextString><tt:Type>{ty}</tt:Type>{plain}{date}{time}{font}{color}</tt:TextString>"#,
        ty = t.text_type,
    )
}

/// Parse an `<trt:OSD>` payload into an `OsdEntry`. The token is left
/// blank — `handle_create_osd` fills it in from `next_token_id`,
/// `handle_set_osd` keeps the existing token.
fn parse_osd_payload(inner: &str) -> Result<OsdEntry, String> {
    let vsc = extract_tag(inner, "VideoSourceConfigurationToken")
        .ok_or_else(|| "VideoSourceConfigurationToken missing".to_string())?;
    let ty = extract_tag(inner, "Type").unwrap_or_else(|| "Text".to_string());

    let pos = extract_tag(inner, "Position").unwrap_or_default();
    let position_type = extract_tag(&pos, "Type").unwrap_or_else(|| "UpperLeft".to_string());
    let position_x = extract_attr(&pos, "Pos", "x").and_then(|s| s.parse().ok());
    let position_y = extract_attr(&pos, "Pos", "y").and_then(|s| s.parse().ok());

    let text = if ty == "Text" {
        let ts = extract_tag(inner, "TextString").unwrap_or_default();
        let text_type = extract_tag(&ts, "Type").unwrap_or_else(|| "Plain".to_string());
        Some(OsdTextEntry {
            text_type,
            plain_text: extract_tag(&ts, "PlainText"),
            date_format: extract_tag(&ts, "DateFormat"),
            time_format: extract_tag(&ts, "TimeFormat"),
            font_size: extract_tag(&ts, "FontSize").and_then(|s| s.parse().ok()),
            font_color: parse_osd_color(&ts),
        })
    } else {
        None
    };

    let image_path = if ty == "Image" {
        extract_tag(inner, "ImgPath")
    } else {
        None
    };

    Ok(OsdEntry {
        token: String::new(),
        video_source_config_token: vsc,
        osd_type: ty,
        position_type,
        position_x,
        position_y,
        text,
        image_path,
    })
}

fn parse_osd_color(text_string: &str) -> Option<OsdColorEntry> {
    let fc = extract_tag(text_string, "FontColor")?;
    let x: f32 = extract_attr(&fc, "Color", "X")?.parse().ok()?;
    let y: f32 = extract_attr(&fc, "Color", "Y")?.parse().ok()?;
    let z: f32 = extract_attr(&fc, "Color", "Z")?.parse().ok()?;
    let colorspace = extract_attr(&fc, "Color", "Colorspace");
    let transparent = extract_tag(&fc, "Transparent").and_then(|s| s.parse().ok());
    Some(OsdColorEntry {
        x,
        y,
        z,
        colorspace,
        transparent,
    })
}

// `extract_all_tags` is currently unused but reserved for future Image-OSD
// support that may need to read multiple `<ImgPath>` siblings. Suppress the
// warning rather than removing the import — keeping it discoverable in tree.
#[allow(dead_code)]
fn _force_use_extract_all() {
    let _ = extract_all_tags("", "");
}

pub fn resp_audio_source_configurations() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioSourceConfigurationsResponse>
          <trt:Configurations token="ASC_1">
            <tt:Name>AudioSourceConfig1</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:SourceToken>AudioSource_1</tt:SourceToken>
          </trt:Configurations>
        </trt:GetAudioSourceConfigurationsResponse>"#,
    )
}

pub fn resp_audio_encoder_configuration() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationResponse>
          <trt:Configuration token="AEC_1">
            <tt:Name>AudioEncoder</tt:Name>
            <tt:UseCount>1</tt:UseCount>
            <tt:Encoding>G711</tt:Encoding>
            <tt:Bitrate>64</tt:Bitrate>
            <tt:SampleRate>8</tt:SampleRate>
          </trt:Configuration>
        </trt:GetAudioEncoderConfigurationResponse>"#,
    )
}

pub fn resp_audio_encoder_configuration_options() -> String {
    soap(
        r#"xmlns:trt="http://www.onvif.org/ver10/media/wsdl""#,
        r#"<trt:GetAudioEncoderConfigurationOptionsResponse>
          <trt:Options>
            <tt:Encoding>G711</tt:Encoding>
            <tt:BitrateList><tt:Items>64</tt:Items></tt:BitrateList>
            <tt:SampleRateList><tt:Items>8</tt:Items></tt:SampleRateList>
          </trt:Options>
          <trt:Options>
            <tt:Encoding>AAC</tt:Encoding>
            <tt:BitrateList><tt:Items>64 128 256</tt:Items></tt:BitrateList>
            <tt:SampleRateList><tt:Items>16 32 44</tt:Items></tt:SampleRateList>
          </trt:Options>
        </trt:GetAudioEncoderConfigurationOptionsResponse>"#,
    )
}
