//! Operation-scoped request parsing for migrated synthetic handlers.
//!
//! Unlike the compatibility client DOM, this keeps namespace scopes and text.
//! It is not an XSD validator. Legacy fragment extractors remain in use by
//! handlers that have not yet migrated; replay and authentication are unchanged.

use std::{borrow::Cow, collections::HashSet};

use quick_xml::{
    NsReader, XmlVersion,
    events::{BytesStart, Event, attributes::Attribute},
    name::{QName, ResolveResult},
};

const SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 16_384;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RequestError(pub &'static str);

#[derive(Debug)]
struct Node {
    ns: String,
    name: String,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn child(&self, ns: &str, name: &str) -> Result<Option<&Self>, RequestError> {
        let mut matches = self
            .children
            .iter()
            .filter(|n| n.ns == ns && n.name == name);
        let result = matches.next();
        if matches.next().is_some() {
            return Err(RequestError("duplicate request field"));
        }
        Ok(result)
    }
}

fn namespace(value: ResolveResult<'_>) -> Result<String, RequestError> {
    match value {
        // NsReader intentionally returns the raw xmlns attribute value.
        // Normalize before comparing namespace identities, not after decoding
        // again (a literal entity-looking substring must stay literal).
        ResolveResult::Bound(ns) => Attribute {
            key: QName("xmlns"),
            value: Cow::Borrowed(ns.as_ref()),
        }
        .normalized_value(XmlVersion::Implicit1_0)
        .map(|value| value.into_owned())
        .map_err(|_| RequestError("invalid namespace value")),
        ResolveResult::Unbound => Ok(String::new()),
        ResolveResult::Unknown(_) => Err(RequestError("unbound namespace prefix")),
    }
}

fn xml_text(value: &str) -> Result<(), RequestError> {
    if value.chars().all(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')) {
        Ok(())
    } else {
        Err(RequestError("invalid XML character"))
    }
}

fn start(reader: &NsReader<&[u8]>, event: &BytesStart<'_>) -> Result<Node, RequestError> {
    let (ns, local) = reader.resolver().resolve_element(event.name());
    let ns = namespace(ns)?;
    let mut seen = HashSet::new();
    for attribute in event.attributes() {
        let attribute = attribute.map_err(|_| RequestError("invalid or duplicate attribute"))?;
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| RequestError("invalid attribute value"))?;
        xml_text(&value)?;
        if attribute.key.as_ref() == "xmlns" || attribute.key.as_ref().starts_with("xmlns:") {
            continue;
        }
        let (ns, name) = reader.resolver().resolve_attribute(attribute.key);
        let key = (namespace(ns)?, name.as_ref().to_string());
        if !seen.insert(key) {
            return Err(RequestError("duplicate expanded attribute name"));
        }
    }
    Ok(Node {
        ns,
        name: local.as_ref().to_string(),
        text: String::new(),
        children: Vec::new(),
    })
}

fn append(stack: &mut [Node], value: &str) -> Result<(), RequestError> {
    xml_text(value)?;
    if let Some(node) = stack.last_mut() {
        node.text.push_str(value);
    } else if !value.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r')) {
        return Err(RequestError("text outside document element"));
    }
    Ok(())
}

fn place(stack: &mut [Node], root: &mut Option<Node>, node: Node) -> Result<(), RequestError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return Err(RequestError("multiple document elements"));
    }
    Ok(())
}

fn parse(xml: &str) -> Result<Node, RequestError> {
    if xml.len() > MAX_BYTES {
        return Err(RequestError("request byte limit exceeded"));
    }
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    reader.config_mut().check_comments = true;
    let mut stack = Vec::new();
    let mut root = None;
    let mut nodes = 0;
    let mut events = 0;
    loop {
        events += 1;
        match reader
            .read_event()
            .map_err(|_| RequestError("malformed XML"))?
        {
            Event::Start(event) => {
                nodes += 1;
                if nodes > MAX_NODES {
                    return Err(RequestError("request node limit exceeded"));
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(RequestError("request depth limit exceeded"));
                }
                stack.push(start(&reader, &event)?);
            }
            Event::Empty(event) => {
                nodes += 1;
                if nodes > MAX_NODES {
                    return Err(RequestError("request node limit exceeded"));
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(RequestError("request depth limit exceeded"));
                }
                let node = start(&reader, &event)?;
                place(&mut stack, &mut root, node)?;
            }
            Event::End(_) => {
                let node = stack.pop().ok_or(RequestError("unmatched end tag"))?;
                place(&mut stack, &mut root, node)?;
            }
            Event::Text(event) => append(&mut stack, &event.xml10_content())?,
            Event::CData(event) => {
                if stack.is_empty() {
                    return Err(RequestError("CDATA outside document element"));
                }
                append(&mut stack, &event.xml10_content())?;
            }
            Event::GeneralRef(event) => {
                if stack.is_empty() {
                    return Err(RequestError("entity outside document element"));
                }
                let decoded = match event.as_ref() {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    _ => event
                        .resolve_char_ref()
                        .map_err(|_| RequestError("invalid character reference"))?
                        .ok_or(RequestError("unknown entity reference"))?,
                };
                append(&mut stack, decoded.encode_utf8(&mut [0; 4]))?;
            }
            Event::DocType(_) => return Err(RequestError("DTD is not supported")),
            Event::Decl(event) => {
                if events != 1
                    || root.is_some()
                    || !stack.is_empty()
                    || !matches!(event.version().as_deref(), Ok("1.0"))
                {
                    return Err(RequestError("invalid XML declaration"));
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::PI(_) => {}
        }
    }
    if !stack.is_empty() {
        return Err(RequestError("unclosed document element"));
    }
    root.ok_or(RequestError("missing document element"))
}

/// Read one required scalar child from an identified operation. The caller
/// supplies its own service/operation contract; no schema catalogue is embedded.
pub(super) fn required_text(
    xml: &str,
    ns: &str,
    operation: &str,
    field: &str,
) -> Result<String, RequestError> {
    let root = parse(xml)?;
    let op = if root.ns == SOAP && root.name == "Envelope" {
        let body = root
            .child(SOAP, "Body")?
            .ok_or(RequestError("missing SOAP Body"))?;
        if body.children.len() != 1 {
            return Err(RequestError("expected one SOAP operation"));
        }
        &body.children[0]
    } else {
        &root
    };
    if op.ns != ns || op.name != operation {
        return Err(RequestError("unexpected operation or namespace"));
    }
    let value = op
        .child(ns, field)?
        .ok_or(RequestError("missing request field"))?;
    if !value.children.is_empty() {
        return Err(RequestError("expected scalar request field"));
    }
    if value.text.is_empty() {
        return Err(RequestError("empty request field"));
    }
    Ok(value.text.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(xml: &str) -> Result<String, RequestError> {
        required_text(xml, "urn:mock:test", "Command", "Key")
    }

    #[test]
    fn text_decodes_once_and_preserves_whitespace() {
        assert_eq!(read("<Command xmlns='urn:mock:test'><Key> 北&amp; &lt;&#x9580;<![CDATA[> ]]>&amp;amp; </Key></Command>").unwrap(), " 北& <門> &amp; ");
    }

    #[test]
    fn namespace_scope_is_not_a_global_prefix_map() {
        let xml = "<m:Command xmlns:m='urn:mock:test'><m:Key>right</m:Key><Extension xmlns:m='urn:other'><m:Key>wrong</m:Key></Extension></m:Command>";
        assert_eq!(read(xml).unwrap(), "right");
        assert_eq!(
            read("<Command xmlns='urn:mock:test'><Key xmlns='urn:other'>wrong</Key></Command>"),
            Err(RequestError("missing request field"))
        );
    }

    #[test]
    fn namespace_attribute_values_are_normalized_once() {
        assert_eq!(
            read("<Command xmlns='urn:mock:te&#x73;t'><Key>valid</Key></Command>").unwrap(),
            "valid"
        );
        assert_eq!(
            required_text(
                "<Command xmlns='urn:mock:&amp;amp;'><Key>literal</Key></Command>",
                "urn:mock:&amp;",
                "Command",
                "Key"
            )
            .unwrap(),
            "literal"
        );
    }

    #[test]
    fn misleading_nested_fields_are_not_selected() {
        assert_eq!(
            read(
                "<Command xmlns='urn:mock:test'><Extension><Key>wrong</Key></Extension></Command>"
            ),
            Err(RequestError("missing request field"))
        );
        assert_eq!(
            read("<Command xmlns='urn:mock:test'><Key>a</Key><Key>b</Key></Command>"),
            Err(RequestError("duplicate request field"))
        );
        assert_eq!(
            read("<Command xmlns='urn:mock:test'><Key><Nested>x</Nested></Key></Command>"),
            Err(RequestError("expected scalar request field"))
        );
    }

    #[test]
    fn soap_header_cannot_supply_the_operation_field() {
        let xml = format!(
            "<s:Envelope xmlns:s='{SOAP}' xmlns:m='urn:mock:test'><s:Header><m:Key>decoy</m:Key></s:Header><s:Body><m:Command><m:Key>actual</m:Key></m:Command></s:Body></s:Envelope>"
        );
        assert_eq!(read(&xml).unwrap(), "actual");
        assert_eq!(
            read(&xml.replace("<m:Key>actual</m:Key>", "")),
            Err(RequestError("missing request field"))
        );
        assert_eq!(
            read(&xml.replace("</s:Body>", "<m:Command/></s:Body>")),
            Err(RequestError("expected one SOAP operation"))
        );
    }

    #[test]
    fn malformed_input_is_rejected_without_echoing_payload() {
        for (xml, error) in [
            (
                "<Command xmlns='urn:mock:test'><Key>x</Key></Command><Extra/>",
                "multiple document elements",
            ),
            (
                "<Command xmlns='urn:mock:test'><Key>&unknown;</Key></Command>",
                "unknown entity reference",
            ),
            ("<!DOCTYPE Command><Command/>", "DTD is not supported"),
            ("<p:Command/>", "unbound namespace prefix"),
            ("<Command p:key='x'/>", "unbound namespace prefix"),
            (
                "<Command xmlns:a='urn:a' xmlns:b='urn:a' a:key='1' b:key='2'/>",
                "duplicate expanded attribute name",
            ),
            ("<Command><Key></Command>", "malformed XML"),
            (
                "<Command xmlns='urn:mock:test'><Key>\u{1}</Key></Command>",
                "invalid XML character",
            ),
            (
                "<?xml version='1.0'?><?xml version='1.0'?><Command/>",
                "invalid XML declaration",
            ),
        ] {
            assert_eq!(read(xml), Err(RequestError(error)), "case: {xml}");
        }
    }

    #[test]
    fn resource_limits_are_enforced() {
        assert_eq!(
            parse(&" ".repeat(MAX_BYTES + 1)).unwrap_err(),
            RequestError("request byte limit exceeded")
        );
        assert_eq!(
            parse(&format!(
                "{}{}",
                "<n>".repeat(MAX_DEPTH + 1),
                "</n>".repeat(MAX_DEPTH + 1)
            ))
            .unwrap_err(),
            RequestError("request depth limit exceeded")
        );
        assert_eq!(
            parse(&format!("<root>{}</root>", "<n/>".repeat(MAX_NODES))).unwrap_err(),
            RequestError("request node limit exceeded")
        );
    }
}
