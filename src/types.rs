use serde::Serialize;

/// Unified DOM node abstraction used by both HTML and XML parsers.
#[derive(Debug, Clone)]
pub struct SimpleNode {
    pub kind: NodeKind,
    pub children: Vec<SimpleNode>,
    /// Byte offset of this node's content in the original source file.
    pub source_offset: Option<usize>,
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
    /// Byte offsets from the beginning of the file for each regex match.
    pub file_offsets: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvaluateXPathResult {
    pub text: String,
    /// Byte offset from the beginning of the file.
    pub file_offset: Option<usize>,
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
