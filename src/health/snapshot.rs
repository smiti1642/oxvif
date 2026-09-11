//! Bounded snapshot downloads shared by health checks and the CLI.
//!
//! Only HTTP(S) on the device host is accepted, without userinfo, fragments,
//! redirects, proxies or HTTPS downgrade. Digest is preferred; a failed or
//! unsupported Digest challenge never triggers Basic fallback. HTTP/1.1 uses
//! title-case field names for firmware interoperability, without weakening auth.
//! A recognized image signature is not proof of decoding or ONVIF conformance.

use digest_auth::{Algorithm, AuthContext, Charset, HttpMethod, Qop, WwwAuthenticateHeader};
use reqwest::{Url, header};
use std::{collections::BTreeMap, time::Duration};
use thiserror::Error;

/// Maximum downloaded snapshot size, including chunked responses.
pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;

/// Download deadline and additional trust roots. Credentials are passed separately.
#[derive(Clone, Debug)]
pub struct SnapshotOptions {
    /// Total deadline covering all authentication attempts and body reads.
    pub timeout: Duration,
    /// PEM certificate bundles added to normal trust roots; never disables verification.
    pub ca_certificates: Vec<Vec<u8>>,
}
impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            ca_certificates: Vec::new(),
        }
    }
}

/// URL/body-free error safe for diagnostic reports.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct SnapshotError {
    message: String,
    invalid_argument: bool,
    retryable: bool,
}
impl SnapshotError {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            invalid_argument: false,
            retryable: false,
        }
    }
    fn invalid(message: &'static str) -> Self {
        Self {
            invalid_argument: true,
            ..Self::failed(message)
        }
    }
    /// Whether this error is invalid configuration rather than a device failure.
    pub fn is_invalid_argument(&self) -> bool {
        self.invalid_argument
    }
    /// Whether a caller may retry the operation within its own retry budget.
    pub fn is_retryable(&self) -> bool {
        self.retryable
    }
}
fn http_error(e: reqwest::Error) -> SnapshotError {
    SnapshotError {
        retryable: e.is_timeout() || e.is_connect(),
        ..SnapshotError::failed(if e.is_timeout() {
            "Snapshot HTTP request timed out (URL withheld)."
        } else {
            "Snapshot HTTP request failed (URL/body withheld)."
        })
    }
}

/// Validate the snapshot destination before sending any network request.
///
/// The same hostname/IP is required; a different port is permitted for camera
/// HTTP services. This is a product credential-boundary policy, not an ONVIF rule.
/// Hostname resolution and HTTPS certificate validation remain the HTTP client's job.
pub fn validate_url(uri: &str, device: &str) -> Result<Url, SnapshotError> {
    let url =
        Url::parse(uri).map_err(|_| SnapshotError::invalid("Invalid snapshot URL (withheld)."))?;
    let device =
        Url::parse(device).map_err(|_| SnapshotError::invalid("Invalid device target."))?;
    if !matches!(device.scheme(), "http" | "https")
        || !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.host_str() != device.host_str()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || (device.scheme() == "https" && url.scheme() != "https")
    {
        return Err(SnapshotError::invalid(
            "Snapshot URL rejected: require HTTP(S), the device host, no userinfo/fragment and no HTTPS downgrade.",
        ));
    }
    Ok(url)
}

/// Download a snapshot and identify its signature, without full image decoding.
///
/// Returns bytes and "jpeg", "png" or "bmp". PNG/BMP acceptance is a compatibility
/// extension; ONVIF snapshots require JPEG. At most one fresh Digest attempt and
/// one same-realm/algorithm, new-nonce stale retry are performed. Basic is used
/// only when explicitly offered and no Digest challenge is present.
///
/// # Errors
///
/// Rejects unsafe destinations, malformed trust roots, unsupported authentication,
/// HTTP status other than 200, oversized bodies, timeouts and unrecognized images.
/// No error includes a URI, credentials, challenge or response body.
pub async fn fetch(
    uri: &str,
    device: &str,
    credentials: Option<(&str, &str)>,
    options: &SnapshotOptions,
) -> Result<(Vec<u8>, &'static str), SnapshotError> {
    let url = validate_url(uri, device)?;
    if options.timeout.is_zero() {
        return Err(SnapshotError::invalid(
            "Snapshot timeout must be greater than zero.",
        ));
    }
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .http1_title_case_headers()
        .timeout(options.timeout);
    for pem in &options.ca_certificates {
        if pem
            .windows(b"PRIVATE KEY".len())
            .any(|w| w == b"PRIVATE KEY")
        {
            return Err(SnapshotError::invalid("Invalid private CA bundle."));
        }
        let certs = reqwest::Certificate::from_pem_bundle(pem)
            .map_err(|_| SnapshotError::invalid("Invalid private CA bundle."))?;
        if certs.is_empty() {
            return Err(SnapshotError::invalid("Empty private CA bundle."));
        }
        for cert in certs {
            builder = builder.add_root_certificate(cert);
        }
    }
    let client = builder
        .build()
        .map_err(|_| SnapshotError::failed("Cannot initialize snapshot HTTP client."))?;
    let download = async {
        let mut response = client.get(url.clone()).send().await.map_err(http_error)?;
        if response.status().as_u16() == 401
            && let Some((user, password)) = credentials
        {
            let selection = select_challenge(response.headers())?;
            let request_uri = match url.query() {
                Some(q) => format!("{}?{q}", url.path()),
                None => url.path().to_owned(),
            };
            match selection {
                Challenge::Digest(mut prompt) => {
                    for attempt in 0..2 {
                        // An empty GET entity is still a body for auth-int hashing.
                        let context = AuthContext::new_with_method(
                            user,
                            password,
                            &request_uri,
                            Some(&[][..]),
                            HttpMethod::GET,
                        );
                        let answer = prompt.respond(&context).map_err(|_| {
                            SnapshotError::failed("Cannot answer snapshot Digest challenge.")
                        })?;
                        response = client
                            .get(url.clone())
                            .header(header::AUTHORIZATION, answer.to_header_string())
                            .send()
                            .await
                            .map_err(http_error)?;
                        if response.status().as_u16() != 401 || attempt == 1 {
                            break;
                        }
                        let next = match select_challenge(response.headers()) {
                            Ok(Challenge::Digest(next)) => next,
                            _ => break,
                        };
                        // Never retry bad credentials, change realms/algorithms, or loop.
                        if !next.stale
                            || next.nonce == prompt.nonce
                            || next.realm != prompt.realm
                            || next.algorithm != prompt.algorithm
                            || next.qop != prompt.qop
                            || next.userhash != prompt.userhash
                            || next.charset != prompt.charset
                        {
                            break;
                        }
                        prompt = next;
                    }
                }
                Challenge::Basic => {
                    response = client
                        .get(url.clone())
                        .basic_auth(user, Some(password))
                        .send()
                        .await
                        .map_err(http_error)?;
                }
            }
        }
        if response.status().as_u16() != 200 {
            return Err(SnapshotError::failed(format!(
                "Snapshot returned HTTP {}; redirects are not followed.",
                response.status().as_u16()
            )));
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_IMAGE_BYTES as u64)
        {
            return Err(SnapshotError::failed("Snapshot exceeds 16 MiB."));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(http_error)? {
            if chunk.len() > MAX_IMAGE_BYTES - bytes.len() {
                return Err(SnapshotError::failed("Snapshot exceeds 16 MiB."));
            }
            bytes.extend_from_slice(&chunk);
        }
        let kind = image_type(&bytes).ok_or_else(|| {
            SnapshotError::failed(
                "Snapshot is empty, truncated or has no supported JPEG/PNG/BMP signature.",
            )
        })?;
        Ok((bytes, kind))
    };
    tokio::time::timeout(options.timeout, download)
        .await
        .map_err(|_| SnapshotError {
            retryable: true,
            ..SnapshotError::failed("Snapshot exceeded its total download timeout.")
        })?
}

/// Identify a bounded image signature. This does not decode the image.
pub fn image_type(bytes: &[u8]) -> Option<&'static str> {
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

enum Challenge {
    Digest(WwwAuthenticateHeader),
    Basic,
}

// Split lists without treating commas or escaped quotes inside strings as syntax.
fn comma_parts(input: &str) -> Option<Vec<&str>> {
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    let mut parts = Vec::new();
    for (i, c) in input.char_indices() {
        if c.is_control() && c != '\t' {
            return None;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ',' if !quoted => {
                parts.push(input[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if quoted || escaped {
        return None;
    }
    parts.push(input[start..].trim());
    Some(parts)
}
fn token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == 96 || b"!#$%&'*+-.^_|~".contains(&b))
}
fn parameter(part: &str) -> Option<(String, String)> {
    let (name, raw) = part.split_once('=')?;
    let name = name.trim();
    let raw = raw.trim();
    if !token(name) {
        return None;
    }
    let value = if let Some(inner) = raw.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        let mut value = String::new();
        let mut escaped = false;
        for c in inner.chars() {
            if escaped {
                value.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                return None;
            } else {
                value.push(c);
            }
        }
        if escaped {
            return None;
        }
        value
    } else if token(raw) {
        raw.to_owned()
    } else {
        return None;
    };
    Some((name.to_ascii_lowercase(), value))
}
fn digest(parts: &[&str]) -> Option<WwwAuthenticateHeader> {
    let mut fields = BTreeMap::new();
    for part in parts {
        let (key, value) = parameter(part)?;
        if fields.insert(key, value).is_some() {
            return None;
        }
    }
    let realm = fields.remove("realm")?;
    let nonce = fields.remove("nonce").filter(|s| !s.is_empty())?;
    let algorithm = fields
        .get("algorithm")
        .map_or(Some(Algorithm::default()), |s| s.parse().ok())?;
    let qop = match fields.get("qop") {
        Some(q) => {
            let supported: Vec<Qop> = q
                .split(',')
                .filter_map(|v| match v.trim() {
                    "auth" => Some(Qop::AUTH),
                    "auth-int" => Some(Qop::AUTH_INT),
                    _ => None,
                })
                .collect();
            if supported.is_empty() {
                return None;
            }
            Some(vec![if supported.contains(&Qop::AUTH) {
                Qop::AUTH
            } else {
                Qop::AUTH_INT
            }])
        }
        None if algorithm.sess => return None,
        None => None,
    };
    let charset = match fields.get("charset").map(String::as_str) {
        None => Charset::ASCII,
        Some("UTF-8") => Charset::UTF8,
        _ => return None,
    };
    Some(WwwAuthenticateHeader {
        domain: None,
        realm,
        nonce,
        opaque: fields.remove("opaque"),
        stale: fields
            .get("stale")
            .is_some_and(|v| v.eq_ignore_ascii_case("true")),
        algorithm,
        qop,
        charset,
        userhash: fields
            .get("userhash")
            .is_some_and(|v| v.eq_ignore_ascii_case("true")),
        nc: 0,
    })
}
fn select_challenge(headers: &header::HeaderMap) -> Result<Challenge, SnapshotError> {
    let unsupported = || SnapshotError::failed("Unsupported snapshot authentication challenge.");
    let mut groups: Vec<(String, Vec<&str>)> = Vec::new();
    for value in headers.get_all(header::WWW_AUTHENTICATE) {
        let value = value.to_str().map_err(|_| unsupported())?;
        let mut current: Option<(String, Vec<&str>)> = None;
        for part in comma_parts(value).ok_or_else(unsupported)? {
            if part.is_empty() {
                continue;
            }
            if parameter(part).is_some() {
                current.as_mut().ok_or_else(unsupported)?.1.push(part);
            } else {
                if let Some(previous) = current.take() {
                    groups.push(previous);
                }
                let (scheme, rest) = part.split_once(char::is_whitespace).unwrap_or((part, ""));
                if !token(scheme) {
                    return Err(unsupported());
                }
                current = Some((
                    scheme.to_ascii_lowercase(),
                    if rest.trim().is_empty() {
                        Vec::new()
                    } else {
                        vec![rest.trim()]
                    },
                ));
            }
        }
        if let Some(previous) = current {
            groups.push(previous);
        }
    }
    let mut offered_digest = false;
    for (scheme, parts) in &groups {
        if scheme == "digest" {
            offered_digest = true;
            if let Some(prompt) = digest(parts) {
                return Ok(Challenge::Digest(prompt));
            }
        }
    }
    if !offered_digest
        && groups.iter().any(|(s, p)| {
            s == "basic"
                && p.iter()
                    .any(|v| parameter(v).is_some_and(|(k, _)| k == "realm"))
        })
    {
        return Ok(Challenge::Basic);
    }
    Err(unsupported())
}

#[cfg(test)]
mod tests;
