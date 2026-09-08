//! Read-only camera workflows and explicit, no-clobber local artifacts.

use std::{
    fs,
    future::Future,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use oxvif::{OnvifError, OnvifSession, soap::SoapError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::{Instant, timeout};

use crate::application::{ResolvedTarget, build_http_transport};
use crate::{AppError, ClockSyncPolicy, ExecutionOptions, TargetSelector};

const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_INVENTORY_BYTES: u64 = 4 * 1024 * 1024;
const SECTIONS: &[&str] = &[
    "hostname",
    "ntp",
    "dns",
    "interfaces",
    "protocols",
    "gateway",
    "profiles",
    "video_encoders",
];

/// Save a single profile's snapshot without overwriting an existing file.
#[derive(Debug, Eq, PartialEq)]
pub struct SnapshotSaveRequest {
    pub selector: TargetSelector,
    pub profile: String,
    pub save: PathBuf,
}

impl SnapshotSaveRequest {
    /// Reject fleet selectors before profile lookup or file creation.
    pub fn validate_selector(selector: &TargetSelector) -> Result<(), AppError> {
        single_target(selector)
    }

    /// Validate local output and selector before prompting or contacting a camera.
    pub fn preflight(&self) -> Result<(), AppError> {
        single_target(&self.selector)?;
        validate_destination(&self.save)
    }
}

/// Run bounded diagnostic stages; omitted profile is selected only if unique.
#[derive(Debug, Eq, PartialEq)]
pub struct DiagnoseRequest {
    pub selector: TargetSelector,
    pub profile: Option<String>,
}

/// Save a versioned, read-only camera inventory, not a restorable backup.
#[derive(Debug, Eq, PartialEq)]
pub struct ConfigExportRequest {
    pub selector: TargetSelector,
    pub save: PathBuf,
}

/// Compare live settings with an inventory from the same camera identity.
#[derive(Debug, Eq, PartialEq)]
pub struct ConfigDiffRequest {
    pub selector: TargetSelector,
    pub against: PathBuf,
}

#[derive(Clone)]
pub(crate) enum Workflow {
    Snapshot { profile: String, save: PathBuf },
    Diagnose { profile: Option<String> },
    Export { save: PathBuf },
    Diff { baseline: Inventory },
}

impl Workflow {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Snapshot { .. } => "media.snapshot-save",
            Self::Diagnose { .. } => "diagnose",
            Self::Export { .. } => "config.export",
            Self::Diff { .. } => "config.diff",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Inventory {
    inventory_version: u32,
    identity: Identity,
    sections: std::collections::BTreeMap<String, Step>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    manufacturer: String,
    model: String,
    serial_number: String,
    hardware_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Pass,
    Fail,
    Unsupported,
    NotTested,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    name: String,
    status: Status,
    elapsed_ms: u64,
    error_code: Option<String>,
    detail: String,
    next_step: String,
    data: Option<Value>,
}

impl Step {
    fn new(name: &str, status: Status, detail: &str, next_step: &str) -> Self {
        Self {
            name: name.into(),
            status,
            elapsed_ms: 0,
            error_code: None,
            detail: detail.into(),
            next_step: next_step.into(),
            data: None,
        }
    }

    fn skipped(name: &str, why: &str) -> Self {
        Self::new(
            name,
            Status::NotTested,
            why,
            "Resolve prerequisites and run the diagnostic again.",
        )
    }
}

pub(crate) fn single_target(selector: &TargetSelector) -> Result<(), AppError> {
    if selector.group.is_some() || selector.view.is_some() {
        return Err(AppError::invalid_argument(
            "File workflows require one device; --group/--view are not supported.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_destination(path: &Path) -> Result<(), AppError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(AppError::resource_exists(
                "output file",
                &path.display().to_string(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(AppError::registry_io("Cannot inspect output destination.")),
    }
    if path.file_name().is_none() || !parent(path).is_dir() {
        return Err(AppError::invalid_argument(
            "The output parent directory must already exist.",
        ));
    }
    Ok(())
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn save_new(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    validate_destination(path)?;
    let mut file = tempfile::NamedTempFile::new_in(parent(path))
        .map_err(|_| AppError::registry_io("Cannot create temporary output file."))?;
    file.write_all(bytes)
        .and_then(|()| file.as_file().sync_all())
        .map_err(|_| {
            AppError::registry_io("Cannot finish output file; destination was not published.")
        })?;
    file.persist_noclobber(path).map_err(|_| {
        AppError::registry_io("Cannot publish output file; existing files are never overwritten.")
    })?;
    Ok(())
}

pub(crate) fn read_baseline(path: &Path) -> Result<Inventory, AppError> {
    if !fs::metadata(path)
        .map_err(|_| AppError::invalid_argument("Cannot inspect baseline."))?
        .is_file()
    {
        return Err(AppError::invalid_argument(
            "Baseline must be a regular file.",
        ));
    }
    let file = fs::File::open(path)
        .map_err(|_| AppError::invalid_argument("Cannot read inventory baseline."))?;
    if !file
        .metadata()
        .map_err(|_| AppError::invalid_argument("Cannot inspect baseline."))?
        .is_file()
    {
        return Err(AppError::invalid_argument(
            "Baseline must be a regular file.",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_INVENTORY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::invalid_argument("Cannot read inventory baseline."))?;
    if bytes.len() as u64 > MAX_INVENTORY_BYTES {
        return Err(AppError::invalid_argument(
            "Inventory baseline exceeds 4 MiB.",
        ));
    }
    let inventory: Inventory = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::invalid_argument("Invalid inventory baseline format."))?;
    if inventory.inventory_version != 1
        || inventory.sections.len() != SECTIONS.len()
        || SECTIONS
            .iter()
            .any(|name| !inventory.sections.contains_key(*name))
    {
        return Err(AppError::invalid_argument(
            "Unsupported inventory version or section catalogue.",
        ));
    }
    for (name, step) in &inventory.sections {
        if &step.name != name || matches!(step.status, Status::Pass) != step.data.is_some() {
            return Err(AppError::invalid_argument(
                "Inconsistent inventory section status/data.",
            ));
        }
        if let Some(data) = &step.data {
            let array = matches!(
                name.as_str(),
                "interfaces" | "protocols" | "profiles" | "video_encoders"
            );
            if (array && !data.is_array()) || (!array && !data.is_object()) {
                return Err(AppError::invalid_argument(
                    "Invalid inventory section data shape.",
                ));
            }
        }
    }
    Ok(inventory)
}

pub(crate) fn report_failed(operation: &str, value: &Value) -> bool {
    matches!(operation, "diagnose" | "config.export" | "config.diff")
        && (value["failed"].as_u64().unwrap_or(0) > 0 || value["complete"] == false)
}

pub(crate) async fn execute(
    resolved: &ResolvedTarget,
    operation: &Workflow,
    options: &ExecutionOptions,
) -> Result<Value, AppError> {
    let (connect, session) = connect(resolved, options).await?;
    if let Workflow::Diagnose { profile } = operation {
        return diagnose(
            resolved,
            options,
            connect,
            session.as_ref(),
            profile.as_deref(),
        )
        .await;
    }
    let session =
        session.ok_or_else(|| AppError::device_operation_failed(connect.detail, false))?;
    match operation {
        Workflow::Snapshot { profile, save } => {
            if profile.trim().is_empty() {
                return Err(AppError::invalid_argument(
                    "An explicit profile token is required.",
                ));
            }
            let (step, uri) = onvif_step("snapshot_uri", options, || {
                session.get_snapshot_uri(profile)
            })
            .await;
            let uri = uri.ok_or_else(|| AppError::device_operation_failed(step.detail, false))?;
            let (bytes, image_type) = fetch_image(&uri.uri, resolved, options).await?;
            save_new(save, &bytes)?;
            Ok(
                json!({"saved_to": save.display().to_string(), "bytes": bytes.len(), "image_type": image_type,
                "profile": profile, "validation": "signature_only_not_full_decode"}),
            )
        }
        Workflow::Export { save } => {
            let inventory = collect_inventory(&session, options).await?;
            let bytes = serde_json::to_vec_pretty(&inventory)
                .map_err(|_| AppError::serialization_failed("Cannot serialize inventory."))?;
            if bytes.len() as u64 > MAX_INVENTORY_BYTES {
                return Err(AppError::invalid_argument("Inventory exceeds 4 MiB."));
            }
            save_new(save, &bytes)?;
            Ok(
                json!({"saved_to": save.display().to_string(), "complete": complete(&inventory), "inventory": inventory}),
            )
        }
        Workflow::Diff { baseline } => {
            let live = collect_inventory(&session, options).await?;
            compare(baseline, &live)
        }
        Workflow::Diagnose { .. } => unreachable!("diagnose is handled above"),
    }
}

async fn connect(
    resolved: &ResolvedTarget,
    options: &ExecutionOptions,
) -> Result<(Step, Option<OnvifSession>), AppError> {
    let transport =
        build_http_transport(options, resolved.username.as_deref(), resolved.password())?;
    Ok(onvif_step("onvif_session", options, || {
        let mut builder = OnvifSession::builder(&resolved.target).with_transport(transport.clone());
        if let (Some(user), Some(password)) = (resolved.username.as_deref(), resolved.password()) {
            builder = builder.with_credentials(user, password);
        }
        if options.clock_sync == ClockSyncPolicy::Always
            || (options.clock_sync == ClockSyncPolicy::Auto && resolved.password.is_some())
        {
            builder = builder.with_clock_sync();
        }
        builder.build()
    })
    .await)
}

async fn onvif_step<T, F, Fut>(
    name: &str,
    options: &ExecutionOptions,
    mut operation: F,
) -> (Step, Option<T>)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, OnvifError>>,
{
    let start = Instant::now();
    let mut step = Step::new(
        name,
        Status::Fail,
        "Operation did not complete.",
        "Check camera connectivity and credentials.",
    );
    for attempt in 0..=options.retries {
        let retryable = match timeout(options.timeout, operation()).await {
            Ok(Ok(value)) => {
                step.status = Status::Pass;
                step.detail = "Operation completed.".into();
                step.error_code = None;
                step.next_step.clear();
                step.elapsed_ms = elapsed(start);
                return (step, Some(value));
            }
            Ok(Err(error)) => {
                let (code, detail, unsupported, retryable) = classify(&error);
                step.status = if unsupported {
                    Status::Unsupported
                } else {
                    Status::Fail
                };
                step.error_code = Some(code.into());
                step.detail = detail.into();
                step.next_step = match code {
                    "NOT_AUTHORIZED" => "Check credentials, account permissions and camera time.",
                    "UNSUPPORTED" => "Check advertised services or try another supported profile.",
                    "SOAP_ERROR" => "Check camera firmware and ONVIF compatibility.",
                    _ => "Check routing, port, TLS trust and camera availability.",
                }
                .into();
                retryable
            }
            Err(_) => {
                step.error_code = Some("TIMEOUT".into());
                step.detail = "Stage exceeded its per-attempt timeout.".into();
                true
            }
        };
        if !retryable || attempt == options.retries {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    step.elapsed_ms = elapsed(start);
    (step, None)
}

fn classify(error: &OnvifError) -> (&'static str, &'static str, bool, bool) {
    match error {
        OnvifError::Soap(SoapError::Fault {
            subcode: Some(code),
            ..
        }) => match code.rsplit(':').next() {
            Some("ActionNotSupported" | "OptionalActionNotImplemented") => (
                "UNSUPPORTED",
                "Device explicitly reports an unsupported action.",
                true,
                false,
            ),
            Some("NotAuthorized") => (
                "NOT_AUTHORIZED",
                "Device rejected authorization.",
                false,
                false,
            ),
            _ => (
                "SOAP_ERROR",
                "Device returned a SOAP fault (raw details withheld).",
                false,
                false,
            ),
        },
        OnvifError::Transport(oxvif::transport::TransportError::HttpStatus {
            status: 401 | 403,
            ..
        }) => (
            "NOT_AUTHORIZED",
            "Device rejected HTTP authorization.",
            false,
            false,
        ),
        OnvifError::Transport(_) => (
            "TRANSPORT_ERROR",
            "Connection, TLS or HTTP transport failed (URL/body withheld).",
            false,
            crate::application::is_retryable_onvif_error(error),
        ),
        OnvifError::InvalidArgument(_) => (
            "INVALID_OPERATION",
            "Operation or service is unavailable for this request.",
            false,
            false,
        ),
        _ => (
            "SOAP_ERROR",
            "Device response could not be used (raw details withheld).",
            false,
            false,
        ),
    }
}

fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

async fn diagnose(
    resolved: &ResolvedTarget,
    options: &ExecutionOptions,
    connect: Step,
    session: Option<&OnvifSession>,
    profile: Option<&str>,
) -> Result<Value, AppError> {
    let mut stages = vec![connect];
    if let Some(session) = session {
        let (info, _) = onvif_step("device_info", options, || session.get_device_info()).await;
        stages.push(info);
        let (mut profiles_step, profiles) =
            onvif_step("media_profiles", options, || session.get_profiles()).await;
        if let Some(profiles) = &profiles {
            profiles_step.data =
                Some(json!({"profiles": profiles.iter().map(|p| &p.token).collect::<Vec<_>>()}));
        }
        stages.push(profiles_step);
        let selected = profiles.as_ref().and_then(|profiles| match profile {
            Some(token) => profiles
                .iter()
                .find(|p| p.token == token)
                .map(|p| p.token.as_str()),
            None if profiles.len() == 1 => Some(profiles[0].token.as_str()),
            _ => None,
        });
        if let Some(token) = selected {
            let (stream, _) =
                onvif_step("stream_uri", options, || session.get_stream_uri(token)).await;
            stages.push(stream);
            let (snapshot, uri) =
                onvif_step("snapshot_uri", options, || session.get_snapshot_uri(token)).await;
            stages.push(snapshot);
            if let Some(uri) = uri {
                let start = Instant::now();
                let mut step = match fetch_image(&uri.uri, resolved, options).await {
                    Ok((bytes, image_type)) => {
                        let mut step = Step::new(
                            "snapshot_fetch",
                            Status::Pass,
                            "Downloaded image with a recognized signature; full decoding was not tested.",
                            "",
                        );
                        step.data = Some(json!({"bytes": bytes.len(), "image_type": image_type}));
                        step
                    }
                    Err(error) => {
                        let mut step = Step::new(
                            "snapshot_fetch",
                            Status::Fail,
                            &error.message,
                            "Check snapshot service access, authentication and image response.",
                        );
                        step.error_code = Some(error.code.as_str().into());
                        step
                    }
                };
                step.elapsed_ms = elapsed(start);
                stages.push(step);
            } else {
                stages.push(Step::skipped(
                    "snapshot_fetch",
                    "Snapshot URI was unavailable.",
                ));
            }
        } else {
            let mut selection = Step::new(
                "profile_selection",
                Status::Fail,
                "No unique matching profile was selected.",
                "Run profiles and pass an explicit --profile token.",
            );
            selection.error_code = Some("PROFILE_SELECTION_REQUIRED".into());
            stages.push(selection);
            for name in ["stream_uri", "snapshot_uri", "snapshot_fetch"] {
                stages.push(Step::skipped(name, "A matching profile is required."));
            }
        }
    } else {
        for name in [
            "device_info",
            "media_profiles",
            "stream_uri",
            "snapshot_uri",
            "snapshot_fetch",
        ] {
            stages.push(Step::skipped(name, "ONVIF session establishment failed."));
        }
    }
    let complete = stages
        .iter()
        .all(|stage| matches!(stage.status, Status::Pass | Status::Unsupported));
    for name in ["rtsp_transport", "video_decode"] {
        stages.push(Step::new(
            name,
            Status::NotTested,
            "Not implemented by this diagnostic; URI retrieval does not prove playback.",
            "Verify playback separately with a trusted RTSP client.",
        ));
    }
    let failed = stages
        .iter()
        .filter(|s| matches!(s.status, Status::Fail))
        .count();
    Ok(
        json!({"failed": failed, "complete": complete, "stages": stages, "playback_verified": false}),
    )
}

fn snapshot_url(uri: &str, device: &str) -> Result<url::Url, AppError> {
    let url = url::Url::parse(uri)
        .map_err(|_| AppError::invalid_argument("Invalid snapshot URL (withheld)."))?;
    let device = url::Url::parse(device)
        .map_err(|_| AppError::invalid_argument("Invalid device target."))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.host_str() != device.host_str()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || (device.scheme() == "https" && url.scheme() != "https")
    {
        return Err(AppError::invalid_argument(
            "Snapshot URL rejected: require HTTP(S), the device host, no userinfo/fragment and no HTTPS downgrade.",
        ));
    }
    Ok(url)
}

async fn fetch_image(
    uri: &str,
    resolved: &ResolvedTarget,
    options: &ExecutionOptions,
) -> Result<(Vec<u8>, &'static str), AppError> {
    let url = snapshot_url(uri, &resolved.target)?;
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(options.timeout);
    for pem in &options.ca_certificates {
        let certificates = reqwest::Certificate::from_pem_bundle(pem)
            .map_err(|_| AppError::invalid_argument("Invalid private CA bundle."))?;
        if certificates.is_empty() {
            return Err(AppError::invalid_argument("Empty private CA bundle."));
        }
        for certificate in certificates {
            builder = builder.add_root_certificate(certificate);
        }
    }
    let client = builder.build().map_err(|_| {
        AppError::device_operation_failed("Cannot initialize snapshot HTTP client.", false)
    })?;
    let future = async {
        let mut response = client.get(url.clone()).send().await.map_err(http_error)?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            && let (Some(user), Some(password)) =
                (resolved.username.as_deref(), resolved.password())
        {
            let challenges = response
                .headers()
                .get_all(reqwest::header::WWW_AUTHENTICATE)
                .iter()
                .filter_map(|h| h.to_str().ok())
                .collect::<Vec<_>>();
            let mut request = client.get(url.clone());
            if let Some(challenge) = challenges
                .iter()
                .find(|h| h.to_ascii_lowercase().starts_with("digest "))
            {
                let request_uri = match url.query() {
                    Some(q) => format!("{}?{q}", url.path()),
                    None => url.path().to_owned(),
                };
                let mut prompt = digest_auth::parse(challenge).map_err(|_| {
                    AppError::device_operation_failed(
                        "Unsupported snapshot Digest challenge.",
                        false,
                    )
                })?;
                let context = digest_auth::AuthContext::new(user, password, &request_uri);
                let answer = prompt.respond(&context).map_err(|_| {
                    AppError::device_operation_failed(
                        "Cannot answer snapshot Digest challenge.",
                        false,
                    )
                })?;
                let header = answer
                    .to_header_string()
                    .replace("qop=auth,", "qop=\"auth\",");
                request = request.header(reqwest::header::AUTHORIZATION, header);
            } else if challenges
                .iter()
                .any(|h| h.to_ascii_lowercase().starts_with("basic "))
            {
                request = request.basic_auth(user, Some(password));
            } else {
                return Err(AppError::device_operation_failed(
                    "Snapshot authentication challenge is unsupported.",
                    false,
                ));
            }
            response = request.send().await.map_err(http_error)?;
        }
        if response.status() != reqwest::StatusCode::OK {
            return Err(AppError::device_operation_failed(
                format!(
                    "Snapshot returned HTTP {}; redirects are not followed.",
                    response.status().as_u16()
                ),
                false,
            ));
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_IMAGE_BYTES as u64)
        {
            return Err(AppError::device_operation_failed(
                "Snapshot exceeds 16 MiB.",
                false,
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(http_error)? {
            if chunk.len() > MAX_IMAGE_BYTES - bytes.len() {
                return Err(AppError::device_operation_failed(
                    "Snapshot exceeds 16 MiB.",
                    false,
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let image_type = image_type(&bytes).ok_or_else(|| {
            AppError::device_operation_failed(
                "Snapshot is empty, truncated or has no supported JPEG/PNG/BMP signature.",
                false,
            )
        })?;
        Ok((bytes, image_type))
    };
    timeout(options.timeout, future).await.map_err(|_| {
        AppError::device_operation_failed("Snapshot exceeded its total download timeout.", true)
    })?
}

fn http_error(error: reqwest::Error) -> AppError {
    AppError::device_operation_failed(
        if error.is_timeout() {
            "Snapshot HTTP request timed out (URL withheld)."
        } else {
            "Snapshot HTTP request failed (URL/body withheld)."
        },
        error.is_timeout() || error.is_connect(),
    )
}

fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 4 && bytes.starts_with(b"\xff\xd8\xff") && bytes.ends_with(b"\xff\xd9") {
        Some("jpeg")
    } else if bytes.len() >= 33
        && bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        && bytes.get(12..16) == Some(b"IHDR")
    {
        Some("png")
    } else if bytes.len() >= 54 && bytes.starts_with(b"BM") {
        Some("bmp")
    } else {
        None
    }
}

async fn collect_inventory(
    session: &OnvifSession,
    options: &ExecutionOptions,
) -> Result<Inventory, AppError> {
    let (step, info) = onvif_step("identity", options, || session.get_device_info()).await;
    let info = info.ok_or_else(|| AppError::device_operation_failed(step.detail, false))?;
    let mut inventory = Inventory {
        inventory_version: 1,
        identity: Identity {
            manufacturer: info.manufacturer,
            model: info.model,
            serial_number: info.serial_number,
            hardware_id: info.hardware_id,
        },
        sections: Default::default(),
    };
    macro_rules! collect {
        ($name:literal, $method:ident) => {{
            let (mut step, data) = onvif_step($name, options, || session.$method()).await;
            if let Some(data) = data {
                let mut value = serde_json::to_value(data).map_err(|_| {
                    AppError::serialization_failed("Cannot serialize inventory section.")
                })?;
                normalize(&mut value);
                step.data = Some(value);
            }
            inventory.sections.insert($name.into(), step);
        }};
    }
    collect!("hostname", get_hostname);
    collect!("ntp", get_ntp);
    collect!("dns", get_dns);
    collect!("interfaces", get_network_interfaces);
    collect!("protocols", get_network_protocols);
    collect!("gateway", get_network_default_gateway);
    collect!("profiles", get_profiles);
    collect!("video_encoders", get_video_encoder_configurations);
    Ok(inventory)
}

fn normalize(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| {
                !matches!(
                    key.to_ascii_lowercase().as_str(),
                    "password" | "credentials" | "uri" | "url"
                )
            });
            for value in map.values_mut() {
                normalize(value);
            }
        }
        Value::Array(values) => {
            for value in values.iter_mut() {
                normalize(value);
            }
            // Order of DNS/NTP preference lists may be significant. Only sort
            // keyed record collections, never arbitrary nested arrays.
            if values
                .iter()
                .all(|value| value.get("token").and_then(Value::as_str).is_some())
            {
                values.sort_by_cached_key(|value| {
                    (
                        value["token"].as_str().unwrap_or_default().to_owned(),
                        value.to_string(),
                    )
                });
            } else if values
                .iter()
                .all(|value| value.get("name").and_then(Value::as_str).is_some())
            {
                values.sort_by_cached_key(|value| {
                    (
                        value["name"].as_str().unwrap_or_default().to_owned(),
                        value.to_string(),
                    )
                });
            }
        }
        Value::String(s) if s.contains("://") => {
            *s = "[URL withheld]".into();
        }
        _ => {}
    }
}

fn complete(inventory: &Inventory) -> bool {
    inventory
        .sections
        .values()
        .all(|step| matches!(step.status, Status::Pass))
}

fn compare(baseline: &Inventory, live: &Inventory) -> Result<Value, AppError> {
    if baseline.identity != live.identity || baseline.identity.serial_number.trim().is_empty() {
        return Err(AppError::invalid_argument(
            "Inventory identity differs or lacks a serial number; comparison refused.",
        ));
    }
    let mut changes = Vec::new();
    let mut incomparable = Vec::new();
    for name in SECTIONS {
        let left = &baseline.sections[*name];
        let right = &live.sections[*name];
        if !matches!(left.status, Status::Pass) || !matches!(right.status, Status::Pass) {
            incomparable.push(*name);
            continue;
        }
        if let (Some(mut before), Some(mut after)) = (left.data.clone(), right.data.clone()) {
            normalize(&mut before);
            normalize(&mut after);
            diff_value(
                &format!("/{name}"),
                Some(&before),
                Some(&after),
                &mut changes,
            );
        } else {
            incomparable.push(*name);
        }
    }
    Ok(
        json!({"complete": incomparable.is_empty(), "matches": if incomparable.is_empty() { Some(changes.is_empty()) } else { None },
        "changes": changes, "incomparable_sections": incomparable}),
    )
}

fn diff_value(path: &str, before: Option<&Value>, after: Option<&Value>, changes: &mut Vec<Value>) {
    if before == after {
        return;
    }
    if let (Some(Value::Object(left)), Some(Value::Object(right))) = (before, after) {
        let keys = left
            .keys()
            .chain(right.keys())
            .collect::<std::collections::BTreeSet<_>>();
        for key in keys {
            diff_value(
                &format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                left.get(key),
                right.get(key),
                changes,
            );
        }
    } else if let (Some(Value::Array(left)), Some(Value::Array(right))) = (before, after) {
        for index in 0..left.len().max(right.len()) {
            diff_value(
                &format!("{path}/{index}"),
                left.get(index),
                right.get(index),
                changes,
            );
        }
    } else {
        changes.push(json!({"path": path, "before_present": before.is_some(), "after_present": after.is_some(), "before": before, "after": after}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Application, CommandData, CommandRequest, MemoryCredentialStore, RegistryStore};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn resolved(url: &str) -> ResolvedTarget {
        ResolvedTarget {
            device_id: None,
            selected_by: None,
            target: url.into(),
            username: None,
            password: None,
        }
    }

    fn options() -> ExecutionOptions {
        ExecutionOptions {
            timeout: Duration::from_secs(2),
            non_interactive: true,
            clock_sync: ClockSyncPolicy::Never,
            ..Default::default()
        }
    }

    fn selector(url: &str) -> TargetSelector {
        TargetSelector {
            target: Some(url.into()),
            ..Default::default()
        }
    }

    fn result(success: &crate::CommandSuccess) -> &Value {
        let CommandData::DeviceDiagnostic { result, .. } = &success.data else {
            panic!("expected workflow report")
        };
        result
    }

    // Synthetic local HTTP responses, unrelated to external ONVIF schemas.
    async fn http_server(
        responses: Vec<Vec<u8>>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/image?ticket=never-log-this",
            listener.local_addr().unwrap()
        );
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = timeout(Duration::from_secs(3), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
                let mut request = Vec::new();
                loop {
                    let mut bytes = [0; 1024];
                    let size = socket.read(&mut bytes).await.unwrap();
                    if size == 0 {
                        break;
                    }
                    request.extend_from_slice(&bytes[..size]);
                    if request.windows(4).any(|s| s == b"\r\n\r\n") {
                        break;
                    }
                }
                requests.push(String::from_utf8(request).unwrap());
                let _ = socket.write_all(&response).await;
            }
            requests
        });
        (url, task)
    }

    fn response(status: &str, headers: &str, bytes: &[u8]) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n",
            bytes.len()
        )
        .into_bytes();
        response.extend_from_slice(bytes);
        response
    }

    #[test]
    fn snapshot_url_rejects_credential_exfiltration_and_downgrades() {
        for uri in [
            "http://foreign.test/image",
            "file:///image",
            "http://user:secret@camera.test/image",
            "https://camera.test/image#token",
            "http://camera.test/image",
        ] {
            let error = snapshot_url(uri, "https://camera.test/onvif/device_service").unwrap_err();
            assert!(!error.message.contains("secret"));
        }
        assert!(
            snapshot_url(
                "https://camera.test:8443/image?q=sensitive",
                "https://camera.test/onvif"
            )
            .is_ok()
        );
    }

    #[test]
    fn image_signatures_do_not_accept_html_or_short_bodies() {
        for bytes in [
            b"".as_slice(),
            b"<html>login</html>",
            b"\xff\xd8",
            b"\x89PNG",
            b"BM",
        ] {
            assert!(image_type(bytes).is_none());
        }
        assert_eq!(image_type(b"\xff\xd8\xff\xd9"), Some("jpeg"));
    }

    #[test]
    fn artifacts_never_clobber_or_leave_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("output.bin");
        save_new(&file, b"original").unwrap();
        assert!(save_new(&file, b"replacement").is_err());
        assert_eq!(fs::read(&file).unwrap(), b"original");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert!(save_new(&directory.path().join("missing/output"), b"data").is_err());
    }

    #[tokio::test]
    async fn snapshot_authentication_is_challenged_and_digest_is_preferred() {
        for challenge in [
            "Basic realm=\"camera\"",
            "Digest realm=\"camera\", nonce=\"test-nonce\", qop=\"auth\", algorithm=MD5",
        ] {
            let (url, task) = http_server(vec![
                response(
                    "401 Unauthorized",
                    &format!("WWW-Authenticate: {challenge}\r\n"),
                    b"",
                ),
                response("200 OK", "", b"\xff\xd8\xff\xd9"),
            ])
            .await;
            let mut target = resolved(&url);
            target.username = Some("admin".into());
            target.password = Some(crate::SecretString::new("test-password").unwrap());
            assert_eq!(
                fetch_image(&url, &target, &options()).await.unwrap().1,
                "jpeg"
            );
            let requests = task.await.unwrap();
            assert!(!requests[0].to_ascii_lowercase().contains("authorization:"));
            assert!(requests[1].to_ascii_lowercase().contains(
                if challenge.starts_with("Digest") {
                    "authorization: digest"
                } else {
                    "authorization: basic"
                }
            ));
            if challenge.starts_with("Digest") {
                assert!(requests[1].contains("/image?ticket=never-log-this"));
                assert!(!requests[1].contains("test-password"));
            }
        }
    }

    #[tokio::test]
    async fn snapshot_rejects_redirect_non_image_and_oversize_without_leaking_urls() {
        for reply in [
            response("302 Found", "Location: http://foreign.test/steal\r\n", b""),
            response("200 OK", "", b"<html>never-log-this</html>"),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_IMAGE_BYTES + 1
            )
            .into_bytes(),
            response(
                "401 Unauthorized",
                "WWW-Authenticate: Bearer\r\n",
                b"never-log-this",
            ),
        ] {
            let (url, task) = http_server(vec![reply]).await;
            let error = fetch_image(&url, &resolved(&url), &options())
                .await
                .unwrap_err();
            assert!(!error.message.contains("never-log-this"));
            assert!(!error.message.contains(&url));
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn chunked_snapshot_limit_is_enforced_without_content_length() {
        let body = vec![b'x'; MAX_IMAGE_BYTES + 1];
        let mut reply = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n",
            body.len()
        )
        .into_bytes();
        reply.extend_from_slice(&body);
        reply.extend_from_slice(b"\r\n0\r\n\r\n");
        let (url, task) = http_server(vec![reply]).await;
        let error = fetch_image(&url, &resolved(&url), &options())
            .await
            .unwrap_err();
        assert!(error.message.contains("16 MiB"));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn stalled_snapshot_times_out_without_publishing_files() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/image", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
        });
        let options = ExecutionOptions {
            timeout: Duration::from_millis(100),
            ..options()
        };
        let error = fetch_image(&url, &resolved(&url), &options)
            .await
            .unwrap_err();
        assert!(error.retryable);
        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn mock_camera_snapshot_export_diff_and_partial_diagnosis() {
        let server = oxvif::mock::MockServer::start().await.unwrap();
        let directory = tempfile::tempdir().unwrap();
        let app = Application::with_stores(
            RegistryStore::at(directory.path()),
            Arc::new(MemoryCredentialStore::default()),
        );
        let snapshot = directory.path().join("camera.bmp");
        let success = app
            .execute(
                CommandRequest::MediaSnapshotSave(SnapshotSaveRequest {
                    selector: selector(server.device_url()),
                    profile: "Profile_1".into(),
                    save: snapshot.clone(),
                }),
                &options(),
            )
            .await
            .unwrap();
        assert_eq!(success.exit_code(), 0);
        assert_eq!(result(&success)["image_type"], "bmp");
        assert!(fs::read(snapshot).unwrap().starts_with(b"BM"));
        let baseline = directory.path().join("baseline.json");
        let export = app
            .execute(
                CommandRequest::ConfigExport(ConfigExportRequest {
                    selector: selector(server.device_url()),
                    save: baseline.clone(),
                }),
                &options(),
            )
            .await
            .unwrap();
        assert_eq!(export.exit_code(), 0, "{}", result(&export));
        let diff = app
            .execute(
                CommandRequest::ConfigDiff(ConfigDiffRequest {
                    selector: selector(server.device_url()),
                    against: baseline.clone(),
                }),
                &options(),
            )
            .await
            .unwrap();
        assert_eq!(result(&diff)["matches"], true);
        server.inject_fault("GetNTP", "s:Receiver", "secret raw fault");
        let diff = app
            .execute(
                CommandRequest::ConfigDiff(ConfigDiffRequest {
                    selector: selector(server.device_url()),
                    against: baseline,
                }),
                &options(),
            )
            .await
            .unwrap();
        assert_eq!(diff.exit_code(), 20);
        assert_eq!(result(&diff)["matches"], Value::Null);
        assert_eq!(result(&diff)["incomparable_sections"], json!(["ntp"]));
        server.inject_fault("GetStreamUri", "s:Receiver", "secret raw fault");
        let diagnostic = app
            .execute(
                CommandRequest::Diagnose(DiagnoseRequest {
                    selector: selector(server.device_url()),
                    profile: Some("Profile_1".into()),
                }),
                &options(),
            )
            .await
            .unwrap();
        assert_eq!(diagnostic.exit_code(), 20);
        let stages = result(&diagnostic)["stages"].as_array().unwrap();
        assert!(
            stages
                .iter()
                .any(|s| s["name"] == "snapshot_fetch" && s["status"] == "pass")
        );
        assert!(
            stages
                .iter()
                .any(|s| s["name"] == "stream_uri" && s["status"] == "fail")
        );
        assert!(
            !serde_json::to_string(result(&diagnostic))
                .unwrap()
                .contains("secret raw fault")
        );
        let no_profile = app
            .execute(
                CommandRequest::Diagnose(DiagnoseRequest {
                    selector: selector(server.device_url()),
                    profile: None,
                }),
                &options(),
            )
            .await
            .unwrap();
        assert_eq!(no_profile.exit_code(), 20);
        assert_eq!(result(&no_profile)["playback_verified"], false);
    }

    fn inventory() -> Inventory {
        Inventory {
            inventory_version: 1,
            identity: Identity {
                manufacturer: "test".into(),
                model: "test".into(),
                serial_number: "123".into(),
                hardware_id: "rev1".into(),
            },
            sections: SECTIONS
                .iter()
                .map(|name| {
                    let mut step = Step::new(name, Status::Pass, "", "");
                    step.data = Some(
                        if matches!(
                            *name,
                            "interfaces" | "protocols" | "profiles" | "video_encoders"
                        ) {
                            json!([])
                        } else {
                            json!({"value": 1})
                        },
                    );
                    ((*name).into(), step)
                })
                .collect(),
        }
    }

    #[test]
    fn inventory_diff_ignores_order_but_detects_changes_missing_data_and_identity() {
        let mut before = inventory();
        before.sections.get_mut("profiles").unwrap().data =
            Some(json!([{"token":"B"},{"token":"A"}]));
        let mut after = before.clone();
        after.sections.get_mut("profiles").unwrap().data =
            Some(json!([{"token":"A"},{"token":"B"}]));
        assert_eq!(compare(&before, &after).unwrap()["matches"], true);
        after.sections.get_mut("ntp").unwrap().data = Some(json!({"value": 2}));
        let diff = compare(&before, &after).unwrap();
        assert_eq!(diff["changes"][0]["path"], "/ntp/value");
        assert_eq!(diff["matches"], false);
        assert!(!report_failed("config.diff", &diff));
        after.sections.get_mut("ntp").unwrap().status = Status::Unsupported;
        assert!(report_failed(
            "config.diff",
            &compare(&before, &after).unwrap()
        ));
        after.identity.serial_number = "other".into();
        assert!(compare(&before, &after).is_err());
        let mut empty = inventory();
        empty.identity.serial_number.clear();
        assert!(compare(&empty, &empty).is_err());
    }

    #[test]
    fn baseline_validation_limits_size_and_rejects_invalid_versions_and_sections() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("baseline.json");
        let mut baseline = inventory();
        fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();
        assert!(read_baseline(&path).is_ok());
        baseline.inventory_version = 2;
        fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();
        assert!(read_baseline(&path).is_err());
        baseline.inventory_version = 1;
        baseline.sections.get_mut("ntp").unwrap().data = None;
        fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();
        assert!(read_baseline(&path).is_err());
        fs::write(&path, vec![b' '; MAX_INVENTORY_BYTES as usize + 1]).unwrap();
        assert!(read_baseline(&path).unwrap_err().message.contains("4 MiB"));
    }

    #[test]
    fn normalized_inventory_excludes_sensitive_fields_and_urls() {
        let mut value = json!({"password":"secret", "nested":{"uri":"rtsp://u:p@camera/x?secret"}, "name":"https://u:p@camera/?secret", "value":123});
        normalize(&mut value);
        let text = value.to_string();
        assert!(!text.contains("secret"));
        assert_eq!(value["value"], 123);
    }

    #[tokio::test]
    async fn snapshot_private_ca_keeps_hostname_verification_enabled() {
        use tokio_rustls::{
            TlsAcceptor,
            rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer},
        };
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let pem = certified.cert.pem().into_bytes();
        let tls = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certified.cert.der().clone()],
                PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der()).into(),
            )
            .unwrap();
        let acceptor = TlsAcceptor::from(Arc::new(tls));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let Ok(mut stream) = acceptor.accept(socket).await else {
                    continue;
                };
                let mut bytes = [0; 4096];
                let _ = stream.read(&mut bytes).await;
                stream
                    .write_all(&response("200 OK", "", b"\xff\xd8\xff\xd9"))
                    .await
                    .unwrap();
                break;
            }
        });
        let url = format!("https://localhost:{port}/image");
        assert!(
            fetch_image(&url, &resolved(&url), &options())
                .await
                .is_err()
        );
        let trusted = ExecutionOptions {
            ca_certificates: vec![pem],
            ..options()
        };
        let wrong_host = format!("https://127.0.0.1:{port}/image");
        assert!(
            fetch_image(&wrong_host, &resolved(&wrong_host), &trusted)
                .await
                .is_err()
        );
        assert_eq!(
            fetch_image(&url, &resolved(&url), &trusted)
                .await
                .unwrap()
                .1,
            "jpeg"
        );
        task.await.unwrap();
    }

    #[tokio::test]
    async fn stage_retry_and_timeout_do_not_hide_unsupported_or_fault_details() {
        let attempts = std::sync::atomic::AtomicUsize::new(0);
        let options = ExecutionOptions {
            retries: 1,
            ..options()
        };
        let (step, value) = onvif_step("test", &options, || async {
            if attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                Err(oxvif::transport::TransportError::HttpStatus {
                    status: 503,
                    body: "secret".into(),
                }
                .into())
            } else {
                Ok(123)
            }
        })
        .await;
        assert!(matches!(step.status, Status::Pass));
        assert_eq!(value, Some(123));
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
        let (unsupported, _) = onvif_step::<(), _, _>("test", &options, || async {
            Err(OnvifError::Soap(SoapError::Fault {
                code: "s:Sender".into(),
                reason: "secret".into(),
                subcode: Some("ter:ActionNotSupported".into()),
                detail: None,
            }))
        })
        .await;
        assert!(matches!(unsupported.status, Status::Unsupported));
        assert!(!unsupported.detail.contains("secret"));
        let options = ExecutionOptions {
            timeout: Duration::from_millis(10),
            ..ExecutionOptions::default()
        };
        let (timed_out, _) = onvif_step::<(), _, _>("test", &options, std::future::pending).await;
        assert_eq!(timed_out.error_code.as_deref(), Some("TIMEOUT"));
    }

    #[test]
    fn normalization_preserves_dns_preference_and_json_pointer_escaping() {
        let mut value = json!(["secondary", "primary"]);
        normalize(&mut value);
        assert_eq!(value, json!(["secondary", "primary"]));
        let mut changes = Vec::new();
        diff_value(
            "",
            Some(&json!({"a/b~": null})),
            Some(&json!({})),
            &mut changes,
        );
        assert_eq!(changes[0]["path"], "/a~1b~0");
        assert_eq!(changes[0]["before_present"], true);
        assert_eq!(changes[0]["after_present"], false);
    }

    #[tokio::test]
    async fn cancelling_a_download_leaves_no_artifact_or_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("cancelled.jpg");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/image", listener.local_addr().unwrap());
        let (connected, ready) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            connected.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        let download = tokio::spawn(async move {
            let (bytes, _) = fetch_image(&url, &resolved(&url), &options()).await?;
            save_new(&destination, &bytes)
        });
        ready.await.unwrap();
        download.abort();
        assert!(download.await.unwrap_err().is_cancelled());
        server.abort();
        let _ = server.await;
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn fleet_diagnose_retains_partial_and_total_failure_evidence() {
        let server = oxvif::mock::MockServer::start().await.unwrap();
        let directory = tempfile::tempdir().unwrap();
        let registry = RegistryStore::at(directory.path());
        registry
            .create_group(crate::NewGroup {
                id: "all".into(),
                name: None,
            })
            .unwrap();
        for (id, target) in [
            ("good", server.device_url()),
            ("bad", "http://127.0.0.1:1/onvif/device_service"),
        ] {
            registry
                .add(crate::NewDevice {
                    id: id.into(),
                    name: None,
                    target: target.into(),
                    tags: vec![],
                })
                .unwrap();
            registry.add_group_member("all", id, id).unwrap();
        }
        let app = Application::with_stores(registry, Arc::new(MemoryCredentialStore::default()));
        let request = || {
            CommandRequest::Diagnose(DiagnoseRequest {
                selector: TargetSelector {
                    group: Some("all".into()),
                    ..Default::default()
                },
                profile: Some("Profile_1".into()),
            })
        };
        let report = app.execute(request(), &options()).await.unwrap();
        assert_eq!(report.exit_code(), 6);
        let jsonl = crate::render_success(crate::OutputFormat::JsonLines, &report).unwrap();
        let lines: Vec<Value> = jsonl
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["data"]["item"]["device_id"], "bad");
        assert_eq!(lines[0]["ok"], false);
        assert!(lines[0]["data"]["item"]["result"]["stages"].is_array());
        assert_eq!(lines[2]["data"]["failed"], 1);
        drop(server);
        let report = app.execute(request(), &options()).await.unwrap();
        assert_eq!(report.exit_code(), 20);
        let CommandData::FleetDiagnostic { items, .. } = &report.data else {
            panic!("expected fleet report")
        };
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| !item.ok && item.result.is_some()));
    }
}
