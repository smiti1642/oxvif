//! SOAP protocol layer for ONVIF.
//!
//! This module handles everything between the raw HTTP string and the typed
//! ONVIF response structs:
//!
//! 1. **Envelope construction** ([`SoapEnvelope`]) — wraps a body fragment in
//!    a fully-namespaced `<s:Envelope>`, optionally with a WS-Security header.
//! 2. **WS-Security** ([`WsSecurityToken`]) — generates a `UsernameToken`
//!    with `PasswordDigest` (SHA-1, base64-encoded) as required by ONVIF
//!    Profile S §5.12.
//! 3. **XML parsing** ([`XmlNode`], [`parse_soap_body`], [`find_response`]) —
//!    a namespace-stripping DOM built with `quick-xml`.
//! 4. **Error types** ([`SoapError`]) — structured errors for parse failures,
//!    missing fields, and SOAP Faults.

mod complete;
pub mod envelope;
pub mod error;
pub(crate) mod qualified;
pub mod security;
pub mod xml;

#[cfg(feature = "mock-server")]
pub(crate) use complete::validate as validate_complete_xml;
pub use complete::{MAX_XML_BYTES, MAX_XML_DEPTH, MAX_XML_NODES};
pub use envelope::SoapEnvelope;
pub use error::{SoapError, XmlResource};
pub use security::WsSecurityToken;
pub use xml::{XmlNode, find_response, parse_soap_body};
