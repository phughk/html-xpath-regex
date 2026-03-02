use regex::Regex;

use crate::types::{CssSelectorMatch, EvaluateXPathResult, NodeKind, SimpleNode};
use crate::xpath::{find_matches, DfsWalkResult};

/// Find all text nodes matching the regex and return CSS selectors for their parent elements.
pub fn css_selector_for_regex(root: &SimpleNode, regex: &Regex) -> Vec<CssSelectorMatch> {
    let mut results = Vec::new();
    let mut path = Vec::new();
    find_matches(root, regex, &mut path, &mut results);
    results
        .into_iter()
        .map(
            |DfsWalkResult {
                 path_indices,
                 matched_full_text,
                 regex_match_strings,
                 source_offset,
                 match_byte_offsets,
             }| {
                let selector = generate_css_selector(root, &path_indices);
                let file_offsets = match source_offset {
                    Some(base) => match_byte_offsets
                        .into_iter()
                        .map(|off| base + off)
                        .collect(),
                    None => match_byte_offsets,
                };
                CssSelectorMatch {
                    selector,
                    matched_text: matched_full_text,
                    regex_matches: regex_match_strings,
                    file_offsets,
                }
            },
        )
        .collect()
}

/// Generate a CSS selector from the root to the node at the given path.
/// Since CSS selectors can't target text nodes, the selector targets the parent element.
fn generate_css_selector(root: &SimpleNode, path_indices: &[usize]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut current = root;
    let mut use_direct_from = 0;

    for (step, &idx) in path_indices.iter().enumerate() {
        let child = &current.children[idx];

        match &child.kind {
            NodeKind::Element {
                local_name,
                attributes,
            } => {
                // If element has an id, reset the selector to just the id
                if let Some(id) = attributes.iter().find(|(k, _)| k == "id").map(|(_, v)| v) {
                    parts.clear();
                    parts.push(format!("#{id}"));
                    use_direct_from = step + 1;
                } else {
                    // Count same-name siblings to determine if :nth-of-type is needed
                    let same_name_total = current
                        .children
                        .iter()
                        .filter(|c| {
                            matches!(&c.kind, NodeKind::Element { local_name: n, .. } if n == local_name)
                        })
                        .count();

                    if same_name_total > 1 {
                        let same_name_before = current
                            .children
                            .iter()
                            .take(idx)
                            .filter(|c| {
                                matches!(&c.kind, NodeKind::Element { local_name: n, .. } if n == local_name)
                            })
                            .count();
                        parts.push(format!(
                            "{}:nth-of-type({})",
                            local_name,
                            same_name_before + 1
                        ));
                    } else {
                        parts.push(local_name.clone());
                    }
                }
            }
            NodeKind::Text(_) => {
                // CSS can't select text nodes — selector stays at the parent element
            }
            NodeKind::Document => {}
        }

        current = child;
    }

    // Skip the implied root document
    let _ = use_direct_from;
    parts.join(" > ")
}

/// Evaluate a CSS selector against a SimpleNode tree and return matching text content.
pub fn evaluate_css_selector(
    root: &SimpleNode,
    selector: &str,
) -> Result<Vec<EvaluateXPathResult>, String> {
    let steps = parse_css_selector(selector)?;
    let mut current_nodes = vec![root];

    for step in &steps {
        let mut next_nodes = Vec::new();
        for node in &current_nodes {
            match step {
                CssStep::Element { name, nth_of_type } => {
                    let matching: Vec<&SimpleNode> = node
                        .children
                        .iter()
                        .filter(|c| {
                            matches!(&c.kind, NodeKind::Element { local_name, .. } if local_name == name)
                        })
                        .collect();
                    if let Some(pos) = nth_of_type {
                        if let Some(child) = matching.get(pos - 1) {
                            next_nodes.push(*child);
                        }
                    } else {
                        next_nodes.extend(matching);
                    }
                }
                CssStep::Id { id } => {
                    find_by_id(root, id, &mut next_nodes);
                }
                CssStep::Class { class } => {
                    find_by_class(node, class, &mut next_nodes);
                }
                CssStep::Attribute { name, value } => {
                    find_by_attribute(node, name, value.as_deref(), &mut next_nodes);
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
        NodeKind::Document | NodeKind::Element { .. } => node
            .children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(""),
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

fn find_by_class<'a>(node: &'a SimpleNode, target_class: &str, results: &mut Vec<&'a SimpleNode>) {
    for child in &node.children {
        if let NodeKind::Element { attributes, .. } = &child.kind {
            let has_class = attributes.iter().any(|(k, v)| {
                k == "class" && v.split_whitespace().any(|c| c == target_class)
            });
            if has_class {
                results.push(child);
            }
        }
        find_by_class(child, target_class, results);
    }
}

fn find_by_attribute<'a>(
    node: &'a SimpleNode,
    attr_name: &str,
    attr_value: Option<&str>,
    results: &mut Vec<&'a SimpleNode>,
) {
    for child in &node.children {
        if let NodeKind::Element { attributes, .. } = &child.kind {
            let matches = attributes.iter().any(|(k, v)| {
                k == attr_name && attr_value.map_or(true, |val| v == val)
            });
            if matches {
                results.push(child);
            }
        }
        find_by_attribute(child, attr_name, attr_value, results);
    }
}

#[derive(Debug)]
enum CssStep {
    Element {
        name: String,
        nth_of_type: Option<usize>,
    },
    Id {
        id: String,
    },
    Class {
        class: String,
    },
    Attribute {
        name: String,
        value: Option<String>,
    },
}

fn parse_css_selector(selector: &str) -> Result<Vec<CssStep>, String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err("Empty CSS selector".to_string());
    }

    let mut steps = Vec::new();

    // Split by " > " (direct child combinator)
    let parts: Vec<&str> = split_css_parts(selector);

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if part.starts_with('#') {
            // ID selector: #myid
            steps.push(CssStep::Id {
                id: part[1..].to_string(),
            });
        } else if part.starts_with('.') {
            // Class selector: .myclass
            steps.push(CssStep::Class {
                class: part[1..].to_string(),
            });
        } else if part.starts_with('[') {
            // Attribute selector: [attr] or [attr="value"]
            let inner = part
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .ok_or("Malformed attribute selector")?;
            if let Some(eq_pos) = inner.find('=') {
                let name = inner[..eq_pos].to_string();
                let value = inner[eq_pos + 1..]
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                steps.push(CssStep::Attribute {
                    name,
                    value: Some(value),
                });
            } else {
                steps.push(CssStep::Attribute {
                    name: inner.to_string(),
                    value: None,
                });
            }
        } else {
            // Element selector, possibly with :nth-of-type(n)
            if let Some(colon_pos) = part.find(':') {
                let name = part[..colon_pos].to_string();
                let pseudo = &part[colon_pos + 1..];
                if pseudo.starts_with("nth-of-type(") && pseudo.ends_with(')') {
                    let n_str = &pseudo[12..pseudo.len() - 1];
                    let n = n_str
                        .parse::<usize>()
                        .map_err(|e| format!("Invalid nth-of-type position: {e}"))?;
                    steps.push(CssStep::Element {
                        name,
                        nth_of_type: Some(n),
                    });
                } else {
                    return Err(format!("Unsupported pseudo-class: {pseudo}"));
                }
            } else {
                steps.push(CssStep::Element {
                    name: part.to_string(),
                    nth_of_type: None,
                });
            }
        }
    }

    Ok(steps)
}

/// Split a CSS selector by the `>` combinator, handling spaces around it.
fn split_css_parts(selector: &str) -> Vec<&str> {
    selector.split('>').map(|s| s.trim()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_css_selector() {
        let tree = crate::parsing::parse_html("<html><body><p>Hello World</p></body></html>");

        let regex = Regex::new("Hello").unwrap();
        let matches = css_selector_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].selector, "html > body > p");
        assert_eq!(matches[0].matched_text, "Hello World");
        assert_eq!(matches[0].regex_matches, vec!["Hello"]);
        assert_eq!(matches[0].file_offsets.len(), 1);
    }

    #[test]
    fn test_positional_nth_of_type() {
        let tree =
            crate::parsing::parse_xml("<root><div>First</div><div>Second</div></root>").unwrap();

        let regex = Regex::new("Second").unwrap();
        let matches = css_selector_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].selector, "root > div:nth-of-type(2)");
        assert_eq!(matches[0].file_offsets, vec![27]);
    }

    #[test]
    fn test_id_shortcut() {
        let tree = crate::parsing::parse_xml(
            r#"<root><div id="main"><p>Target</p></div></root>"#,
        )
        .unwrap();

        let regex = Regex::new("Target").unwrap();
        let matches = css_selector_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].selector, "#main > p");
    }

    #[test]
    fn test_no_matches() {
        let tree = crate::parsing::parse_xml(r#"<root>Nothing here</root>"#).unwrap();

        let regex = Regex::new("MISSING").unwrap();
        let matches = css_selector_for_regex(&tree, &regex);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_multiple_regex_matches_in_one_node() {
        let tree = crate::parsing::parse_xml(r#"<p>foo bar foo baz foo</p>"#).unwrap();

        let regex = Regex::new("foo").unwrap();
        let matches = css_selector_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].regex_matches, vec!["foo", "foo", "foo"]);
        assert_eq!(matches[0].selector, "p");
        assert_eq!(matches[0].file_offsets, vec![3, 11, 19]);
    }

    // evaluate_css_selector tests

    #[test]
    fn test_evaluate_simple_selector() {
        let tree = crate::parsing::parse_html("<html><body><p>Hello World</p></body></html>");
        let results = evaluate_css_selector(&tree, "html > body > p").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Hello World");
        assert!(results[0].file_offset.is_some());
    }

    #[test]
    fn test_evaluate_nth_of_type() {
        let tree =
            crate::parsing::parse_xml("<root><div>First</div><div>Second</div></root>").unwrap();
        let results = evaluate_css_selector(&tree, "root > div:nth-of-type(2)").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Second");
        assert_eq!(results[0].file_offset, Some(22));
    }

    #[test]
    fn test_evaluate_by_id() {
        let tree = crate::parsing::parse_xml(
            r#"<root><div id="main"><p>Target</p></div></root>"#,
        )
        .unwrap();
        let results = evaluate_css_selector(&tree, "#main > p").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Target");
    }

    #[test]
    fn test_evaluate_collects_all_text() {
        let tree =
            crate::parsing::parse_xml(r#"<root><div>Hello <b>World</b></div></root>"#).unwrap();
        let results = evaluate_css_selector(&tree, "root > div").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Hello World");
    }

    #[test]
    fn test_evaluate_no_match() {
        let tree = crate::parsing::parse_xml("<root><item>Text</item></root>").unwrap();
        let results = evaluate_css_selector(&tree, "root > missing").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_roundtrip_css_then_evaluate() {
        let tree =
            crate::parsing::parse_xml(r#"<root><a>One</a><b>Two</b><a>Three</a></root>"#)
                .unwrap();
        let regex = Regex::new("Three").unwrap();
        let matches = css_selector_for_regex(&tree, &regex);
        assert_eq!(matches.len(), 1);

        let results = evaluate_css_selector(&tree, &matches[0].selector).unwrap();
        // a:nth-of-type(2) matches the second <a> element
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Three");
        assert!(results[0].file_offset.is_some());
    }

    #[test]
    fn test_evaluate_by_class() {
        let tree = crate::parsing::parse_xml(
            r#"<root><div class="container"><p>Inside</p></div></root>"#,
        )
        .unwrap();
        let results = evaluate_css_selector(&tree, ".container > p").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Inside");
    }

    #[test]
    fn test_file_offsets_xml_integration() {
        use std::io::Write;
        let content = "<root><div>First</div><div>Second</div></root>";
        let mut tmp = tempfile::Builder::new()
            .suffix(".xml")
            .tempfile()
            .unwrap();
        tmp.write_all(content.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let tree = crate::parsing::parse_file(tmp.path()).unwrap();

        let regex = Regex::new("Second").unwrap();
        let matches = css_selector_for_regex(&tree, &regex);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file_offsets, vec![27]);

        let results = evaluate_css_selector(&tree, "root > div:nth-of-type(2)").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_offset, Some(22));
    }

    #[test]
    fn test_file_offsets_html_integration() {
        use std::io::Write;
        let content = "<html><body><p>Hello World</p></body></html>";
        let mut tmp = tempfile::Builder::new()
            .suffix(".html")
            .tempfile()
            .unwrap();
        tmp.write_all(content.as_bytes()).unwrap();
        tmp.flush().unwrap();

        let tree = crate::parsing::parse_file(tmp.path()).unwrap();

        let regex = Regex::new("Hello").unwrap();
        let matches = css_selector_for_regex(&tree, &regex);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file_offsets, vec![15]);

        let results = evaluate_css_selector(&tree, "html > body > p").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].file_offset.is_some());
    }
}
