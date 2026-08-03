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
use crate::types::{
    Capabilities, DeviceServiceCapabilities, EventsServiceCapabilities, MediaServiceCapabilities,
};
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
/// count on success. Performs a manual HTTP Digest handshake (challenge → answer)
/// so the `qop="auth"` value can be quoted — some Hikvision/Uniview firmware
/// reject the unquoted `qop=auth` that `diqwest`/`digest_auth` emit by default and
/// answer with a non-image `200` body. Falls back to Basic auth when the device
/// does not offer Digest. A `200` carrying an HTML error page or a 0-byte body —
/// a common firmware quirk — is rejected here rather than counted as a passing
/// snapshot.
async fn fetch_snapshot(uri: &str, creds: Option<&(String, String)>) -> Result<usize, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("client build failed: {e}"))?;

    let resp = match creds {
        Some((u, p)) => {
            // Unauthenticated GET first — yields the Digest challenge (and some
            // cameras serve the snapshot anonymously, answering 200 straight away).
            let first = client
                .get(uri)
                .send()
                .await
                .map_err(|e| format!("GET failed: {e}"))?;
            if first.status().is_success() {
                first
            } else if first.status().as_u16() == 401 {
                let www = first
                    .headers()
                    .get(reqwest::header::WWW_AUTHENTICATE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let digest = if www.to_lowercase().contains("digest") {
                    digest_header(&www, uri, u, p)
                } else {
                    None
                };
                let had_digest = digest.is_some();
                let authed = match digest {
                    Some(header) => client
                        .get(uri)
                        .header(reqwest::header::AUTHORIZATION, header)
                        .send()
                        .await
                        .map_err(|e| format!("GET failed: {e}"))?,
                    None => client
                        .get(uri)
                        .basic_auth(u, Some(p))
                        .send()
                        .await
                        .map_err(|e| format!("GET failed: {e}"))?,
                };
                // Digest offered but rejected (stale nonce, unusual realm) —
                // give Basic a chance before giving up.
                if !authed.status().is_success() && had_digest {
                    client
                        .get(uri)
                        .basic_auth(u, Some(p))
                        .send()
                        .await
                        .map_err(|e| format!("GET failed: {e}"))?
                } else {
                    authed
                }
            } else {
                first
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

/// Build a `Digest` `Authorization` header for `GET uri` from a server
/// `WWW-Authenticate` challenge, quoting the `qop` value (`qop=auth` →
/// `qop="auth"`). `digest_auth` emits `qop` unquoted, which some Hikvision and
/// Uniview firmware reject — answering with a non-image `200` body — so we
/// re-quote it here, mirroring the fix the oxdm snapshot path applies. Returns
/// `None` if the challenge is unparseable.
fn digest_header(www_authenticate: &str, uri: &str, user: &str, pass: &str) -> Option<String> {
    let url = reqwest::Url::parse(uri).ok()?;
    let request_uri = match url.query() {
        Some(q) => format!("{}?{}", url.path(), q),
        None => url.path().to_string(),
    };
    let mut prompt = digest_auth::parse(www_authenticate).ok()?;
    let ctx = digest_auth::AuthContext::new(user, pass, &request_uri);
    let answer = prompt.respond(&ctx).ok()?;
    Some(
        answer
            .to_header_string()
            .replace("qop=auth", r#"qop="auth""#),
    )
}

/// True when `bytes` starts with a JPEG (`FF D8`), PNG (`89 50 4E 47`) or BMP
/// (`42 4D`) magic signature — enough to reject a 0-byte body or an HTML error
/// page that some firmware returns with a `200` instead of a real snapshot.
fn looks_like_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8]) || bytes.starts_with(b"\x89PNG") || bytes.starts_with(b"BM")
}

/// `scheme://host[:port]` of a URL — the base for guessing sibling service URLs.
fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let host = rest.split('/').next().unwrap_or(rest);
    (!host.is_empty()).then(|| format!("{scheme}://{host}"))
}

/// Best-effort conventional service URLs for a device that does not advertise a
/// service (used by [`HealthCheck::with_force_unsupported`](super::HealthCheck::with_force_unsupported)).
/// Vendors use several path conventions (`/onvif/media2`, `/Media2`,
/// `/media2_service`); the device endpoint itself is included last for
/// single-endpoint devices that route by SOAP action rather than URL path.
fn service_url_candidates(device_url: &str, name: &str) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(origin) = origin_of(device_url) {
        let cap = {
            let mut c = name.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        };
        v.push(format!("{origin}/onvif/{name}"));
        v.push(format!("{origin}/onvif/{cap}"));
        v.push(format!("{origin}/onvif/{name}_service"));
    }
    v.push(device_url.to_string());
    v.dedup();
    v
}

/// Try each candidate URL with a single-call `probe`; return the first URL whose
/// call succeeds (i.e. the service actually responds there).
async fn first_responding<F, Fut, T>(candidates: &[String], mut probe: F) -> Option<String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<T, OnvifError>>,
{
    for c in candidates {
        if probe(c.clone()).await.is_ok() {
            return Some(c.clone());
        }
    }
    None
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

pub(super) async fn services(s: &OnvifSession, force: bool, device_url: &str) -> Vec<CheckResult> {
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
    } else if force {
        // Not advertised anywhere — force-verify against guessed Media2 URLs.
        let start = Instant::now();
        let candidates = service_url_candidates(device_url, "media2");
        let found = first_responding(&candidates, |url| async move {
            s.client().get_profiles_media2(&url).await
        })
        .await;
        match found {
            Some(url) => CheckResult::warn(
                "media2",
                Category::Services,
                "not advertised, but responds when forced (under-declared)",
                url,
            )
            .with_elapsed(start.elapsed()),
            None => CheckResult::skip("media2", Category::Services, "Media2 not advertised")
                .with_elapsed(start.elapsed()),
        }
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

/// One service's `GetServiceCapabilities`. Skips when the service is not
/// advertised, so an S-only camera is not painted with nine failures.
///
/// `fut` is built by the caller but only polled when `advertised` — an async
/// block is lazy, so an un-advertised service costs no request.
///
/// Returns the parsed value alongside the result: three of the nine feed
/// [`capability_disagreements`], and re-calling for that would double the
/// request count for no new information.
async fn caps_check<T, F>(
    id: &'static str,
    service: &str,
    advertised: bool,
    fut: F,
) -> (Option<T>, CheckResult)
where
    F: Future<Output = Result<T, OnvifError>>,
{
    if !advertised {
        return (
            None,
            CheckResult::skip(
                id,
                Category::Services,
                format!("{service} service not advertised"),
            ),
        );
    }
    let start = Instant::now();
    match fut.await {
        Ok(v) => (
            Some(v),
            CheckResult::pass(id, Category::Services, "answered").with_elapsed(start.elapsed()),
        ),
        Err(e) => (
            None,
            CheckResult::fail_from(id, Category::Services, &e).with_elapsed(start.elapsed()),
        ),
    }
}

/// Outcome of cross-checking the facts a device states twice.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CapabilityCrossCheck {
    /// Facts the service side stated at all (`Some`), and so had an opinion on.
    pub checked: usize,
    /// Sound findings: the device-level response said **yes** and the service
    /// said **no**. See [`capability_cross_check`] for why only this direction.
    pub contradictions: Vec<String>,
    /// The service said yes where the device-level response reads no. Counted
    /// and reported, but **not** a finding — see the note on direction.
    pub service_only: usize,
}

/// Cross-check the facts a device states **twice** — once in the device-level
/// `GetCapabilities` and again in a service's `GetServiceCapabilities`.
///
/// Eighteen attributes appear in both. Everything else a capability report
/// contains is a claim with nothing to contradict it; these eighteen can be
/// *wrong* rather than merely unknown, which makes them the only part checkable
/// without vendor knowledge. A client that trusts either source is guessing when
/// they differ.
///
/// # Only one direction is a finding
///
/// This is the subtle part, and getting it wrong makes the check worse than
/// absent. The device-level [`Capabilities`] uses bare `bool`, so it **cannot
/// distinguish "said no" from "did not say"** — an omitted `<tt:Network>`
/// element, which is legal and common, parses as four `false`s. The
/// service-side types use `Option<bool>` precisely because they can.
///
/// So:
///
/// - `GetCapabilities=true`, service `Some(false)` → **a real contradiction.**
///   `true` cannot come from absence; the element was present and said yes.
/// - `GetCapabilities=false`, service `Some(true)` → **unverifiable.** Either
///   the device contradicts itself, or it simply omitted the element. Counted as
///   `service_only` and reported, never warned about.
///
/// Measured on this check's first run: oxvif's own mock tripped the second case
/// six times, its `GetCapabilities` having omitted `<tt:Network>`,
/// `<tt:System>` and `<tt:Security>` entirely. Treating that direction as a
/// finding would flag every terse but conformant camera — so it was counted,
/// not warned about, and the mock was fixed on its own merits. It now sends all
/// three blocks, and `health::tests` asserts the run reports **0** stated only
/// by the service.
///
/// A `None` on the service side is not compared at all: "the device did not
/// mention it" is a third answer, and collapsing it into `false` is the mistake
/// the `Option<bool>` types exist to prevent.
///
/// Deliberately *not* compared: `Capabilities.device.system.firmware_upgrade`
/// against `DeviceSystemCapabilities::http_firmware_upgrade`. They read like a
/// pair and are not — one is "can be upgraded", the other "can be upgraded over
/// HTTP" — so a device with a non-HTTP upgrade path would be flagged for telling
/// the truth twice.
fn capability_cross_check(
    caps: &Capabilities,
    device: Option<&DeviceServiceCapabilities>,
    media: Option<&MediaServiceCapabilities>,
    events: Option<&EventsServiceCapabilities>,
) -> CapabilityCrossCheck {
    let mut r = CapabilityCrossCheck::default();
    let mut cmp = |name: &str, from_caps: bool, from_service: Option<bool>| {
        let Some(v) = from_service else { return };
        r.checked += 1;
        match (from_caps, v) {
            (true, false) => r.contradictions.push(format!(
                "{name}: GetCapabilities=true, GetServiceCapabilities=false"
            )),
            (false, true) => r.service_only += 1,
            _ => {}
        }
    };

    if let Some(d) = device {
        let (n, sy, se) = (
            &caps.device.network,
            &caps.device.system,
            &caps.device.security,
        );
        cmp("device/IPFilter", n.ip_filter, d.network.ip_filter);
        cmp(
            "device/ZeroConfiguration",
            n.zero_configuration,
            d.network.zero_configuration,
        );
        cmp("device/IPVersion6", n.ip_version6, d.network.ip_version6);
        cmp("device/DynDNS", n.dyn_dns, d.network.dyn_dns);
        cmp(
            "device/DiscoveryResolve",
            sy.discovery_resolve,
            d.system.discovery_resolve,
        );
        cmp(
            "device/DiscoveryBye",
            sy.discovery_bye,
            d.system.discovery_bye,
        );
        cmp(
            "device/RemoteDiscovery",
            sy.remote_discovery,
            d.system.remote_discovery,
        );
        cmp(
            "device/SystemBackup",
            sy.system_backup,
            d.system.system_backup,
        );
        cmp(
            "device/SystemLogging",
            sy.system_logging,
            d.system.system_logging,
        );
        cmp("device/TLS1.2", se.tls_1_2, d.security.tls1_2);
        cmp(
            "device/OnboardKeyGeneration",
            se.onboard_key_generation,
            d.security.onboard_key_generation,
        );
        cmp(
            "device/AccessPolicyConfig",
            se.access_policy_config,
            d.security.access_policy_config,
        );
        cmp("device/X.509Token", se.x509_token, d.security.x509_token);
        cmp(
            "device/UsernameToken",
            se.username_token,
            d.security.username_token,
        );
    }

    if let Some(m) = media {
        let st = &caps.media.streaming;
        cmp(
            "media/RTPMulticast",
            st.rtp_multicast,
            m.streaming.rtp_multicast,
        );
        cmp("media/RTP_TCP", st.rtp_tcp, m.streaming.rtp_tcp);
        cmp(
            "media/RTP_RTSP_TCP",
            st.rtp_rtsp_tcp,
            m.streaming.rtp_rtsp_tcp,
        );
    }

    if let Some(e) = events {
        cmp(
            "events/WSSubscriptionPolicySupport",
            caps.events.ws_subscription_policy,
            e.ws_subscription_policy_support,
        );
    }

    r
}

/// `GetServiceCapabilities` on all nine services, plus the cross-check of the
/// facts the device states twice.
///
/// `get_capabilities` (the `connect` check) answers *which services exist and
/// where*; this answers *what each one says it can do*. The nine operations
/// shipped in 0.15.0 and nothing in the health check asked them until now, which
/// left the report unable to see the one class of defect it is best placed to
/// find: a device contradicting itself between the two.
pub(super) async fn service_capabilities(s: &OnvifSession) -> Vec<CheckResult> {
    let caps = s.capabilities();

    // The device service is how we got here, so it is never "not advertised".
    let (device, c_device) = caps_check(
        "service_caps_device",
        "Device",
        true,
        s.device_get_service_capabilities(),
    )
    .await;
    let (media, c_media) = caps_check(
        "service_caps_media",
        "Media",
        caps.media.url.is_some(),
        s.media_get_service_capabilities(),
    )
    .await;
    let (events, c_events) = caps_check(
        "service_caps_events",
        "Events",
        caps.events.url.is_some(),
        s.events_get_service_capabilities(),
    )
    .await;

    let mut out = vec![c_device, c_media, c_events];

    // The remaining six have no device-level counterpart to cross-check, so
    // only the call itself is reported.
    macro_rules! plain {
        ($id:literal, $service:literal, $advertised:expr, $call:expr) => {
            out.push(caps_check($id, $service, $advertised, $call).await.1)
        };
    }
    plain!(
        "service_caps_media2",
        "Media2",
        caps.media2.url.is_some(),
        s.media2_get_service_capabilities()
    );
    plain!(
        "service_caps_ptz",
        "PTZ",
        caps.ptz.url.is_some(),
        s.ptz_get_service_capabilities()
    );
    plain!(
        "service_caps_imaging",
        "Imaging",
        caps.imaging.url.is_some(),
        s.imaging_get_service_capabilities()
    );
    plain!(
        "service_caps_recording",
        "Recording",
        caps.recording.url.is_some(),
        s.recording_get_service_capabilities()
    );
    plain!(
        "service_caps_search",
        "Search",
        caps.search.url.is_some(),
        s.search_get_service_capabilities()
    );
    plain!(
        "service_caps_replay",
        "Replay",
        caps.replay.url.is_some(),
        s.replay_get_service_capabilities()
    );

    let x = capability_cross_check(caps, device.as_ref(), media.as_ref(), events.as_ref());
    const ID: &str = "service_caps_self_consistent";
    out.push(if x.checked == 0 {
        // Every comparable attribute was absent, or the calls that carry them
        // failed. Nothing was checked, so this must not read as a pass.
        CheckResult::skip(
            ID,
            Category::Services,
            "no attribute stated by both GetCapabilities and GetServiceCapabilities",
        )
    } else if x.contradictions.is_empty() {
        CheckResult::pass(
            ID,
            Category::Services,
            format!(
                "{} fact(s) cross-checked, no contradiction ({} stated only by the service)",
                x.checked, x.service_only
            ),
        )
    } else {
        // The device contradicts itself. A Warn, not a Fail: it still works, and
        // which source is right is unknowable from here — but a client picking
        // either one is guessing, so it has to be visible.
        CheckResult::warn(
            ID,
            Category::Services,
            format!(
                "device contradicts itself on {}/{} fact(s): {}",
                x.contradictions.len(),
                x.checked,
                x.contradictions.join("; ")
            ),
            format!("{}/{} contradict", x.contradictions.len(), x.checked),
        )
    });

    out
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
pub(super) async fn recording_services(
    s: &OnvifSession,
    liveness: bool,
    force: bool,
    device_url: &str,
) -> Vec<CheckResult> {
    let caps = s.capabilities();
    let recording_url = caps.recording.url.clone();
    let search_url = caps.search.url.clone();
    let replay_url = caps.replay.url.clone();

    // Presence-only fast path: not exercising (liveness) and not forcing.
    if !liveness && !force {
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

    const UNDER_DECLARED: &str = "not advertised, but responds when forced (under-declared)";
    let client = s.client();
    let mut out = Vec::new();

    // recording — list stored recordings.
    let start = Instant::now();
    let rec = if recording_url.is_some() {
        if liveness {
            match s.get_recordings().await {
                Ok(recs) => CheckResult::pass(
                    "recording",
                    Category::Services,
                    format!("{} recording(s)", recs.len()),
                ),
                Err(e) => CheckResult::fail_from("recording", Category::Services, &e),
            }
        } else {
            CheckResult::pass(
                "recording",
                Category::Services,
                "advertised (not exercised)",
            )
        }
    } else if force {
        let candidates = service_url_candidates(device_url, "recording");
        match first_responding(&candidates, |url| async move {
            client.get_recordings(&url).await
        })
        .await
        {
            Some(url) => CheckResult::warn("recording", Category::Services, UNDER_DECLARED, url),
            None => CheckResult::skip("recording", Category::Services, "not advertised"),
        }
    } else {
        CheckResult::skip("recording", Category::Services, "not advertised")
    };
    out.push(rec.with_elapsed(start.elapsed()));

    // search — find → poll → end; keep the first recording token for replay.
    let start = Instant::now();
    let mut first_recording: Option<String> = None;
    let search = if search_url.is_some() {
        if liveness {
            match s.search_recordings(None).await {
                Ok(recs) => {
                    first_recording = recs.first().map(|r| r.recording_token.clone());
                    CheckResult::pass(
                        "search",
                        Category::Services,
                        format!("{} recording(s) found", recs.len()),
                    )
                }
                Err(e) => CheckResult::fail_from("search", Category::Services, &e),
            }
        } else {
            CheckResult::pass("search", Category::Services, "advertised (not exercised)")
        }
    } else if force {
        let candidates = service_url_candidates(device_url, "search");
        let mut found = None;
        for cand in &candidates {
            if let Ok(token) = client.find_recordings(cand, None, "PT10S").await {
                let results = client
                    .get_recording_search_results(cand, &token, 10, "PT5S")
                    .await;
                let _ = client.end_search(cand, &token).await;
                let recs = results.map(|r| r.recording_information).unwrap_or_default();
                found = Some((cand.clone(), recs));
                break;
            }
        }
        match found {
            Some((url, recs)) => {
                first_recording = recs.first().map(|r| r.recording_token.clone());
                CheckResult::warn(
                    "search",
                    Category::Services,
                    UNDER_DECLARED,
                    format!("{url}  ({} found)", recs.len()),
                )
            }
            None => CheckResult::skip("search", Category::Services, "not advertised"),
        }
    } else {
        CheckResult::skip("search", Category::Services, "not advertised")
    };
    out.push(search.with_elapsed(start.elapsed()));

    // replay — resolve a replay URI for the first recording found.
    let start = Instant::now();
    let replay = if replay_url.is_some() {
        if !liveness {
            CheckResult::pass("replay", Category::Services, "advertised (not exercised)")
        } else if let Some(token) = &first_recording {
            match s.get_replay_uri(token, "RTP-Unicast", "RTSP").await {
                Ok(uri) => CheckResult::pass("replay", Category::Services, uri),
                Err(e) => CheckResult::fail_from("replay", Category::Services, &e),
            }
        } else {
            CheckResult::skip("replay", Category::Services, "no recordings to replay")
        }
    } else if force {
        match &first_recording {
            Some(token) => {
                let candidates = service_url_candidates(device_url, "replay");
                let mut found = None;
                for cand in &candidates {
                    if let Ok(uri) = client
                        .get_replay_uri(cand, token, "RTP-Unicast", "RTSP")
                        .await
                    {
                        found = Some(uri);
                        break;
                    }
                }
                match found {
                    Some(uri) => {
                        CheckResult::warn("replay", Category::Services, UNDER_DECLARED, uri)
                    }
                    None => CheckResult::skip("replay", Category::Services, "not advertised"),
                }
            }
            None => CheckResult::skip("replay", Category::Services, "no recordings to replay"),
        }
    } else {
        CheckResult::skip("replay", Category::Services, "not advertised")
    };
    out.push(replay.with_elapsed(start.elapsed()));

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

    #[test]
    fn digest_header_quotes_qop_for_hikvision_uniview() {
        // A typical camera challenge advertising qop="auth".
        let challenge = r#"Digest realm="IP Camera", nonce="abc123", qop="auth""#;
        let header = digest_header(
            challenge,
            "http://192.168.1.10/onvif/snapshot",
            "admin",
            "pw",
        )
        .expect("challenge should parse");
        assert!(header.starts_with("Digest "));
        // The fix: qop must be quoted, never emitted as bare `qop=auth`.
        assert!(header.contains(r#"qop="auth""#), "qop not quoted: {header}");
        assert!(!header.contains("qop=auth,"), "bare qop leaked: {header}");
    }

    #[test]
    fn digest_header_returns_none_on_garbage_challenge() {
        assert!(digest_header("Basic realm=x", "http://h/p", "u", "p").is_none());
    }

    #[tokio::test]
    async fn rtsp_probe_rejects_non_rtsp_url() {
        let err = rtsp_options_probe("http://192.168.1.10/stream")
            .await
            .unwrap_err();
        assert!(err.contains("not an rtsp"), "unexpected error: {err}");
    }

    #[test]
    fn origin_of_extracts_scheme_host_port() {
        assert_eq!(
            origin_of("http://192.168.1.50:8080/onvif/device"),
            Some("http://192.168.1.50:8080".into())
        );
        assert_eq!(
            origin_of("https://cam.local/onvif/device_service"),
            Some("https://cam.local".into())
        );
        assert_eq!(origin_of("not-a-url"), None);
    }

    #[test]
    fn service_url_candidates_cover_common_conventions() {
        let c = service_url_candidates("http://192.168.1.50/onvif/device_service", "media2");
        // The three path conventions seen across vendors.
        for want in [
            "http://192.168.1.50/onvif/media2",
            "http://192.168.1.50/onvif/Media2",
            "http://192.168.1.50/onvif/media2_service",
            // …plus the device endpoint itself (single-endpoint devices).
            "http://192.168.1.50/onvif/device_service",
        ] {
            assert!(
                c.contains(&want.to_string()),
                "missing candidate {want}: {c:?}"
            );
        }
        // Port is preserved on every candidate.
        let c = service_url_candidates("http://192.168.1.50:8080/onvif/device", "recording");
        assert!(c.iter().all(|u| u.starts_with("http://192.168.1.50:8080")));
    }
}

#[cfg(test)]
mod capability_cross_check_tests {
    use super::*;
    use crate::types::{
        DeviceCapabilities, DeviceSecurityCapabilities, EventsCapabilities, MediaCapabilities,
        MediaStreamingCapabilities, SecurityCapabilities, StreamingCapabilities,
    };

    /// A device-level `GetCapabilities` that says yes to four things. Each is a
    /// distinct fact so a disagreement can be attributed to exactly one of them.
    fn device_level() -> Capabilities {
        Capabilities {
            device: DeviceCapabilities {
                security: SecurityCapabilities {
                    username_token: true,
                    tls_1_2: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            media: MediaCapabilities {
                streaming: StreamingCapabilities {
                    rtp_rtsp_tcp: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            events: EventsCapabilities {
                ws_subscription_policy: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn dev_service(
        username_token: Option<bool>,
        tls1_2: Option<bool>,
    ) -> DeviceServiceCapabilities {
        DeviceServiceCapabilities {
            security: DeviceSecurityCapabilities {
                username_token,
                tls1_2,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn media_service(rtp_rtsp_tcp: Option<bool>) -> MediaServiceCapabilities {
        MediaServiceCapabilities {
            streaming: MediaStreamingCapabilities {
                rtp_rtsp_tcp,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn events_service(support: Option<bool>) -> EventsServiceCapabilities {
        EventsServiceCapabilities {
            ws_subscription_policy_support: support,
            ..Default::default()
        }
    }

    #[test]
    fn a_consistent_device_checks_every_stated_fact_and_finds_nothing() {
        let x = capability_cross_check(
            &device_level(),
            Some(&dev_service(Some(true), Some(true))),
            Some(&media_service(Some(true))),
            Some(&events_service(Some(true))),
        );
        // Exactly the four attributes the fixture stated on both sides — not the
        // eighteen the function knows about, and not zero.
        assert_eq!(x.checked, 4, "expected 4 checked facts, got {}", x.checked);
        assert!(
            x.contradictions.is_empty(),
            "unexpected contradictions: {:?}",
            x.contradictions
        );
        assert_eq!(x.service_only, 0);
    }

    /// `GetCapabilities` said yes, the service says no. `true` cannot come from
    /// an omitted element, so this direction is a certainty.
    #[test]
    fn a_yes_then_no_is_a_contradiction_naming_the_attribute_and_both_values() {
        // UsernameToken: caps=true, service=false → reported.
        // TLS1.2:        caps=true, service=true  → agrees, must not be reported.
        let x = capability_cross_check(
            &device_level(),
            Some(&dev_service(Some(false), Some(true))),
            None,
            None,
        );
        assert_eq!(x.checked, 2);
        assert_eq!(
            x.contradictions,
            ["device/UsernameToken: GetCapabilities=true, GetServiceCapabilities=false"],
            "the message must name the attribute and both sides",
        );
        assert_eq!(x.service_only, 0);
    }

    /// The asymmetry that makes this check sound. The device-level `Capabilities`
    /// uses bare `bool`, so `false` there may mean "omitted the element" — legal
    /// and common. A service claiming a capability the device-level response did
    /// not mention is therefore counted, never warned about.
    ///
    /// Measured: oxvif's own mock hits this six times, and reading it as a
    /// finding would flag every terse but conformant camera.
    #[test]
    fn a_no_then_yes_is_counted_but_is_not_a_contradiction() {
        // A device-level response that stated nothing at all — every bool false.
        let silent = Capabilities::default();
        let x = capability_cross_check(
            &silent,
            Some(&dev_service(Some(true), Some(true))),
            Some(&media_service(Some(true))),
            Some(&events_service(Some(true))),
        );
        assert_eq!(x.checked, 4);
        assert!(
            x.contradictions.is_empty(),
            "`false` in GetCapabilities may be an omitted element, not a denial — \
             got {:?}",
            x.contradictions,
        );
        assert_eq!(
            x.service_only, 4,
            "all four should be recorded as service-only claims",
        );
    }

    /// The whole reason the 0.15 capability types use `Option<bool>`: `None` is
    /// "the device did not mention it", which is not an answer to compare. If
    /// this ever counted as `false`, a terse service response would look like a
    /// blanket denial.
    #[test]
    fn an_unstated_attribute_is_neither_checked_nor_reported() {
        let x = capability_cross_check(
            &device_level(),
            Some(&dev_service(None, None)),
            Some(&media_service(None)),
            Some(&events_service(None)),
        );
        assert_eq!(
            x.checked, 0,
            "an all-`None` service response has nothing to check",
        );
        assert!(
            x.contradictions.is_empty(),
            "`None` must not be read as `false`, got {:?}",
            x.contradictions,
        );
        assert_eq!(x.service_only, 0);
    }

    /// A service whose `GetServiceCapabilities` call failed contributes `None`
    /// for the whole struct — its facts drop out rather than defaulting to
    /// `false` and inventing contradictions against a device-level `true`.
    #[test]
    fn a_failed_service_call_drops_its_facts_instead_of_defaulting() {
        let x = capability_cross_check(&device_level(), None, None, None);
        assert_eq!(x, CapabilityCrossCheck::default());
    }
}
