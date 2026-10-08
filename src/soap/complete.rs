//! Complete, bounded input checks before constructing the compatibility DOM.

use quick_xml::{Reader, XmlVersion, events::Event};
use std::collections::HashSet;

use super::error::{SoapError, XmlResource};

/// Maximum UTF-8 production SOAP document bytes, including markup and attributes.
pub const MAX_XML_BYTES: usize = 4 * 1024 * 1024;
/// Maximum production element depth, counting the root as one.
pub const MAX_XML_DEPTH: usize = 64;
/// Maximum production element count, including self-closing elements.
pub const MAX_XML_NODES: usize = 65_536;

fn syntax(message: &str) -> SoapError {
    SoapError::XmlParse(message.into())
}

fn limit(resource: XmlResource, value: usize) -> SoapError {
    SoapError::ResourceLimit {
        resource,
        limit: value,
    }
}

fn xml_char(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn name_start(ch: char) -> bool {
    matches!(ch, ':' | '_' | 'A'..='Z' | 'a'..='z' | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}' | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}' | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}' | '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}' | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
}

fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters.next().is_some_and(name_start)
        && characters.all(|ch| name_start(ch) || matches!(ch, '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'))
}

pub(super) fn validate(xml: &str) -> Result<(), SoapError> {
    if xml.len() > MAX_XML_BYTES {
        return Err(limit(XmlResource::Bytes, MAX_XML_BYTES));
    }
    if !xml.chars().all(xml_char) {
        return Err(syntax("invalid XML character"));
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().check_comments = true;
    let mut depth = 0;
    let mut nodes = 0;
    let mut root_seen = false;
    let mut declaration_seen = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| SoapError::XmlParse(error.to_string()))?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                if !valid_name(element.name().as_ref()) {
                    return Err(syntax("invalid XML element name"));
                }
                if depth == 0 {
                    if root_seen {
                        return Err(syntax("multiple XML roots"));
                    }
                    root_seen = true;
                }
                if depth == MAX_XML_DEPTH {
                    return Err(limit(XmlResource::Depth, MAX_XML_DEPTH));
                }
                if nodes == MAX_XML_NODES {
                    return Err(limit(XmlResource::Nodes, MAX_XML_NODES));
                }
                nodes += 1;
                // quick-xml's duplicate check compares each key with all prior
                // keys. Use a set so a wide, still byte-bounded element cannot
                // turn either this pass or DOM construction into quadratic work.
                let mut attributes = HashSet::new();
                for attr in element.attributes().with_checks(false) {
                    let attr = attr.map_err(|_| syntax("invalid XML attribute"))?;
                    if !valid_name(attr.key.as_ref()) || !attributes.insert(attr.key.into_inner()) {
                        return Err(syntax("invalid XML attribute"));
                    }
                    if attr.value.contains('<') {
                        return Err(syntax("invalid XML attribute"));
                    }
                    let value = attr
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|_| syntax("invalid XML attribute"))?;
                    if !value.chars().all(xml_char) {
                        return Err(syntax("invalid XML attribute"));
                    }
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| syntax("unmatched XML end"))?;
            }
            Event::Text(text) => {
                if depth == 0
                    && !text
                        .as_ref()
                        .bytes()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
                {
                    return Err(syntax("content outside XML root"));
                }
                if text.as_ref().contains("]]>") {
                    return Err(syntax("invalid XML text"));
                }
            }
            Event::CData(_) if depth == 0 => return Err(syntax("CDATA outside XML root")),
            Event::GeneralRef(reference) => {
                if depth == 0 {
                    return Err(syntax("reference outside XML root"));
                }
                match reference.resolve_char_ref() {
                    Ok(Some(ch)) if xml_char(ch) => {}
                    Ok(None)
                        if matches!(reference.as_ref(), "amp" | "lt" | "gt" | "quot" | "apos") => {}
                    _ => return Err(syntax("invalid XML reference")),
                }
            }
            Event::DocType(_) => return Err(syntax("DTD is not allowed in SOAP XML")),
            Event::Decl(declaration) => {
                if root_seen || depth != 0 || declaration_seen {
                    return Err(syntax("misplaced XML declaration"));
                }
                declaration_seen = true;
                let attributes =
                    quick_xml::events::BytesStart::from_content(declaration.as_ref(), 3);
                let mut previous = 0;
                for attr in attributes.attributes().with_checks(false) {
                    let attr = attr.map_err(|_| syntax("invalid XML declaration"))?;
                    let order = match attr.key.as_ref() {
                        "version" => 1,
                        "encoding" => 2,
                        "standalone" if matches!(attr.value.as_ref(), "yes" | "no") => 3,
                        _ => return Err(syntax("invalid XML declaration")),
                    };
                    if order <= previous {
                        return Err(syntax("invalid XML declaration"));
                    }
                    previous = order;
                }
                if declaration
                    .version()
                    .map_err(|_| syntax("invalid XML declaration"))?
                    .as_ref()
                    != "1.0"
                {
                    return Err(syntax("unsupported XML version"));
                }
                if let Some(encoding) = declaration.encoding() {
                    let encoding = encoding.map_err(|_| syntax("invalid XML declaration"))?;
                    if !encoding.eq_ignore_ascii_case("UTF-8") {
                        return Err(syntax("unsupported XML encoding declaration"));
                    }
                }
            }
            Event::Eof => {
                if depth != 0 {
                    return Err(syntax("incomplete XML document"));
                }
                if !root_seen {
                    return Err(syntax("empty XML document"));
                }
                return Ok(());
            }
            _ => {}
        }
    }
}
