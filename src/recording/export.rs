use std::{future::Future, sync::Arc, time::Duration};

use crate::{
    OnvifClient, OnvifError, OnvifSession,
    configuration_write::ConfigurationWriteError,
    scoped_rpc::{SCHEMA, ScopedService, field, required, scalar, text, token},
    soap::{
        SoapError,
        qualified::{EXTENSION_BYTES, QualifiedElement, invalid},
    },
    transport::EndpointPolicy,
};

use super::{RecordingScope, RecordingTimestamp};

const RECORDING: &str = "http://www.onvif.org/ver10/recording/wsdl";
const MAX_FILES: usize = 512;

/// The explicit ONVIF export request and remote storage destination.
#[derive(Clone, Debug)]
pub struct ExportRequest {
    /// Inclusive start of the requested range, if specified.
    pub start: Option<RecordingTimestamp>,
    /// End of the requested range, if specified.
    pub end: Option<RecordingTimestamp>,
    /// Which recordings and sources to export.
    pub scope: RecordingScope,
    /// Export format advertised by the Recording service.
    pub file_format: String,
    /// Token of a provisioned device storage configuration.
    pub storage_token: String,
    /// Relative destination directory on the configured storage, if specified.
    pub relative_directory: Option<String>,
}

/// Progress reported for one exported file on the device's storage.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportFileProgress {
    /// Reported file name. Treat it as device data.
    pub file_name: String,
    /// Completion fraction between 0 and 1.
    pub progress: f32,
}

/// Snapshot of one operation's reported progress.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportProgress {
    /// Aggregate completion fraction between 0 and 1.
    pub progress: f32,
    /// Per-file progress in device order.
    pub files: Vec<ExportFileProgress>,
}

/// Bounded observation of an export job. Cancel uses a separate deadline.
#[derive(Clone, Copy, Debug)]
pub struct ExportPollOptions {
    /// Total polling time.
    pub timeout: Duration,
    /// Minimum time between polls, at least 100 ms.
    pub interval: Duration,
    /// Maximum number of progress calls, at least one.
    pub max_polls: usize,
}

impl Default for ExportPollOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            interval: Duration::from_secs(1),
            max_polls: 120,
        }
    }
}

/// Why a bounded wait did not observe export completion.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExportPollError {
    /// Invalid polling budgets.
    #[error("invalid export poll limits")]
    InvalidOptions,
    /// Total polling time elapsed.
    #[error("export polling deadline elapsed")]
    Deadline,
    /// Poll count reached its limit.
    #[error("export poll limit reached")]
    PollLimit,
    /// The caller cancelled observation and attempted one explicit stop.
    #[error("export polling cancelled")]
    Cancelled {
        /// Independent stop acknowledgement or error.
        stop: Result<ExportProgress, ConfigurationWriteError>,
    },
    /// Progress query failed.
    #[error("export progress query failed")]
    Query(#[source] OnvifError),
}

/// A trusted ONVIF Recording service.
pub struct ExportService {
    service: Arc<ScopedService>,
}

impl OnvifClient {
    /// Bind exports to a Recording service on the configured device origin.
    pub fn recording_exports(&self, endpoint: &str) -> Result<ExportService, OnvifError> {
        let url = EndpointPolicy::default().check(self.device_url(), endpoint)?;
        Ok(ExportService::new(self, url))
    }
}

impl OnvifSession {
    /// Bind exports to the session's trusted Recording endpoint.
    pub fn recording_exports(&self) -> Result<ExportService, OnvifError> {
        let client = self.client();
        let url = self
            .endpoint_policy
            .check(client.device_url(), self.recording_url()?)?;
        Ok(ExportService::new(client, url))
    }
}

impl ExportService {
    fn new(client: &OnvifClient, url: reqwest::Url) -> Self {
        Self {
            service: Arc::new(ScopedService {
                client: client.direct(),
                url,
                namespace: RECORDING,
            }),
        }
    }

    /// Read Recording capabilities, including supported export file formats.
    pub async fn capabilities(
        &self,
    ) -> Result<crate::types::RecordingServiceCapabilities, OnvifError> {
        self.service
            .client
            .recording_get_service_capabilities(self.service.url.as_str())
            .await
    }

    /// Start one job. A missing or malformed response can mean the remote
    /// outcome is unknown. The server chooses the job token and file names.
    pub async fn start(
        &self,
        request: &ExportRequest,
    ) -> Result<ExportJob, ConfigurationWriteError> {
        use ConfigurationWriteError as Error;
        let fields = export_fields(request).map_err(Error::Preflight)?;
        let response = self
            .service
            .call("ExportRecordedData", "ExportRecordedDataResponse", &fields)
            .await
            .map_err(Error::after_send)?;
        let operation_token = text(&response, RECORDING, "OperationToken")
            .map_err(|error| Error::OutcomeUnknown(error.into()))?;
        token(&operation_token).map_err(Error::OutcomeUnknown)?;
        // A valid job token can still be used for cleanup when the names fail to parse.
        let files = file_names(&response);
        Ok(ExportJob {
            service: self.service.clone(),
            operation_token,
            files,
            stop_attempted: false,
        })
    }
}

/// Owner of an acknowledged remote export. Drop releases local state only.
pub struct ExportJob {
    service: Arc<ScopedService>,
    operation_token: String,
    files: Result<Vec<String>, SoapError>,
    stop_attempted: bool,
}

impl ExportJob {
    /// Device-assigned operation token.
    pub fn token(&self) -> &str {
        &self.operation_token
    }

    /// Initially reported file names. The job remains manageable if these
    /// fields were malformed. A file name does not provide a download URL.
    pub fn initial_file_names(&self) -> Result<&[String], &SoapError> {
        self.files.as_deref()
    }

    /// Read the latest job and per-file progress without side effects.
    pub async fn progress(&self) -> Result<ExportProgress, OnvifError> {
        self.status(
            "GetExportRecordedDataState",
            "GetExportRecordedDataStateResponse",
        )
        .await
    }

    /// Attempt one StopExportRecordedData within a separate two-second budget.
    /// Subsequent calls refuse to send a duplicate stop for this owner.
    pub async fn cancel(&mut self) -> Result<ExportProgress, ConfigurationWriteError> {
        use ConfigurationWriteError as Error;
        if self.stop_attempted {
            return Err(Error::invalid("export stop already attempted"));
        }
        self.stop_attempted = true;
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            self.status("StopExportRecordedData", "StopExportRecordedDataResponse"),
        )
        .await
        .map_err(|_| Error::OutcomeUnknown(crate::transport::TransportError::Deadline.into()))?;
        result.map_err(Error::after_send)
    }

    /// Poll with time, interval and count limits. If `cancel` resolves,
    /// attempt Stop and return its separate outcome. A polling failure leaves
    /// the job available for explicit reconciliation.
    pub async fn wait_until(
        &mut self,
        options: ExportPollOptions,
        cancel: impl Future<Output = ()>,
    ) -> Result<ExportProgress, ExportPollError> {
        if options.timeout.is_zero()
            || options.interval < Duration::from_millis(100)
            || options.max_polls == 0
        {
            return Err(ExportPollError::InvalidOptions);
        }
        let wait = async {
            for i in 0..options.max_polls {
                if i != 0 {
                    tokio::time::sleep(options.interval).await;
                }
                let progress = self.progress().await.map_err(ExportPollError::Query)?;
                if progress.progress == 1.0 {
                    return Ok(progress);
                }
            }
            Err(ExportPollError::PollLimit)
        };
        tokio::select! {
            biased;
            () = cancel => {},
            result = tokio::time::timeout(options.timeout, wait) => {
                return result.unwrap_or(Err(ExportPollError::Deadline));
            }
        }
        Err(ExportPollError::Cancelled {
            stop: self.cancel().await,
        })
    }

    async fn status(
        &self,
        operation: &'static str,
        response: &'static str,
    ) -> Result<ExportProgress, OnvifError> {
        let response = self
            .service
            .call(
                operation,
                response,
                &field("OperationToken", &self.operation_token),
            )
            .await?;
        parse_progress(&response).map_err(Into::into)
    }
}

fn export_fields(request: &ExportRequest) -> Result<String, OnvifError> {
    if request
        .start
        .as_ref()
        .zip(request.end.as_ref())
        .is_some_and(|(a, b)| a.instant() >= b.instant())
    {
        return Err(OnvifError::InvalidArgument(
            "export start must precede end".into(),
        ));
    }
    if request.file_format.is_empty()
        || request.file_format.len() > 64
        || request
            .file_format
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(OnvifError::InvalidArgument(
            "invalid export file format".into(),
        ));
    }
    let mut fields = String::new();
    for (name, value) in [
        ("StartPoint", request.start.as_ref()),
        ("EndPoint", request.end.as_ref()),
    ] {
        if let Some(value) = value {
            fields.push_str(&field(name, value.as_str()));
        }
    }
    fields.push_str(&format!(
        "<SearchScope>{}</SearchScope>",
        request.scope.fields()?
    ));
    fields.push_str(&field("FileFormat", &request.file_format));
    token(&request.storage_token)?;
    fields.push_str("<StorageDestination>");
    fields.push_str(&field("tt:StorageToken", &request.storage_token));
    if let Some(path) = &request.relative_directory {
        if path.len() > 1024
            || path.split('/').any(|part| {
                part.trim().is_empty()
                    || matches!(part.trim(), "." | "..")
                    || part.contains(['\\', ':', '%'])
                    || part.chars().any(char::is_control)
            })
        {
            return Err(OnvifError::InvalidArgument(
                "invalid relative export directory".into(),
            ));
        }
        fields.push_str(&field("tt:RelativePath", path));
    }
    fields.push_str("</StorageDestination>");
    Ok(fields)
}

fn file_names(response: &QualifiedElement) -> Result<Vec<String>, SoapError> {
    if response.serialized_len() > EXTENSION_BYTES {
        return Err(invalid("export file list exceeds retention limit"));
    }
    let mut files = Vec::new();
    for file in response
        .children()
        .filter(|node| node.namespace() == RECORDING && node.local_name() == "FileNames")
    {
        if files.len() == MAX_FILES {
            return Err(invalid("too many export file names"));
        }
        files.push(file.scalar()?);
    }
    Ok(files)
}

fn parse_progress(response: &QualifiedElement) -> Result<ExportProgress, SoapError> {
    if response.serialized_len() > EXTENSION_BYTES {
        return Err(invalid("export progress exceeds retention limit"));
    }
    let progress = fraction(response, RECORDING)?;
    let status = required(response, RECORDING, "FileProgressStatus")?;
    let mut files = Vec::new();
    for file in status
        .children()
        .filter(|node| node.namespace() == SCHEMA && node.local_name() == "FileProgress")
    {
        if files.len() == MAX_FILES {
            return Err(invalid("too many export file entries"));
        }
        files.push(ExportFileProgress {
            file_name: text(file, SCHEMA, "FileName")?,
            progress: fraction(file, SCHEMA)?,
        });
    }
    Ok(ExportProgress { progress, files })
}

fn fraction(node: &QualifiedElement, namespace: &str) -> Result<f32, SoapError> {
    let value = scalar(node, namespace, "Progress")?;
    if !(0.0..=1.0).contains(&value) {
        return Err(invalid("invalid export progress fraction"));
    }
    Ok(value)
}
