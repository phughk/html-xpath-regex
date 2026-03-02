use regex::Regex;

use crate::types::{EvaluateXPathResult, NodeKind, SimpleNode, XPathMatch};

/// Find all text nodes matching the regex and return their XPath expressions.
pub fn xpath_for_regex(root: &SimpleNode, regex: &Regex) -> Vec<XPathMatch> {
    let mut results = Vec::new();
    let mut path = Vec::new();
    find_matches(root, regex, &mut path, &mut results);
    results
        .into_iter()
        .map(|DfsWalkResult{path_indices, matched_full_text, regex_match_strings, source_offset, match_byte_offsets}| {
            let xpath = generate_xpath(root, &path_indices);
            let file_offsets = match source_offset {
                Some(base) => match_byte_offsets.into_iter().map(|off| base + off).collect(),
                None => match_byte_offsets,
            };
            XPathMatch {
                xpath,
                matched_text: matched_full_text,
                regex_matches: regex_match_strings,
                file_offsets,
            }
        })
        .collect()
}

pub(crate) struct DfsWalkResult {
    pub path_indices: Vec<usize>,
    pub matched_full_text: String,
    pub regex_match_strings: Vec<String>,
    pub source_offset: Option<usize>,
    /// Byte offsets of each regex match within the text node.
    pub match_byte_offsets: Vec<usize>,
}

/// DFS walk to find text nodes matching the regex.
/// Returns (path_indices, full_text, regex_match_strings) for each match.
pub(crate) fn find_matches(
    node: &SimpleNode,
    regex: &Regex,
    path: &mut Vec<usize>,
    results: &mut Vec<DfsWalkResult>,
) {
    match &node.kind {
        NodeKind::Text(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let regex_matches: Vec<regex::Match> = regex.find_iter(text).collect();
                if !regex_matches.is_empty() {
                    let match_byte_offsets: Vec<usize> = regex_matches.iter().map(|m| m.start()).collect();
                    let regex_match_strings: Vec<String> = regex_matches.iter().map(|m| m.as_str().to_string()).collect();
                    results.push(DfsWalkResult{
                        path_indices: path.clone(),
                        matched_full_text: text.clone(),
                        regex_match_strings,
                        source_offset: node.source_offset,
                        match_byte_offsets,
                    });
                }
            }
        }
        NodeKind::Document | NodeKind::Element { .. } => {
            for (i, child) in node.children.iter().enumerate() {
                path.push(i);
                find_matches(child, regex, path, results);
                path.pop();
            }
        }
    }
}

/// Generate an XPath expression from the root to the node at the given path.
fn generate_xpath(root: &SimpleNode, path_indices: &[usize]) -> String {
    let mut xpath = String::new();
    let mut current = root;

    for &idx in path_indices {
        let child = &current.children[idx];

        match &child.kind {
            NodeKind::Element {
                local_name,
                attributes,
            } => {
                // If element has an id, use it as a shortcut
                if let Some(id) = attributes.iter().find(|(k, _)| k == "id").map(|(_, v)| v) {
                    xpath = format!("//*[@id='{id}']");
                } else {
                    // Count same-name siblings before this index
                    let same_name_before = current
                        .children
                        .iter()
                        .take(idx)
                        .filter(|c| matches!(&c.kind, NodeKind::Element { local_name: n, .. } if n == local_name))
                        .count();

                    let same_name_total = current
                        .children
                        .iter()
                        .filter(|c| matches!(&c.kind, NodeKind::Element { local_name: n, .. } if n == local_name))
                        .count();

                    if same_name_total > 1 {
                        xpath.push_str(&format!("/{}[{}]", local_name, same_name_before + 1));
                    } else {
                        xpath.push_str(&format!("/{}", local_name));
                    }
                }
            }
            NodeKind::Text(_) => {
                // Count text node siblings before this index
                let text_before = current
                    .children
                    .iter()
                    .take(idx)
                    .filter(|c| matches!(&c.kind, NodeKind::Text(_)))
                    .count();

                let text_total = current
                    .children
                    .iter()
                    .filter(|c| matches!(&c.kind, NodeKind::Text(_)))
                    .count();

                if text_total > 1 {
                    xpath.push_str(&format!("/text()[{}]", text_before + 1));
                } else {
                    xpath.push_str("/text()");
                }
            }
            NodeKind::Document => {}
        }

        current = child;
    }

    xpath
}

/// Evaluate an XPath expression against a SimpleNode tree and return matching text content with file offsets.
pub fn evaluate_xpath(root: &SimpleNode, xpath: &str) -> Result<Vec<EvaluateXPathResult>, String> {
    let steps = parse_xpath(xpath)?;
    let mut current_nodes = vec![root];

    for step in &steps {
        let mut next_nodes = Vec::new();
        for node in &current_nodes {
            match step {
                XPathStep::Child { name, position } => {
                    let matching: Vec<&SimpleNode> = node
                        .children
                        .iter()
                        .filter(|c| {
                            matches!(&c.kind, NodeKind::Element { local_name, .. } if local_name == name)
                        })
                        .collect();
                    if let Some(pos) = position {
                        if let Some(child) = matching.get(pos - 1) {
                            next_nodes.push(*child);
                        }
                    } else {
                        next_nodes.extend(matching);
                    }
                }
                XPathStep::Text { position } => {
                    let text_children: Vec<&SimpleNode> = node
                        .children
                        .iter()
                        .filter(|c| matches!(&c.kind, NodeKind::Text(t) if !t.trim().is_empty()))
                        .collect();
                    if let Some(pos) = position {
                        if let Some(child) = text_children.get(pos - 1) {
                            next_nodes.push(*child);
                        }
                    } else {
                        next_nodes.extend(text_children);
                    }
                }
                XPathStep::DescendantById { id } => {
                    let mut found = Vec::new();
                    find_by_id(root, id, &mut found);
                    next_nodes.extend(found);
                }
            }
        }
        current_nodes = next_nodes;
    }

    Ok(current_nodes
        .into_iter()
        .map(|n| {
            let text = collect_text(n);
            let file_offset = n.source_offset;
            EvaluateXPathResult { text, file_offset }
        })
        .filter(|r| !r.text.is_empty())
        .collect())
}

fn collect_text(node: &SimpleNode) -> String {
    match &node.kind {
        NodeKind::Text(t) => t.clone(),
        NodeKind::Document | NodeKind::Element { .. } => {
            node.children.iter().map(collect_text).collect::<Vec<_>>().join("")
        }
    }
}

fn find_by_id<'a>(node: &'a SimpleNode, target_id: &str, results: &mut Vec<&'a SimpleNode>) {
    if let NodeKind::Element { attributes, .. } = &node.kind {
        if attributes.iter().any(|(k, v)| k == "id" && v == target_id) {
            results.push(node);
            return;
        }
    }
    for child in &node.children {
        find_by_id(child, target_id, results);
    }
}

#[derive(Debug)]
enum XPathStep {
    Child { name: String, position: Option<usize> },
    Text { position: Option<usize> },
    DescendantById { id: String },
}

fn parse_xpath(xpath: &str) -> Result<Vec<XPathStep>, String> {
    let mut steps = Vec::new();
    let xpath = xpath.trim();

    if xpath.is_empty() {
        return Err("Empty XPath expression".to_string());
    }

    // Handle //*[@id='...'] prefix
    if xpath.starts_with("//*[@id='") {
        let rest = &xpath[9..]; // after //*[@id='
        let end = rest.find("']").ok_or("Malformed id selector")?;
        let id = &rest[..end];
        steps.push(XPathStep::DescendantById { id: id.to_string() });
        let remaining = &rest[end + 2..]; // after ']
        if remaining.is_empty() {
            return Ok(steps);
        }
        // Parse remaining steps after the id selector
        return parse_path_steps(remaining, &mut steps).map(|()| steps);
    }

    // Regular absolute path
    if !xpath.starts_with('/') {
        return Err(format!("XPath must start with '/' or '//*', got: {xpath}"));
    }

    parse_path_steps(xpath, &mut steps)?;
    Ok(steps)
}

fn parse_path_steps(path: &str, steps: &mut Vec<XPathStep>) -> Result<(), String> {
    // Split by '/' but skip leading empty segment
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    for seg in segments {
        if seg.starts_with("text()") {
            // text() or text()[n]
            let position = if seg.contains('[') {
                let start = seg.find('[').unwrap() + 1;
                let end = seg.find(']').ok_or("Malformed text() position")?;
                Some(seg[start..end].parse::<usize>().map_err(|e| format!("Invalid position: {e}"))?)
            } else {
                None
            };
            steps.push(XPathStep::Text { position });
        } else {
            // element or element[n]
            let (name, position) = if let Some(bracket_pos) = seg.find('[') {
                let name = &seg[..bracket_pos];
                let end = seg.find(']').ok_or("Malformed element position")?;
                let pos = seg[bracket_pos + 1..end]
                    .parse::<usize>()
                    .map_err(|e| format!("Invalid position: {e}"))?;
                (name.to_string(), Some(pos))
            } else {
                (seg.to_string(), None)
            };
            steps.push(XPathStep::Child { name, position });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_xpath() {
        let tree = crate::parsing::parse_html("<html><body><p>Hello World</p></body></html>");

        let regex = Regex::new("Hello").unwrap();
        let matches = xpath_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].xpath, "/html/body/p/text()");
        assert_eq!(matches[0].matched_text, "Hello World");
        assert_eq!(matches[0].regex_matches, vec!["Hello"]);
        assert_eq!(matches[0].file_offsets.len(), 1);
    }

    #[test]
    fn test_positional_index() {
        let tree = crate::parsing::parse_xml("<root><div>First</div><div>Second</div></root>").unwrap();

        let regex = Regex::new("Second").unwrap();
        let matches = xpath_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].xpath, "/root/div[2]/text()");
        // "Second" starts at byte offset 22 in "<root><div>First</div><div>Second</div></root>"
        assert_eq!(matches[0].file_offsets, vec![27]);
    }

    #[test]
    fn test_id_shortcut() {
        let tree = crate::parsing::parse_xml(r#"<root><div id="main"><p>Target</p></div></root>"#).unwrap();

        let regex = Regex::new("Target").unwrap();
        let matches = xpath_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].xpath, "//*[@id='main']/p/text()");
    }

    #[test]
    fn test_multiple_text_nodes() {
        let tree = crate::parsing::parse_xml(r#"<p>Hello <b>World</b> Today</p>"#).unwrap();

        let regex = Regex::new("Today").unwrap();
        let matches = xpath_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].xpath, "/p/text()[2]");
    }

    #[test]
    fn test_no_matches() {
        let tree = crate::parsing::parse_xml(r#"<root>Nothing here</root>"#).unwrap();

        let regex = Regex::new("MISSING").unwrap();
        let matches = xpath_for_regex(&tree, &regex);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_multiple_regex_matches_in_one_node() {
        let tree = crate::parsing::parse_xml(r#"<p>foo bar foo baz foo</p>"#).unwrap();

        let regex = Regex::new("foo").unwrap();
        let matches = xpath_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].regex_matches, vec!["foo", "foo", "foo"]);
        assert_eq!(matches[0].xpath, "/p/text()");
        // "foo bar foo baz foo" starts at byte 3 in "<p>foo bar foo baz foo</p>"
        // Regex matches at offsets 0, 8, 16 within the text
        assert_eq!(matches[0].file_offsets, vec![3, 11, 19]);
    }

    // evaluate_xpath tests

    #[test]
    fn test_evaluate_simple_path() {
        let tree = crate::parsing::parse_html("<html><body><p>Hello World</p></body></html>");
        let results = evaluate_xpath(&tree, "/html/body/p/text()").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Hello World");
        assert!(results[0].file_offset.is_some());
    }

    #[test]
    fn test_evaluate_positional() {
        let tree = crate::parsing::parse_xml("<root><div>First</div><div>Second</div></root>").unwrap();
        let results = evaluate_xpath(&tree, "/root/div[2]/text()").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Second");
        assert_eq!(results[0].file_offset, Some(27));
    }

    #[test]
    fn test_evaluate_by_id() {
        let tree = crate::parsing::parse_xml(r#"<root><div id="main"><p>Target</p></div></root>"#).unwrap();
        let results = evaluate_xpath(&tree, "//*[@id='main']/p/text()").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Target");
    }

    #[test]
    fn test_evaluate_element_collects_all_text() {
        let tree = crate::parsing::parse_xml(r#"<root><div>Hello <b>World</b></div></root>"#).unwrap();
        let results = evaluate_xpath(&tree, "/root/div").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Hello World");
    }

    #[test]
    fn test_evaluate_no_match() {
        let tree = crate::parsing::parse_xml("<root><item>Text</item></root>").unwrap();
        let results = evaluate_xpath(&tree, "/root/missing/text()").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_roundtrip_regex_then_evaluate() {
        let tree = crate::parsing::parse_xml(r#"<root><a>One</a><b>Two</b><a>Three</a></root>"#).unwrap();
        let regex = Regex::new("Three").unwrap();
        let matches = xpath_for_regex(&tree, &regex);
        assert_eq!(matches.len(), 1);

        let results = evaluate_xpath(&tree, &matches[0].xpath).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Three");
        assert!(results[0].file_offset.is_some());
    }

    #[test]
    fn test_file_offsets_xml_integration() {
        use std::io::Write;
        let content = "<root><div>First</div><div>Second</div></root>";
        let mut tmp = tempfile::Builder::new().suffix(".xml").tempfile().unwrap();
        tmp.write_all(content.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let tree = crate::parsing::parse_file(tmp.path()).unwrap();

        // xpath_for_regex
        let regex = Regex::new("Second").unwrap();
        let matches = xpath_for_regex(&tree, &regex);
        assert_eq!(matches.len(), 1);
        // <root><div>First</div><div> = 27 bytes, then "Second" starts
        assert_eq!(matches[0].file_offsets, vec![27]);

        // evaluate_xpath
        let results = evaluate_xpath(&tree, "/root/div[2]/text()").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_offset, Some(27));
    }

    #[test]
    fn test_file_offsets_html_integration() {
        use std::io::Write;
        let content = "<html><body><p>Hello World</p></body></html>";
        let mut tmp = tempfile::Builder::new().suffix(".html").tempfile().unwrap();
        tmp.write_all(content.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let tree = crate::parsing::parse_file(tmp.path()).unwrap();

        // xpath_for_regex
        let regex = Regex::new("Hello").unwrap();
        let matches = xpath_for_regex(&tree, &regex);
        assert_eq!(matches.len(), 1);
        // <html><body><p> = 15 bytes, then "Hello World" starts
        assert_eq!(matches[0].file_offsets, vec![15]);

        // evaluate_xpath
        let results = evaluate_xpath(&tree, "/html/body/p/text()").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_offset, Some(15));
    }
}
