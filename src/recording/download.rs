//! Bounded transfer of an explicitly supplied recording-export download URL.

use std::{future::Future, path::Path, time::Duration};

use tokio::io::AsyncWriteExt;

use crate::transport::EndpointPolicy;

/// Resource and certificate policy for one exported-file transfer.
#[derive(Clone)]
pub struct DownloadOptions {
    /// Total time for connection, transfer and temporary-file writes.
    pub timeout: Duration,
    /// Maximum bytes written to the temporary file.
    pub max_bytes: u64,
    /// Additional PEM CA bundles.
    pub ca_certificates: Vec<Vec<u8>>,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            max_bytes: 256 * 1024 * 1024,
            ca_certificates: Vec::new(),
        }
    }
}

/// Controlled transfer failure without raw URLs, headers or response text.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DownloadError {
    /// The URL, CA configuration, local path or requested limits were refused.
    #[error("invalid export download configuration")]
    Invalid,
    /// Another file or directory already occupies the destination.
    #[error("export destination exists")]
    Exists,
    /// Request or body transfer failed.
    #[error("export download transport failed")]
    Transport,
    /// A non-200 status, including redirects and partial responses.
    #[error("export download refused with status {0}")]
    Refused(u16),
    /// Reported or received size exceeded the limit.
    #[error("export download exceeds byte limit")]
    Limit,
    /// Deadline expired.
    #[error("export download timed out")]
    Deadline,
    /// Cancellation was signalled before publishing the file.
    #[error("export download cancelled")]
    Cancelled,
    /// Temporary-file write, flush or publish failed.
    #[error("export download could not write destination")]
    Io,
}

/// Download to a temporary file beside the destination and publish without
/// replacing an existing entry. ONVIF export file names are not URLs. Supply
/// an independently trusted URL and an explicit local destination. No
/// camera credentials, redirects or system proxy are used.
pub async fn download_until(
    uri: &str,
    trusted_endpoint: &str,
    destination: &Path,
    options: &DownloadOptions,
    cancel: impl Future<Output = ()>,
) -> Result<u64, DownloadError> {
    if uri.len() > 8192
        || destination.file_name().is_none()
        || options.max_bytes == 0
        || options.timeout.is_zero()
    {
        return Err(DownloadError::Invalid);
    }
    let url = EndpointPolicy::default()
        .check(trusted_endpoint, uri)
        .map_err(|_| DownloadError::Invalid)?;
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let transfer = async {
        match tokio::fs::symlink_metadata(destination).await {
            Ok(_) => return Err(DownloadError::Exists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(DownloadError::Io),
        }
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy();
        for bundle in &options.ca_certificates {
            let certs = reqwest::Certificate::from_pem_bundle(bundle)
                .map_err(|_| DownloadError::Invalid)?;
            if certs.is_empty() {
                return Err(DownloadError::Invalid);
            }
            builder = builder.tls_certs_merge(certs);
        }
        let client = builder.build().map_err(|_| DownloadError::Invalid)?;
        let mut response = client
            .get(url)
            .send()
            .await
            .map_err(|_| DownloadError::Transport)?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(DownloadError::Refused(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|len| len > options.max_bytes)
        {
            return Err(DownloadError::Limit);
        }
        let temp = tempfile::Builder::new()
            .prefix(".oxvif-export-")
            .tempfile_in(parent)
            .map_err(|_| DownloadError::Io)?;
        let (file, path) = temp.into_parts();
        let mut file = tokio::fs::File::from_std(file);
        let mut written = 0u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| DownloadError::Transport)?
        {
            written = written
                .checked_add(chunk.len() as u64)
                .ok_or(DownloadError::Limit)?;
            if written > options.max_bytes {
                return Err(DownloadError::Limit);
            }
            file.write_all(&chunk)
                .await
                .map_err(|_| DownloadError::Io)?;
        }
        file.flush().await.map_err(|_| DownloadError::Io)?;
        drop(file);
        Ok((path, written))
    };
    let (path, written) = tokio::select! {
        biased;
        () = cancel => return Err(DownloadError::Cancelled),
        result = tokio::time::timeout(options.timeout, transfer) => result.map_err(|_| DownloadError::Deadline)??,
    };
    path.persist_noclobber(destination).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            DownloadError::Exists
        } else {
            DownloadError::Io
        }
    })?;
    Ok(written)
}
