use std::{collections::BTreeMap, fmt::Write, sync::Arc};

use quick_xml::{XmlVersion, events::Event, name::ResolveResult, reader::NsReader};

use super::SoapError;
use crate::{OnvifError, types::xml_escape};

pub(crate) const SOAP: &str = "http://www.w3.org/2003/05/soap-envelope";
pub(crate) const ADDRESSING: &str = "http://www.w3.org/2005/08/addressing";
pub(crate) const RESOURCE_UNKNOWN: &str =
    "{http://docs.oasis-open.org/wsrf/r-2}ResourceUnknownFault";
pub(crate) const EXTENSION_BYTES: usize = 256 * 1024;
const MAX_SCOPE_BYTES: usize = 4 * 1024 * 1024;
const MAX_BINDINGS: usize = 128;

type Scope = BTreeMap<String, String>;

/// Expanded XML attribute with its original lexical prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QualifiedAttribute {
    name: String,
    namespace: String,
    local: String,
    value: String,
}

impl QualifiedAttribute {
    /// Attribute namespace URI, empty for unqualified attributes.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    /// Local attribute name.
    pub fn local_name(&self) -> &str {
        &self.local
    }
    /// Normalized XML attribute value.
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Text and child elements in document order.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum QualifiedContent {
    /// Decoded XML text, including character references and CDATA.
    Text(String),
    /// Qualified child element.
    Element(QualifiedElement),
}

/// A bounded XML element with resolved names and in-scope namespace bindings.
/// Values may contain private device data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QualifiedElement {
    name: String,
    namespace: String,
    local: String,
    scope: Arc<Scope>,
    attributes: Vec<QualifiedAttribute>,
    content: Vec<QualifiedContent>,
}

impl QualifiedElement {
    /// Element namespace URI, empty if unqualified.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    /// Local element name.
    pub fn local_name(&self) -> &str {
        &self.local
    }
    /// Ordered, expanded attributes.
    pub fn attributes(&self) -> &[QualifiedAttribute] {
        &self.attributes
    }
    /// Ordered mixed content.
    pub fn content(&self) -> &[QualifiedContent] {
        &self.content
    }
    /// Namespace bound to a lexical prefix, including the empty default prefix.
    pub fn namespace_for_prefix(&self, prefix: &str) -> Option<&str> {
        if prefix == "xml" {
            Some("http://www.w3.org/XML/1998/namespace")
        } else {
            self.scope.get(prefix).map(String::as_str)
        }
    }

    pub(crate) fn children(&self) -> impl Iterator<Item = &Self> {
        self.content.iter().filter_map(|item| match item {
            QualifiedContent::Element(element) => Some(element),
            QualifiedContent::Text(_) => None,
        })
    }
    pub(crate) fn one(&self, ns: &str, name: &'static str) -> Result<Option<&Self>, SoapError> {
        let mut candidates = self
            .children()
            .filter(|e| e.namespace == ns && e.local == name);
        let first = candidates.next();
        if candidates.next().is_some() {
            return Err(invalid("duplicate qualified XML field"));
        }
        Ok(first)
    }
    pub(crate) fn attr(&self, ns: &str, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.namespace == ns && a.local == name)
            .map(|a| a.value.as_str())
    }
    pub(crate) fn scalar(&self) -> Result<String, SoapError> {
        if self.children().next().is_some() {
            return Err(invalid("expected XML scalar"));
        }
        Ok(self
            .content
            .iter()
            .filter_map(|item| match item {
                QualifiedContent::Text(text) => Some(text.as_str()),
                QualifiedContent::Element(_) => None,
            })
            .collect())
    }

    pub(crate) fn legacy_projection(&self, namespaces: &[&str]) -> super::XmlNode {
        let text: String = self
            .content
            .iter()
            .filter_map(|item| match item {
                QualifiedContent::Text(text) => Some(text.as_str()),
                QualifiedContent::Element(_) => None,
            })
            .collect();
        let text = text.trim();
        super::XmlNode {
            local_name: self.local.clone(),
            text: (!text.is_empty()).then(|| text.to_owned()),
            attrs: self
                .attributes
                .iter()
                .filter(|attr| attr.namespace.is_empty())
                .map(|attr| (attr.local.clone(), attr.value.clone()))
                .collect(),
            children: self
                .children()
                .filter(|child| namespaces.contains(&child.namespace()))
                .map(|child| child.legacy_projection(namespaces))
                .collect(),
        }
    }

    pub(crate) fn write_reference(&self, out: &mut String) {
        self.write_xml(out, None, true);
    }

    pub(crate) fn serialized_len(&self) -> usize {
        let mut out = String::new();
        self.write_xml(&mut out, None, false);
        out.len()
    }

    fn write_xml(&self, out: &mut String, parent: Option<&Scope>, reference: bool) {
        write!(out, "<{}", self.name).unwrap();
        for (prefix, uri) in self.scope.iter() {
            if parent.and_then(|p| p.get(prefix)) == Some(uri) {
                continue;
            }
            if prefix.is_empty() {
                write!(out, " xmlns=\"{}\"", attribute_value(uri)).unwrap();
            } else {
                write!(out, " xmlns:{prefix}=\"{}\"", attribute_value(uri)).unwrap();
            }
        }
        if reference {
            let prefix = (0..=MAX_BINDINGS)
                .map(|i| format!("refp{i}"))
                .find(|p| !self.scope.contains_key(p))
                .expect("bounded namespace scope");
            write!(
                out,
                " xmlns:{prefix}=\"{ADDRESSING}\" {prefix}:IsReferenceParameter=\"true\""
            )
            .unwrap();
        }
        for attr in &self.attributes {
            if reference && attr.namespace == ADDRESSING && attr.local == "IsReferenceParameter" {
                continue;
            }
            write!(out, " {}=\"{}\"", attr.name, attribute_value(&attr.value)).unwrap();
        }
        out.push('>');
        for item in &self.content {
            match item {
                QualifiedContent::Element(child) => child.write_xml(out, Some(&self.scope), false),
                QualifiedContent::Text(text) => {
                    out.push_str(&xml_escape(text).replace('\r', "&#13;"));
                }
            }
        }
        write!(out, "</{}>", self.name).unwrap();
    }
}

fn attribute_value(value: &str) -> String {
    xml_escape(value)
        .replace('\t', "&#9;")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
}

pub(crate) fn invalid(message: &'static str) -> SoapError {
    SoapError::XmlParse(message.into())
}

fn namespace(value: ResolveResult<'_>) -> Result<String, SoapError> {
    match value {
        ResolveResult::Unbound => Ok(String::new()),
        ResolveResult::Unknown(_) => Err(invalid("unbound XML prefix")),
        ResolveResult::Bound(value) => {
            use quick_xml::{events::attributes::Attribute, name::QName};
            let attr = Attribute {
                key: QName("xmlns"),
                value: value.as_ref().into(),
            };
            attr.normalized_value(XmlVersion::Implicit1_0)
                .map(|v| v.into_owned())
                .map_err(|_| invalid("invalid XML namespace"))
        }
    }
}

pub(crate) fn valid_qname(name: &str) -> bool {
    let ncname = |part: &str| super::complete::valid_name(part) && !part.contains(':');
    match name.split_once(':') {
        Some((prefix, local)) => ncname(prefix) && ncname(local),
        None => ncname(name),
    }
}

fn push_text(node: Option<&mut QualifiedElement>, value: &str) {
    if let Some(node) = node {
        if let Some(QualifiedContent::Text(last)) = node.content.last_mut() {
            last.push_str(value);
        } else {
            node.content.push(QualifiedContent::Text(value.to_owned()));
        }
    }
}

pub(crate) fn parse(xml: &str) -> Result<QualifiedElement, SoapError> {
    super::complete::validate(xml)?;
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<QualifiedElement> = Vec::new();
    let mut root = None;
    let empty_scope = Arc::new(Scope::new());
    let mut expanded_bytes = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|_| invalid("invalid qualified XML"))?;
        let finished = match event {
            Event::Start(ref start) | Event::Empty(ref start) => {
                let qname = start.name();
                if !valid_qname(qname.as_ref()) {
                    return Err(invalid("invalid qualified XML name"));
                }
                let (ns, local) = reader.resolver().resolve_element(qname);
                let ns = namespace(ns)?;
                expanded_bytes += ns.len();
                let mut node = QualifiedElement {
                    name: qname.as_ref().to_owned(),
                    namespace: ns,
                    local: local.as_ref().to_owned(),
                    scope: stack.last().map_or(&empty_scope, |n| &n.scope).clone(),
                    attributes: Vec::new(),
                    content: Vec::new(),
                };
                let mut scope = None;
                let mut attributes = std::collections::HashSet::new();
                for attr in start.attributes().with_checks(false) {
                    let attr = attr.map_err(|_| invalid("invalid XML attribute"))?;
                    let value = attr
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|_| invalid("invalid XML attribute value"))?
                        .into_owned();
                    let name = attr.key.as_ref();
                    if !valid_qname(name) {
                        return Err(invalid("invalid qualified XML attribute name"));
                    }
                    if let Some(prefix) = name
                        .strip_prefix("xmlns:")
                        .or_else(|| (name == "xmlns").then_some(""))
                    {
                        if value.len() > 1024 {
                            return Err(invalid("XML namespace URI exceeds limit"));
                        }
                        scope
                            .get_or_insert_with(|| (*node.scope).clone())
                            .insert(prefix.to_owned(), value);
                    } else {
                        let (ns, local) = reader.resolver().resolve_attribute(attr.key);
                        let ns = namespace(ns)?;
                        expanded_bytes += ns.len();
                        if !attributes.insert((ns.clone(), local.as_ref().to_owned())) {
                            return Err(invalid("duplicate expanded XML attribute"));
                        }
                        node.attributes.push(QualifiedAttribute {
                            name: name.into(),
                            namespace: ns,
                            local: local.as_ref().into(),
                            value,
                        });
                    }
                    if expanded_bytes > MAX_SCOPE_BYTES {
                        return Err(invalid("expanded XML names exceed limit"));
                    }
                }
                if let Some(scope) = scope {
                    if scope.len() > MAX_BINDINGS {
                        return Err(invalid("XML namespace count exceeds limit"));
                    }
                    expanded_bytes += scope
                        .iter()
                        .map(|(prefix, uri)| prefix.len() + uri.len())
                        .sum::<usize>();
                    node.scope = Arc::new(scope);
                }
                if expanded_bytes > MAX_SCOPE_BYTES {
                    return Err(invalid("expanded XML names exceed limit"));
                }
                if matches!(event, Event::Empty(_)) {
                    Some(node)
                } else {
                    stack.push(node);
                    None
                }
            }
            Event::End(_) => stack.pop(),
            Event::Text(text) => {
                push_text(stack.last_mut(), &text.xml10_content());
                None
            }
            Event::CData(text) => {
                push_text(stack.last_mut(), &text.xml10_content());
                None
            }
            Event::GeneralRef(reference) => {
                let ch = match reference.as_ref() {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    _ => reference
                        .resolve_char_ref()
                        .ok()
                        .flatten()
                        .ok_or_else(|| invalid("invalid XML reference"))?,
                };
                push_text(stack.last_mut(), ch.encode_utf8(&mut [0; 4]));
                None
            }
            Event::Eof => break,
            _ => None,
        };
        if let Some(node) = finished {
            if let Some(parent) = stack.last_mut() {
                parent.content.push(QualifiedContent::Element(node));
            } else {
                root = Some(node);
            }
        }
    }
    root.ok_or_else(|| invalid("missing XML root"))
}

pub(crate) fn response(
    xml: &str,
    ns: &str,
    name: &'static str,
) -> Result<QualifiedElement, OnvifError> {
    let root = parse(xml)?;
    if root.namespace() != SOAP || root.local_name() != "Envelope" {
        return Err(invalid("expected SOAP 1.2 Envelope").into());
    }
    let body = root.one(SOAP, "Body")?.ok_or(SoapError::MissingBody)?;
    if let Some(fault) = body.one(SOAP, "Fault")? {
        let code = fault.one(SOAP, "Code")?;
        let subcode = code.map(|c| c.one(SOAP, "Subcode")).transpose()?.flatten();
        let value = subcode.map(|s| s.one(SOAP, "Value")).transpose()?.flatten();
        let mut subcode = value
            .map(|v| {
                let text = v.scalar()?;
                let text = text.trim();
                if !valid_qname(text) {
                    return Err(invalid("invalid SOAP Fault subcode QName"));
                }
                let (prefix, local) = text.split_once(':').unwrap_or(("", text));
                Ok::<_, SoapError>(
                    v.namespace_for_prefix(prefix)
                        .map(|ns| format!("{{{ns}}}{local}"))
                        .unwrap_or_default(),
                )
            })
            .transpose()?
            .filter(|v| !v.is_empty());
        if subcode.is_none()
            && let Some(detail) = fault.one(SOAP, "Detail")?
            && detail
                .one(
                    "http://docs.oasis-open.org/wsrf/r-2",
                    "ResourceUnknownFault",
                )?
                .is_some()
        {
            subcode = Some(RESOURCE_UNKNOWN.into());
        }
        let reason = fault.one(SOAP, "Reason")?;
        return Err(SoapError::Fault {
            code: code
                .map(|c| scalar(c, SOAP, "Value"))
                .transpose()?
                .flatten()
                .unwrap_or_default(),
            reason: reason
                .map(|r| scalar(r, SOAP, "Text"))
                .transpose()?
                .flatten()
                .unwrap_or_default(),
            subcode,
            detail: None,
        }
        .into());
    }
    body.one(ns, name)?
        .cloned()
        .ok_or_else(|| SoapError::UnexpectedResponse(name.into()).into())
}

fn scalar(
    node: &QualifiedElement,
    ns: &str,
    name: &'static str,
) -> Result<Option<String>, SoapError> {
    node.one(ns, name)?
        .map(QualifiedElement::scalar)
        .transpose()
}
