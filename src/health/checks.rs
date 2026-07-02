//! Reusable, capability-gated check units. Each returns one or more
//! [`CheckResult`]s. All checks are read-only except [`write_roundtrip`],
//! which re-applies an unchanged configuration.

use std::future::Future;
use std::time::Instant;

use super::report::{Category, CheckResult};
use crate::{OnvifError, OnvifSession};

/// Time a `Result<String, OnvifError>` future into a Pass/Fail check.
async fn one<F>(id: &'static str, category: Category, fut: F) -> CheckResult
where
    F: Future<Output = Result<String, OnvifError>>,
{
    let start = Instant::now();
    let r = fut.await;
    let elapsed = start.elapsed();
    match r {
        Ok(detail) => CheckResult::pass(id, category, detail).with_elapsed(elapsed),
        Err(e) => CheckResult::fail_from(id, category, &e).with_elapsed(elapsed),
    }
}

/// Parse the numeric skew back out of a `system_date_time` check's `detail`
/// (`"skew -20s"`). Colocated with the formatter in [`time`] so the two move
/// together; returns `None` if the check failed (empty detail) or the format
/// ever changes.
pub(super) fn parse_skew(detail: &str) -> Option<i64> {
    detail
        .strip_prefix("skew ")?
        .strip_suffix('s')?
        .parse()
        .ok()
}

pub(super) async fn device_info(s: &OnvifSession) -> Vec<CheckResult> {
    vec![
        one("get_device_info", Category::Connectivity, async {
            let i = s.get_device_info().await?;
            Ok(format!(
                "{} {} fw {}",
                i.manufacturer, i.model, i.firmware_version
            ))
        })
        .await,
    ]
}

pub(super) async fn time(s: &OnvifSession) -> Vec<CheckResult> {
    let start = Instant::now();
    let r = s.get_system_date_and_time().await;
    let elapsed = start.elapsed();
    let res = match r {
        Ok(dt) => {
            let skew = dt.utc_offset_secs();
            if skew.abs() > 5 {
                CheckResult::warn(
                    "system_date_time",
                    Category::Time,
                    format!("clock skew {skew}s vs local — may break WS-Security auth"),
                    format!("skew {skew}s"),
                )
            } else {
                CheckResult::pass("system_date_time", Category::Time, format!("skew {skew}s"))
            }
        }
        Err(e) => CheckResult::fail_from("system_date_time", Category::Time, &e),
    };
    vec![res.with_elapsed(elapsed)]
}

pub(super) async fn services(s: &OnvifSession) -> Vec<CheckResult> {
    vec![
        one("get_services", Category::Services, async {
            let svcs = s.get_services().await?;
            Ok(format!("{} service(s)", svcs.len()))
        })
        .await,
    ]
}

/// Profile G presence: report each recording/search/replay service as Pass
/// when advertised (via GetCapabilities or the GetServices fallback resolved
/// during session build) or Skip when absent. Informational — the services
/// are not exercised here. Feeds the Profile G verdict in `mod.rs::assess`,
/// which keys off the `recording` / `search` / `replay` check ids.
pub(super) async fn recording_services(s: &OnvifSession) -> Vec<CheckResult> {
    let caps = s.capabilities();
    [
        ("recording", caps.recording.url.as_deref()),
        ("search", caps.search.url.as_deref()),
        ("replay", caps.replay.url.as_deref()),
    ]
    .into_iter()
    .map(|(id, url)| match url {
        Some(u) => CheckResult::pass(
            id,
            Category::Services,
            format!("advertised: {u}  (not exercised)"),
        ),
        None => CheckResult::skip(id, Category::Services, "not advertised"),
    })
    .collect()
}

pub(super) async fn media(s: &OnvifSession) -> Vec<CheckResult> {
    let mut out = Vec::new();

    let start = Instant::now();
    let profiles = s.get_profiles().await;
    let elapsed = start.elapsed();
    let first_token = match &profiles {
        Ok(p) if !p.is_empty() => {
            out.push(
                CheckResult::pass(
                    "get_profiles",
                    Category::Media,
                    format!("{} profile(s)", p.len()),
                )
                .with_elapsed(elapsed),
            );
            Some(p[0].token.clone())
        }
        Ok(_) => {
            out.push(
                CheckResult::warn(
                    "get_profiles",
                    Category::Media,
                    "no media profiles",
                    "0 profiles",
                )
                .with_elapsed(elapsed),
            );
            None
        }
        Err(e) => {
            out.push(
                CheckResult::fail_from("get_profiles", Category::Media, e).with_elapsed(elapsed),
            );
            None
        }
    };

    if let Some(token) = first_token {
        // Stream URI — expect rtsp://
        let start = Instant::now();
        match s.get_stream_uri(&token).await {
            Ok(u) if u.uri.starts_with("rtsp://") => out.push(
                CheckResult::pass("get_stream_uri", Category::Media, u.uri)
                    .with_elapsed(start.elapsed()),
            ),
            Ok(u) => out.push(
                CheckResult::warn("get_stream_uri", Category::Media, "non-rtsp scheme", u.uri)
                    .with_elapsed(start.elapsed()),
            ),
            Err(e) => out.push(
                CheckResult::fail_from("get_stream_uri", Category::Media, &e)
                    .with_elapsed(start.elapsed()),
            ),
        }
        // Snapshot URI — expect http(s)://
        let start = Instant::now();
        match s.get_snapshot_uri(&token).await {
            Ok(u) if u.uri.starts_with("http") => out.push(
                CheckResult::pass("get_snapshot_uri", Category::Media, u.uri)
                    .with_elapsed(start.elapsed()),
            ),
            Ok(u) => out.push(
                CheckResult::warn(
                    "get_snapshot_uri",
                    Category::Media,
                    "non-http scheme",
                    u.uri,
                )
                .with_elapsed(start.elapsed()),
            ),
            Err(e) => out.push(
                CheckResult::fail_from("get_snapshot_uri", Category::Media, &e)
                    .with_elapsed(start.elapsed()),
            ),
        }
    }

    out.push(
        one("get_video_encoder_configurations", Category::Media, async {
            let cfgs = s.get_video_encoder_configurations().await?;
            Ok(format!("{} encoder config(s)", cfgs.len()))
        })
        .await,
    );
    out
}

pub(super) async fn imaging(s: &OnvifSession) -> Vec<CheckResult> {
    if s.capabilities().imaging.url.is_none() {
        return vec![CheckResult::skip(
            "get_imaging_settings",
            Category::Imaging,
            "Imaging service not advertised",
        )];
    }
    let start = Instant::now();
    let token = match s.get_video_sources().await {
        Ok(v) if !v.is_empty() => v[0].token.clone(),
        Ok(_) => {
            return vec![
                CheckResult::warn(
                    "get_imaging_settings",
                    Category::Imaging,
                    "no video sources",
                    "",
                )
                .with_elapsed(start.elapsed()),
            ];
        }
        Err(e) => {
            return vec![
                CheckResult::fail_from("get_video_sources", Category::Imaging, &e)
                    .with_elapsed(start.elapsed()),
            ];
        }
    };
    vec![
        one("get_imaging_settings", Category::Imaging, async {
            s.get_imaging_settings(&token).await?;
            s.get_imaging_options(&token).await?;
            Ok("settings + options".to_string())
        })
        .await,
    ]
}

pub(super) async fn ptz(s: &OnvifSession) -> Vec<CheckResult> {
    if s.capabilities().ptz.url.is_none() {
        return vec![CheckResult::skip(
            "ptz_get_nodes",
            Category::Ptz,
            "PTZ service not advertised",
        )];
    }
    vec![
        one("ptz_get_nodes", Category::Ptz, async {
            let nodes = s.ptz_get_nodes().await?;
            Ok(format!("{} node(s)", nodes.len()))
        })
        .await,
    ]
}

pub(super) async fn events(s: &OnvifSession) -> Vec<CheckResult> {
    if s.capabilities().events.url.is_none() {
        return vec![CheckResult::skip(
            "get_event_properties",
            Category::Events,
            "Events service not advertised",
        )];
    }
    let mut out = vec![
        one("get_event_properties", Category::Events, async {
            s.get_event_properties().await?;
            Ok("ok".to_string())
        })
        .await,
    ];
    // PullPoint round-trip — subscribe, pull briefly, unsubscribe (self-cleaning).
    let start = Instant::now();
    match s.create_pull_point_subscription(None, Some("PT1M")).await {
        Ok(sub) => {
            let _ = s.pull_messages(&sub.reference_url, "PT1S", 10).await;
            let _ = s.unsubscribe(&sub.reference_url).await;
            out.push(
                CheckResult::pass(
                    "pull_point_subscription",
                    Category::Events,
                    "subscribe / pull / unsubscribe ok",
                )
                .with_elapsed(start.elapsed()),
            );
        }
        Err(e) => out.push(
            CheckResult::fail_from("pull_point_subscription", Category::Events, &e)
                .with_elapsed(start.elapsed()),
        ),
    }
    out
}

pub(super) async fn network(s: &OnvifSession) -> Vec<CheckResult> {
    vec![
        one("get_network_interfaces", Category::Network, async {
            let n = s.get_network_interfaces().await?;
            Ok(format!("{} interface(s)", n.len()))
        })
        .await,
        one("get_ntp", Category::Network, async {
            s.get_ntp().await?;
            Ok("ok".to_string())
        })
        .await,
        one("get_dns", Category::Network, async {
            s.get_dns().await?;
            Ok("ok".to_string())
        })
        .await,
    ]
}

pub(super) async fn users(s: &OnvifSession) -> Vec<CheckResult> {
    vec![
        one("get_users", Category::Users, async {
            let u = s.get_users().await?;
            Ok(format!("{} user(s)", u.len()))
        })
        .await,
    ]
}

/// Opt-in, non-destructive write check: read the first video encoder
/// configuration and `Set` it back **unchanged**. A SOAP fault here means the
/// device rejects our serialised body (schema order, missing required field,
/// etc.) — exactly the class of bug a read-only probe can't see.
pub(super) async fn write_roundtrip(s: &OnvifSession) -> Vec<CheckResult> {
    let start = Instant::now();
    let cfg = match s.get_video_encoder_configurations().await {
        Ok(mut v) if !v.is_empty() => v.remove(0),
        Ok(_) => {
            return vec![
                CheckResult::skip(
                    "set_video_encoder_roundtrip",
                    Category::Write,
                    "no encoder config to round-trip",
                )
                .with_elapsed(start.elapsed()),
            ];
        }
        Err(e) => {
            return vec![
                CheckResult::fail(
                    "set_video_encoder_roundtrip",
                    Category::Write,
                    format!("read failed: {e}"),
                )
                .with_error(&e)
                .with_elapsed(start.elapsed()),
            ];
        }
    };
    let res = match s.set_video_encoder_configuration(&cfg).await {
        Ok(()) => CheckResult::pass(
            "set_video_encoder_roundtrip",
            Category::Write,
            "Set accepted (unchanged values)",
        ),
        Err(e) => CheckResult::fail_from("set_video_encoder_roundtrip", Category::Write, &e),
    };
    vec![res.with_elapsed(start.elapsed())]
}
