use std::path::Path;

use crate::types::{FileFormat, NodeKind, SimpleNode};

pub fn detect_format(path: &Path, content: &str) -> FileFormat {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => FileFormat::Html,
        Some("xml") | Some("xsl") | Some("xslt") | Some("svg") => FileFormat::Xml,
        Some("xhtml") => {
            if content.trim_start().starts_with("<?xml") {
                FileFormat::Xml
            } else {
                FileFormat::Html
            }
        }
        _ => {
            if content.trim_start().starts_with("<?xml") {
                FileFormat::Xml
            } else {
                FileFormat::Html
            }
        }
    }
}

pub fn parse_file(path: &Path) -> Result<SimpleNode, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {e}"))?;

    let format = detect_format(path, &content);

    match format {
        FileFormat::Html => Ok(parse_html(&content)),
        FileFormat::Xml => parse_xml(&content),
    }
}

pub(crate) fn parse_html(content: &str) -> SimpleNode {
    let document = scraper::Html::parse_document(content);
    let mut root = convert_html_node(document.tree.root());
    assign_html_offsets(&mut root, content, &mut 0);
    root
}

fn convert_html_node(node_ref: ego_tree::NodeRef<scraper::Node>) -> SimpleNode {
    let kind = match node_ref.value() {
        scraper::Node::Document | scraper::Node::Fragment => NodeKind::Document,
        scraper::Node::Element(el) => NodeKind::Element {
            local_name: el.name().to_string(),
            attributes: el
                .attrs()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        },
        scraper::Node::Text(text) => NodeKind::Text(text.to_string()),
        _ => NodeKind::Text(String::new()),
    };

    let children: Vec<SimpleNode> = node_ref.children().map(convert_html_node).collect();

    SimpleNode {
        kind,
        children,
        source_offset: 0,
    }
}

/// Post-process HTML tree to assign byte offsets by scanning the raw content.
/// This is approximate for HTML with decoded entities.
fn assign_html_offsets(node: &mut SimpleNode, content: &str, cursor: &mut usize) {
    match &node.kind {
        NodeKind::Text(text) => {
            let search_text = text.trim();
            if !search_text.is_empty() {
                if let Some(pos) = content[*cursor..].find(search_text) {
                    node.source_offset = *cursor + pos;
                    *cursor += pos + search_text.len();
                }
            }
        }
        NodeKind::Element { local_name, .. } => {
            // Find the opening tag in the raw content
            let tag_pattern = format!("<{}", local_name);
            if let Some(pos) = content[*cursor..].find(&tag_pattern) {
                node.source_offset = *cursor + pos;
            }
            for child in &mut node.children {
                assign_html_offsets(child, content, cursor);
            }
        }
        NodeKind::Document => {
            for child in &mut node.children {
                assign_html_offsets(child, content, cursor);
            }
        }
    }
}

pub(crate) fn parse_xml(content: &str) -> Result<SimpleNode, String> {
    let doc =
        roxmltree::Document::parse(content).map_err(|e| format!("Failed to parse XML: {e}"))?;

    Ok(convert_xml_node(&doc.root()))
}

fn convert_xml_node(node: &roxmltree::Node) -> SimpleNode {
    let kind = match node.node_type() {
        roxmltree::NodeType::Root => NodeKind::Document,
        roxmltree::NodeType::Element => NodeKind::Element {
            local_name: node.tag_name().name().to_string(),
            attributes: node
                .attributes()
                .map(|a| (a.name().to_string(), a.value().to_string()))
                .collect(),
        },
        roxmltree::NodeType::Text => NodeKind::Text(node.text().unwrap_or("").to_string()),
        _ => NodeKind::Text(String::new()),
    };

    let source_offset = Some(node.range().start);
    let children: Vec<SimpleNode> = node.children().map(|c| convert_xml_node(&c)).collect();

    SimpleNode {
        kind,
        children,
        source_offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_html_basic() {
        let html = "<html><body><p>Hello World</p></body></html>";
        let root = parse_html(html);
        assert!(matches!(root.kind, NodeKind::Document));
        assert!(!root.children.is_empty());
    }

    #[test]
    fn test_parse_xml_basic() {
        let xml = "<?xml version=\"1.0\"?><root><item>Text</item></root>";
        let result = parse_xml(xml);
        assert!(result.is_ok());
        let root = result.unwrap();
        assert!(matches!(root.kind, NodeKind::Document));
    }

    #[test]
    fn test_detect_format_html() {
        assert!(matches!(
            detect_format(Path::new("test.html"), ""),
            FileFormat::Html
        ));
    }

    #[test]
    fn test_detect_format_xml() {
        assert!(matches!(
            detect_format(Path::new("test.xml"), ""),
            FileFormat::Xml
        ));
    }

    #[test]
    fn test_detect_format_sniff() {
        assert!(matches!(
            detect_format(Path::new("test.txt"), "<?xml version=\"1.0\"?>"),
            FileFormat::Xml
        ));
    }
}
