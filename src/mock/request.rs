//! Operation-scoped request parsing for migrated synthetic handlers.
//!
//! Unlike the compatibility client DOM, this keeps namespace scopes and text.
//! It is not an XSD validator. Legacy fragment extractors remain in use by
//! handlers that have not yet migrated; replay and authentication are unchanged.

use std::{borrow::Cow, collections::HashMap};

use quick_xml::{
    NsReader, XmlVersion,
    events::{BytesStart, Event, attributes::Attribute},
    name::{QName, ResolveResult},
};

const SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 16_384;

/// Typed private diagnostics: routing/fault policy must match variants, never
/// guess a category from a human-readable message. No request text is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RequestError {
    DuplicateField,
    ExpectedScalar,
    MissingField,
    EmptyField,
    MissingBody,
    OperationCount,
    OperationIdentity,
    InvalidNamespace,
    UnboundPrefix,
    InvalidCharacter,
    InvalidAttribute,
    InvalidAttributeValue,
    DuplicateExpandedAttribute,
    TextOutsideRoot,
    MultipleRoots,
    ByteLimit,
    MalformedXml,
    NodeLimit,
    DepthLimit,
    UnmatchedEnd,
    CdataOutsideRoot,
    EntityOutsideRoot,
    InvalidCharacterReference,
    UnknownEntity,
    DtdUnsupported,
    InvalidDeclaration,
    UnclosedRoot,
    MissingRoot,
}

impl RequestError {
    pub(super) const fn message(self) -> &'static str {
        match self {
            Self::DuplicateField => "duplicate request field",
            Self::ExpectedScalar => "expected scalar request field",
            Self::MissingField => "missing request field",
            Self::EmptyField => "empty request field",
            Self::MissingBody => "missing SOAP Body",
            Self::OperationCount => "expected one SOAP operation",
            Self::OperationIdentity => "unexpected operation or namespace",
            Self::InvalidNamespace => "invalid namespace value",
            Self::UnboundPrefix => "unbound namespace prefix",
            Self::InvalidCharacter => "invalid XML character",
            Self::InvalidAttribute => "invalid or duplicate attribute",
            Self::InvalidAttributeValue => "invalid attribute value",
            Self::DuplicateExpandedAttribute => "duplicate expanded attribute name",
            Self::TextOutsideRoot => "text outside document element",
            Self::MultipleRoots => "multiple document elements",
            Self::ByteLimit => "request byte limit exceeded",
            Self::MalformedXml => "malformed XML",
            Self::NodeLimit => "request node limit exceeded",
            Self::DepthLimit => "request depth limit exceeded",
            Self::UnmatchedEnd => "unmatched end tag",
            Self::CdataOutsideRoot => "CDATA outside document element",
            Self::EntityOutsideRoot => "entity outside document element",
            Self::InvalidCharacterReference => "invalid character reference",
            Self::UnknownEntity => "unknown entity reference",
            Self::DtdUnsupported => "DTD is not supported",
            Self::InvalidDeclaration => "invalid XML declaration",
            Self::UnclosedRoot => "unclosed document element",
            Self::MissingRoot => "missing document element",
        }
    }
}

#[derive(Debug)]
pub(super) struct Node {
    ns: String,
    name: String,
    text: String,
    attributes: HashMap<(String, String), String>,
    children: Vec<Node>,
}

impl Node {
    /// Direct children only, in document order; extension subtrees retain scope.
    pub(super) fn children_named<'a, 'q>(
        &'a self,
        ns: &'q str,
        name: &'q str,
    ) -> impl Iterator<Item = &'a Self> + 'q
    where
        'a: 'q,
    {
        self.children
            .iter()
            .filter(move |n| n.ns == ns && n.name == name)
    }

    pub(super) fn child<'a>(
        &'a self,
        ns: &str,
        name: &str,
    ) -> Result<Option<&'a Self>, RequestError> {
        let mut matches = self.children_named(ns, name);
        let result = matches.next();
        if matches.next().is_some() {
            return Err(RequestError::DuplicateField);
        }
        Ok(result)
    }

    /// Decoded scalar text; an empty element is distinct from an absent child.
    pub(super) fn scalar_text(&self) -> Result<&str, RequestError> {
        if !self.children.is_empty() {
            return Err(RequestError::ExpectedScalar);
        }
        Ok(&self.text)
    }

    pub(super) fn required_child_text(&self, ns: &str, name: &str) -> Result<&str, RequestError> {
        let value = self
            .child(ns, name)?
            .ok_or(RequestError::MissingField)?
            .scalar_text()?;
        if value.is_empty() {
            return Err(RequestError::EmptyField);
        }
        Ok(value)
    }

    /// Attributes use expanded names. The default namespace does not qualify
    /// an unprefixed attribute. Values are XML-normalized and decoded once.
    // Staged W04 API: retained for attribute/subtree handler migration; tested
    // here without broadening the existing DeleteProfile scalar contract.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn attribute(&self, ns: &str, name: &str) -> Option<&str> {
        self.attributes.iter().find_map(|((uri, local), value)| {
            (uri == ns && local == name).then_some(value.as_str())
        })
    }
}

/// Own one decoded tree; borrowed views never reconstruct XML fragments.
#[derive(Debug)]
pub(super) struct Request {
    root: Node,
}

impl Request {
    pub(super) fn parse(xml: &str) -> Result<Self, RequestError> {
        Ok(Self { root: parse(xml)? })
    }

    pub(super) fn operation(&self, ns: &str, name: &str) -> Result<&Node, RequestError> {
        let op = if self.root.ns == SOAP && self.root.name == "Envelope" {
            let body = self
                .root
                .child(SOAP, "Body")?
                .ok_or(RequestError::MissingBody)?;
            if body.children.len() != 1 {
                return Err(RequestError::OperationCount);
            }
            &body.children[0]
        } else {
            &self.root
        };
        if op.ns != ns || op.name != name {
            return Err(RequestError::OperationIdentity);
        }
        Ok(op)
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
        .map_err(|_| RequestError::InvalidNamespace),
        ResolveResult::Unbound => Ok(String::new()),
        ResolveResult::Unknown(_) => Err(RequestError::UnboundPrefix),
    }
}

fn xml_text(value: &str) -> Result<(), RequestError> {
    if value.chars().all(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')) {
        Ok(())
    } else {
        Err(RequestError::InvalidCharacter)
    }
}

fn start(reader: &NsReader<&[u8]>, event: &BytesStart<'_>) -> Result<Node, RequestError> {
    let (ns, local) = reader.resolver().resolve_element(event.name());
    let ns = namespace(ns)?;
    let mut attributes = HashMap::new();
    for attribute in event.attributes() {
        let attribute = attribute.map_err(|_| RequestError::InvalidAttribute)?;
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|_| RequestError::InvalidAttributeValue)?;
        xml_text(&value)?;
        if attribute.key.as_ref() == "xmlns" || attribute.key.as_ref().starts_with("xmlns:") {
            continue;
        }
        let (ns, name) = reader.resolver().resolve_attribute(attribute.key);
        let key = (namespace(ns)?, name.as_ref().to_string());
        if attributes.insert(key, value.into_owned()).is_some() {
            return Err(RequestError::DuplicateExpandedAttribute);
        }
    }
    Ok(Node {
        ns,
        name: local.as_ref().to_string(),
        text: String::new(),
        attributes,
        children: Vec::new(),
    })
}

fn append(stack: &mut [Node], value: &str) -> Result<(), RequestError> {
    xml_text(value)?;
    if let Some(node) = stack.last_mut() {
        node.text.push_str(value);
    } else if !value.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r')) {
        return Err(RequestError::TextOutsideRoot);
    }
    Ok(())
}

fn place(stack: &mut [Node], root: &mut Option<Node>, node: Node) -> Result<(), RequestError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return Err(RequestError::MultipleRoots);
    }
    Ok(())
}

fn parse(xml: &str) -> Result<Node, RequestError> {
    if xml.len() > MAX_BYTES {
        return Err(RequestError::ByteLimit);
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
            .map_err(|_| RequestError::MalformedXml)?
        {
            Event::Start(event) => {
                nodes += 1;
                if nodes > MAX_NODES {
                    return Err(RequestError::NodeLimit);
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(RequestError::DepthLimit);
                }
                stack.push(start(&reader, &event)?);
            }
            Event::Empty(event) => {
                nodes += 1;
                if nodes > MAX_NODES {
                    return Err(RequestError::NodeLimit);
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(RequestError::DepthLimit);
                }
                let node = start(&reader, &event)?;
                place(&mut stack, &mut root, node)?;
            }
            Event::End(_) => {
                let node = stack.pop().ok_or(RequestError::UnmatchedEnd)?;
                place(&mut stack, &mut root, node)?;
            }
            Event::Text(event) => append(&mut stack, &event.xml10_content())?,
            Event::CData(event) => {
                if stack.is_empty() {
                    return Err(RequestError::CdataOutsideRoot);
                }
                append(&mut stack, &event.xml10_content())?;
            }
            Event::GeneralRef(event) => {
                if stack.is_empty() {
                    return Err(RequestError::EntityOutsideRoot);
                }
                let decoded = match event.as_ref() {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    _ => event
                        .resolve_char_ref()
                        .map_err(|_| RequestError::InvalidCharacterReference)?
                        .ok_or(RequestError::UnknownEntity)?,
                };
                append(&mut stack, decoded.encode_utf8(&mut [0; 4]))?;
            }
            Event::DocType(_) => return Err(RequestError::DtdUnsupported),
            Event::Decl(event) => {
                if events != 1
                    || root.is_some()
                    || !stack.is_empty()
                    || !matches!(event.version().as_deref(), Ok("1.0"))
                {
                    return Err(RequestError::InvalidDeclaration);
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::PI(_) => {}
        }
    }
    if !stack.is_empty() {
        return Err(RequestError::UnclosedRoot);
    }
    root.ok_or(RequestError::MissingRoot)
}

/// Read one required scalar child from an identified operation. The caller
/// supplies its own service/operation contract; no schema catalogue is embedded.
pub(super) fn required_text(
    xml: &str,
    ns: &str,
    operation: &str,
    field: &str,
) -> Result<String, RequestError> {
    let request = Request::parse(xml)?;
    Ok(request
        .operation(ns, operation)?
        .required_child_text(ns, field)?
        .to_owned())
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
    fn attributes_keep_expanded_names_and_do_not_inherit_default_namespace() {
        let request = Request::parse(
            "<Command xmlns='urn:mock:test' xmlns:a='urn:attrs' key='bare' a:key='qualified'/>",
        )
        .unwrap();
        let op = request.operation("urn:mock:test", "Command").unwrap();
        assert_eq!(op.attribute("", "key"), Some("bare"));
        assert_eq!(op.attribute("urn:attrs", "key"), Some("qualified"));
        assert_eq!(op.attribute("urn:mock:test", "key"), None);
        assert_eq!(op.attribute("", "xmlns"), None);
        assert_eq!(op.attribute("http://www.w3.org/2000/xmlns/", "a"), None);
    }

    #[test]
    fn attributes_normalize_literal_whitespace_but_preserve_character_references() {
        let request = Request::parse(
            "<Command xmlns='urn:mock:test' key='A\r\nB\tC&#x9;&#xD;&#xA;&amp;amp;'/>",
        )
        .unwrap();
        assert_eq!(
            request
                .operation("urn:mock:test", "Command")
                .unwrap()
                .attribute("", "key"),
            Some("A B C\t\r\n&amp;")
        );
    }

    #[test]
    fn repeated_children_preserve_order_subtrees_and_namespace_scope() {
        let request = Request::parse(
            "<Command xmlns='urn:mock:test' xmlns:a='urn:outer'>\
             <Entry a:key='first'><Key>one&amp;two</Key></Entry>\
             <Extension xmlns:a='urn:inner'><Entry a:key='nested'/></Extension>\
             <Entry xmlns='urn:other' a:key='foreign'/>\
             <Entry a:key='last'><Key>three</Key></Entry></Command>",
        )
        .unwrap();
        let op = request.operation("urn:mock:test", "Command").unwrap();
        let entries: Vec<_> = op.children_named("urn:mock:test", "Entry").collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].attribute("urn:outer", "key"), Some("first"));
        assert_eq!(entries[1].attribute("urn:outer", "key"), Some("last"));
        assert_eq!(
            entries[0]
                .required_child_text("urn:mock:test", "Key")
                .unwrap(),
            "one&two"
        );
        assert_eq!(
            entries[1]
                .required_child_text("urn:mock:test", "Key")
                .unwrap(),
            "three"
        );
        assert_eq!(
            op.child("urn:mock:test", "Entry").unwrap_err(),
            RequestError::DuplicateField
        );
        let nested = op
            .child("urn:mock:test", "Extension")
            .unwrap()
            .unwrap()
            .child("urn:mock:test", "Entry")
            .unwrap()
            .unwrap();
        assert_eq!(nested.attribute("urn:inner", "key"), Some("nested"));
        assert_eq!(nested.attribute("urn:outer", "key"), None);
    }

    #[test]
    fn absent_empty_scalar_and_subtree_are_distinct() {
        let request = Request::parse(
            "<Command xmlns='urn:mock:test'><Empty/><Tree><Key>nested</Key></Tree></Command>",
        )
        .unwrap();
        let op = request.operation("urn:mock:test", "Command").unwrap();
        assert!(op.child("urn:mock:test", "Absent").unwrap().is_none());
        assert_eq!(
            op.child("urn:mock:test", "Empty")
                .unwrap()
                .unwrap()
                .scalar_text(),
            Ok("")
        );
        assert_eq!(
            op.required_child_text("urn:mock:test", "Empty"),
            Err(RequestError::EmptyField)
        );
        assert_eq!(
            op.required_child_text("urn:mock:test", "Absent"),
            Err(RequestError::MissingField)
        );
        assert_eq!(
            op.child("urn:mock:test", "Tree")
                .unwrap()
                .unwrap()
                .scalar_text(),
            Err(RequestError::ExpectedScalar)
        );
    }

    #[test]
    fn decoded_tree_is_reusable_without_reparsing_or_renormalizing_text() {
        let request = Request::parse(
            "<Command xmlns='urn:mock:test'><Key>A\r\nB\rC&#xD;D<![CDATA[\r\nE]]></Key></Command>",
        )
        .unwrap();
        let first = request.operation("urn:mock:test", "Command").unwrap();
        let second = request.operation("urn:mock:test", "Command").unwrap();
        assert!(std::ptr::eq(first, second));
        assert_eq!(
            first.required_child_text("urn:mock:test", "Key").unwrap(),
            "A\nB\nC\rD\nE"
        );
        assert_eq!(
            second.required_child_text("urn:mock:test", "Key").unwrap(),
            "A\nB\nC\rD\nE"
        );
        assert_eq!(
            request.operation("urn:other", "Command").unwrap_err(),
            RequestError::OperationIdentity
        );
    }

    #[test]
    fn normalized_namespace_aliases_cannot_duplicate_an_attribute() {
        assert_eq!(
            Request::parse("<Command xmlns:a='urn:a' xmlns:b='urn:&#97;' a:key='1' b:key='2'/>")
                .unwrap_err(),
            RequestError::DuplicateExpandedAttribute
        );
    }

    #[test]
    fn resource_limits_accept_the_boundary_not_only_reject_large_inputs() {
        let xml = format!("{}{}", "<n>".repeat(MAX_DEPTH), "</n>".repeat(MAX_DEPTH));
        assert_eq!(
            Request::parse(&xml)
                .unwrap()
                .operation("", "n")
                .unwrap()
                .name,
            "n"
        );
        let xml = format!("<root>{}</root>", "<n/>".repeat(MAX_NODES - 1));
        let request = Request::parse(&xml).unwrap();
        assert_eq!(
            request
                .operation("", "root")
                .unwrap()
                .children_named("", "n")
                .count(),
            MAX_NODES - 1
        );
    }

    #[test]
    fn namespace_scope_is_not_a_global_prefix_map() {
        let xml = "<m:Command xmlns:m='urn:mock:test'><m:Key>right</m:Key><Extension xmlns:m='urn:other'><m:Key>wrong</m:Key></Extension></m:Command>";
        assert_eq!(read(xml).unwrap(), "right");
        assert_eq!(
            read("<Command xmlns='urn:mock:test'><Key xmlns='urn:other'>wrong</Key></Command>"),
            Err(RequestError::MissingField)
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
            Err(RequestError::MissingField)
        );
        assert_eq!(
            read("<Command xmlns='urn:mock:test'><Key>a</Key><Key>b</Key></Command>"),
            Err(RequestError::DuplicateField)
        );
        assert_eq!(
            read("<Command xmlns='urn:mock:test'><Key><Nested>x</Nested></Key></Command>"),
            Err(RequestError::ExpectedScalar)
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
            Err(RequestError::MissingField)
        );
        assert_eq!(
            read(&xml.replace("</s:Body>", "<m:Command/></s:Body>")),
            Err(RequestError::OperationCount)
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
            assert_eq!(
                read(xml).map_err(RequestError::message),
                Err(error),
                "case: {xml}"
            );
        }
    }

    #[test]
    fn resource_limits_are_enforced() {
        assert_eq!(
            parse(&" ".repeat(MAX_BYTES + 1)).unwrap_err(),
            RequestError::ByteLimit
        );
        assert_eq!(
            parse(&format!(
                "{}{}",
                "<n>".repeat(MAX_DEPTH + 1),
                "</n>".repeat(MAX_DEPTH + 1)
            ))
            .unwrap_err(),
            RequestError::DepthLimit
        );
        assert_eq!(
            parse(&format!("<root>{}</root>", "<n/>".repeat(MAX_NODES))).unwrap_err(),
            RequestError::NodeLimit
        );
    }
}
