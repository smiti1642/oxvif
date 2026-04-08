//! Typed response structs for all ONVIF operations.

mod audio;
mod capabilities;
mod device;
mod events;
mod imaging;
mod media;
mod osd;
mod ptz;
mod ptz_config;
mod recording;
mod video;

pub use audio::*;
pub use capabilities::*;
pub use device::*;
pub use events::*;
pub use imaging::*;
pub use media::*;
pub use osd::*;
pub use ptz::*;
pub use ptz_config::*;
pub use recording::*;
pub use video::*;

use std::borrow::Cow;

use crate::soap::XmlNode;

// Re-exported so that the `mod tests` submodule can reach them via `use super::*`.
#[cfg(test)]
pub use crate::error::OnvifError;
#[cfg(test)]
pub use crate::soap::SoapError;
#[cfg(test)]
pub(crate) use device::civil_to_unix;

// ── XML helpers ───────────────────────────────────────────────────────────────
// Shared by all submodules via `use super::{xml_bool, xml_u32, xml_str}`.

/// Parse a boolean child element. Returns `true` for `"true"` or `"1"`.
pub(crate) fn xml_bool(node: &XmlNode, child: &str) -> bool {
    node.child(child)
        .is_some_and(|n| n.text() == "true" || n.text() == "1")
}

/// Parse an optional `u32` child element.
pub(crate) fn xml_u32(node: &XmlNode, child: &str) -> Option<u32> {
    node.child(child).and_then(|n| n.text().parse().ok())
}

/// Extract the text of a child element as an owned `String`.
pub(crate) fn xml_str(node: &XmlNode, child: &str) -> Option<String> {
    node.child(child).map(|n| n.text().to_string())
}

/// Escape XML special characters for safe embedding in element content or attributes.
///
/// Returns a [`Cow::Borrowed`] reference when no escaping is needed, avoiding
/// allocation in the common case (tokens, numeric strings, ISO durations, etc.).
pub(crate) fn xml_escape(s: &str) -> Cow<'_, str> {
    if s.bytes()
        .any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\''))
    {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&apos;"),
                _ => out.push(c),
            }
        }
        Cow::Owned(out)
    } else {
        Cow::Borrowed(s)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "../tests/types_tests.rs"]
mod tests;
