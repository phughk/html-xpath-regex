use serde::Serialize;

/// Unified DOM node abstraction used by both HTML and XML parsers.
#[derive(Debug, Clone)]
pub struct SimpleNode {
    pub kind: NodeKind,
    pub children: Vec<SimpleNode>,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Document,
    Element {
        local_name: String,
        attributes: Vec<(String, String)>,
    },
    Text(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct XPathMatch {
    pub xpath: String,
    pub matched_text: String,
    pub regex_matches: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct XPathForRegexResponse {
    pub file: String,
    pub regex: String,
    pub matches: Vec<XPathMatch>,
}

pub enum FileFormat {
    Html,
    Xml,
}
