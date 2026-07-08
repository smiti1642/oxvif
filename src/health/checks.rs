//! Reusable, capability-gated check units. Each returns one or more
//! [`CheckResult`]s.
//!
//! Most checks are read-only, but a few actively touch the device:
//! [`events`]'s pull-point round-trip subscribes / pulls / unsubscribes
//! (self-cleaning); the opt-in [`write_roundtrip`] re-applies an unchanged
//! configuration; and when liveness probing is enabled, [`media`] opens an
//! RTSP `OPTIONS` connection + fetches a snapshot, and [`recording_services`]
//! exercises the real recording-search / replay operations.

use std::future::Future;
use std::time::{Duration, Instant};

use super::report::{Category, CheckError, CheckResult};
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

/// Non-destructive RTSP reachability probe: open a TCP connection to the stream
/// endpoint and send an `OPTIONS` request. A resolved stream URI is no guarantee
/// the RTSP server actually answers; `200` is ideal and `401` still proves the
/// server is alive (it just wants auth), so both count as reachable. Read-only —
/// never issues `DESCRIBE` / `SETUP` / `PLAY`. IPv4/hostname authorities only.
async fn rtsp_options_probe(rtsp_url: &str) -> Result<(), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    let authority = rtsp_url
        .strip_prefix("rtsp://")
        .ok_or("not an rtsp:// url")?
        .split('/')
        .next()
        .unwrap_or("");
    // Drop any userinfo, then split host:port (default 554).
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(554)),
        None => (hostport, 554u16),
    };
    if host.is_empty() {
        return Err("empty host".to_string());
    }

    let mut stream = timeout(Duration::from_secs(5), TcpStream::connect((host, port)))
        .await
        .map_err(|_| "connect timed out".to_string())?
        .map_err(|e| format!("connect failed: {e}"))?;

    let req = format!(
        "OPTIONS {rtsp_url} RTSP/1.0\r\nCSeq: 1\r\nUser-Agent: oxvif\r\nAccept: */*\r\n\r\n"
    );
    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("write failed: {e}"))?;

    let mut buf = [0u8; 256];
    let n = timeout(Duration::from_secs(5), stream.read(&mut buf))
        .await
        .map_err(|_| "read timed out".to_string())?
        .map_err(|e| format!("read failed: {e}"))?;

    let head = String::from_utf8_lossy(&buf[..n]);
    let status = head.lines().next().unwrap_or("").trim();
    if status.contains(" 200") || status.contains(" 401") {
        Ok(())
    } else {
        Err(format!("OPTIONS refused: {status}"))
    }
}

/// Fetch the snapshot URI and confirm the body is a real image. Returns the byte
/// count on success. Tries HTTP Digest (via `diqwest`, the same path the SOAP
/// transport uses) then falls back to Basic auth when credentials are supplied.
/// A `200` carrying an HTML error page or a 0-byte body — a common firmware quirk
/// — is rejected here rather than counted as a passing snapshot.
async fn fetch_snapshot(uri: &str, creds: Option<&(String, String)>) -> Result<usize, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("client build failed: {e}"))?;

    let resp = match creds {
        Some((u, p)) => {
            use diqwest::WithDigestAuth as _;
            let session = diqwest::DigestAuthSession::new(u.clone(), p.clone());
            match client.get(uri).send_digest_auth(&session).await {
                Ok(r) if r.status().is_success() => r,
                // Digest failed / server wants Basic — retry with Basic auth.
                _ => client
                    .get(uri)
                    .basic_auth(u, Some(p))
                    .send()
                    .await
                    .map_err(|e| format!("GET failed: {e}"))?,
            }
        }
        None => client
            .get(uri)
            .send()
            .await
            .map_err(|e| format!("GET failed: {e}"))?,
    };

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("read body: {e}"))?;
    if looks_like_image(&bytes) {
        Ok(bytes.len())
    } else {
        Err(format!("not an image ({} bytes)", bytes.len()))
    }
}

/// True when `bytes` starts with a JPEG (`FF D8`), PNG (`89 50 4E 47`) or BMP
/// (`42 4D`) magic signature — enough to reject a 0-byte body or an HTML error
/// page that some firmware returns with a `200` instead of a real snapshot.
fn looks_like_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8]) || bytes.starts_with(b"\x89PNG") || bytes.starts_with(b"BM")
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
    let start = Instant::now();
    let svcs = s.get_services().await;
    let elapsed = start.elapsed();

    // Media2 (`ver20/media`) presence — a Profile T requirement. Many devices
    // omit Media2 from the GetCapabilities extension and only list it in
    // GetServices, so check both; GetServices is the reliable source.
    let has_media2 = s.capabilities().media2.url.is_some()
        || svcs
            .as_ref()
            .map(|list| list.iter().any(|x| x.is_media2()))
            .unwrap_or(false);
    let media2 = if has_media2 {
        CheckResult::pass("media2", Category::Services, "advertised")
    } else {
        CheckResult::skip("media2", Category::Services, "Media2 not advertised")
    };

    let get_services = match &svcs {
        Ok(list) => CheckResult::pass(
            "get_services",
            Category::Services,
            format!("{} service(s)", list.len()),
        )
        .with_elapsed(elapsed),
        Err(e) => {
            CheckResult::fail_from("get_services", Category::Services, e).with_elapsed(elapsed)
        }
    };

    vec![get_services, media2]
}

/// Profile G assessment for the `recording` / `search` / `replay` check ids
/// (fed to `mod.rs::assess`).
///
/// Without liveness probing this is presence-only: Pass when the service is
/// advertised (via GetCapabilities or the GetServices fallback resolved during
/// session build), Skip when absent — the services are not exercised.
///
/// With liveness probing on, each advertised service is actually exercised:
/// `search` runs a real recording search (find → poll → end), `replay`
/// resolves a replay URI for the first recording found, and `recording`
/// lists recordings. A SOAP fault here is a genuine Profile G failure, no
/// longer hidden behind "advertised".
pub(super) async fn recording_services(s: &OnvifSession, liveness: bool) -> Vec<CheckResult> {
    let caps = s.capabilities();
    let recording_url = caps.recording.url.clone();
    let search_url = caps.search.url.clone();
    let replay_url = caps.replay.url.clone();

    if !liveness {
        return [
            ("recording", recording_url.as_deref()),
            ("search", search_url.as_deref()),
            ("replay", replay_url.as_deref()),
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
        .collect();
    }

    let mut out = Vec::new();

    // recording — list stored recordings.
    if recording_url.is_some() {
        out.push(
            one("recording", Category::Services, async {
                let recs = s.get_recordings().await?;
                Ok(format!("{} recording(s)", recs.len()))
            })
            .await,
        );
    } else {
        out.push(CheckResult::skip(
            "recording",
            Category::Services,
            "not advertised",
        ));
    }

    // search — real find → poll → end; keep the first recording token for replay.
    let mut first_recording: Option<String> = None;
    if search_url.is_some() {
        let start = Instant::now();
        match s.search_recordings(None).await {
            Ok(recs) => {
                first_recording = recs.first().map(|r| r.recording_token.clone());
                out.push(
                    CheckResult::pass(
                        "search",
                        Category::Services,
                        format!("{} recording(s) found", recs.len()),
                    )
                    .with_elapsed(start.elapsed()),
                );
            }
            Err(e) => out.push(
                CheckResult::fail_from("search", Category::Services, &e)
                    .with_elapsed(start.elapsed()),
            ),
        }
    } else {
        out.push(CheckResult::skip(
            "search",
            Category::Services,
            "not advertised",
        ));
    }

    // replay — resolve a replay URI for the first recording found.
    match (replay_url.is_some(), first_recording) {
        (false, _) => out.push(CheckResult::skip(
            "replay",
            Category::Services,
            "not advertised",
        )),
        (true, None) => out.push(CheckResult::skip(
            "replay",
            Category::Services,
            "no recordings to replay",
        )),
        (true, Some(token)) => {
            let start = Instant::now();
            match s.get_replay_uri(&token, "RTP-Unicast", "RTSP").await {
                Ok(uri) => out.push(
                    CheckResult::pass("replay", Category::Services, uri)
                        .with_elapsed(start.elapsed()),
                ),
                Err(e) => out.push(
                    CheckResult::fail_from("replay", Category::Services, &e)
                        .with_elapsed(start.elapsed()),
                ),
            }
        }
    }

    out
}

pub(super) async fn media(
    s: &OnvifSession,
    liveness: bool,
    creds: Option<&(String, String)>,
) -> Vec<CheckResult> {
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
        // Stream URI — expect rtsp://. With liveness on, also probe the RTSP
        // server (a resolved URI is no guarantee the server actually answers).
        let start = Instant::now();
        match s.get_stream_uri(&token).await {
            Ok(u) if u.uri.starts_with("rtsp://") => {
                let elapsed = start.elapsed();
                let res = if liveness {
                    match rtsp_options_probe(&u.uri).await {
                        Ok(()) => CheckResult::pass(
                            "get_stream_uri",
                            Category::Media,
                            format!("{} (RTSP OPTIONS ok)", u.uri),
                        ),
                        Err(why) => CheckResult::warn(
                            "get_stream_uri",
                            Category::Media,
                            format!("RTSP not reachable: {why}"),
                            u.uri,
                        ),
                    }
                } else {
                    CheckResult::pass("get_stream_uri", Category::Media, u.uri)
                };
                out.push(res.with_elapsed(elapsed));
            }
            Ok(u) => out.push(
                CheckResult::warn("get_stream_uri", Category::Media, "non-rtsp scheme", u.uri)
                    .with_elapsed(start.elapsed()),
            ),
            Err(e) => out.push(
                CheckResult::fail_from("get_stream_uri", Category::Media, &e)
                    .with_elapsed(start.elapsed()),
            ),
        }
        // Snapshot URI — expect http(s)://. With liveness on, also fetch the
        // bytes and confirm they are a real image (not a 0-byte body or an
        // HTML error page some firmware returns with a 200).
        let start = Instant::now();
        match s.get_snapshot_uri(&token).await {
            Ok(u) if u.uri.starts_with("http") => {
                let elapsed = start.elapsed();
                let res = if liveness {
                    match fetch_snapshot(&u.uri, creds).await {
                        Ok(bytes) => CheckResult::pass(
                            "get_snapshot_uri",
                            Category::Media,
                            format!("{} ({} KB image)", u.uri, bytes / 1024),
                        ),
                        Err(why) => CheckResult::warn(
                            "get_snapshot_uri",
                            Category::Media,
                            format!("snapshot fetch: {why}"),
                            u.uri,
                        ),
                    }
                } else {
                    CheckResult::pass("get_snapshot_uri", Category::Media, u.uri)
                };
                out.push(res.with_elapsed(elapsed));
            }
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
    let mut out = Vec::new();

    // GetEventProperties — and, from the same response, whether the device
    // exposes a motion-alarm topic (a Profile T requirement). A device that
    // answers GetEventProperties but advertises no motion topic is likely
    // Profile S, not T; keep it a Skip so it flags "couldn't confirm T"
    // (Inconclusive) rather than painting a false failure on an S-only device.
    let start = Instant::now();
    match s.get_event_properties().await {
        Ok(props) => {
            out.push(
                CheckResult::pass(
                    "get_event_properties",
                    Category::Events,
                    format!("{} topic(s)", props.topics.len()),
                )
                .with_elapsed(start.elapsed()),
            );
            let motion = props
                .topics
                .iter()
                .find(|t| t.to_ascii_lowercase().contains("motion"));
            out.push(match motion {
                Some(t) => CheckResult::pass("event_motion_topic", Category::Events, t.clone()),
                None => CheckResult::skip(
                    "event_motion_topic",
                    Category::Events,
                    "no motion-alarm topic advertised",
                ),
            });
        }
        Err(e) => out.push(
            CheckResult::fail_from("get_event_properties", Category::Events, &e)
                .with_elapsed(start.elapsed()),
        ),
    }
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

/// Negative security probe: confirm the device actually *enforces*
/// authentication. Calls `GetDeviceInformation` on a credential-free client —
/// an operation the ONVIF access policy requires authentication for (unlike the
/// pre-auth `GetSystemDateAndTime` / `GetCapabilities`). If it returns data
/// without credentials, the device is leaking device info to anonymous clients
/// (a security finding, `Warn`), not a conformance pass. An auth rejection is
/// the healthy outcome (`Pass`); any other error leaves it undetermined
/// (`Skip`). Only runs when credentials were supplied — otherwise every call is
/// already anonymous and there is nothing to compare against.
pub(super) async fn auth_enforcement(device_url: &str, had_creds: bool) -> Vec<CheckResult> {
    if !had_creds {
        return vec![CheckResult::skip(
            "auth_enforcement",
            Category::Security,
            "no credentials supplied to test enforcement",
        )];
    }
    let start = Instant::now();
    // A credential-free client aimed straight at the device service — no
    // GetCapabilities round-trip, so it works even where that is auth-gated.
    let client = crate::OnvifClient::new(device_url);
    let res = match client.get_device_info().await {
        Ok(_) => CheckResult::warn(
            "auth_enforcement",
            Category::Security,
            "device returned GetDeviceInformation without authentication",
            "unauthenticated read allowed",
        ),
        Err(e) if CheckError::from(&e).is_auth() => CheckResult::pass(
            "auth_enforcement",
            Category::Security,
            "GetDeviceInformation rejected without credentials",
        ),
        Err(e) => CheckResult::skip(
            "auth_enforcement",
            Category::Security,
            format!("undetermined: {e}"),
        ),
    };
    vec![res.with_elapsed(start.elapsed())]
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

#[cfg(test)]
mod probe_tests {
    use super::*;

    #[test]
    fn image_magic_accepts_jpeg_png_rejects_html_and_empty() {
        assert!(looks_like_image(&[0xFF, 0xD8, 0xFF, 0xE0])); // JPEG
        assert!(looks_like_image(b"\x89PNG\r\n\x1a\n")); // PNG
        assert!(looks_like_image(b"BM\x00\x00")); // BMP
        assert!(!looks_like_image(b"<html><body>401</body></html>")); // error page
        assert!(!looks_like_image(b"")); // 0-byte body
    }

    #[tokio::test]
    async fn rtsp_probe_rejects_non_rtsp_url() {
        let err = rtsp_options_probe("http://192.168.1.10/stream")
            .await
            .unwrap_err();
        assert!(err.contains("not an rtsp"), "unexpected error: {err}");
    }
}
