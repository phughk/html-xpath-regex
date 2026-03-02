use regex::Regex;

use crate::types::{NodeKind, SimpleNode, XPathMatch};

/// Find all text nodes matching the regex and return their XPath expressions.
pub fn xpath_for_regex(root: &SimpleNode, regex: &Regex) -> Vec<XPathMatch> {
    let mut results = Vec::new();
    let mut path = Vec::new();
    find_matches(root, regex, &mut path, &mut results);
    results
        .into_iter()
        .map(|DfsWalkResult{path_indices, matched_full_text, regex_match_strings}| {
            let xpath = generate_xpath(root, &path_indices);
            XPathMatch {
                xpath,
                matched_text: matched_full_text,
                regex_matches: regex_match_strings,
            }
        })
        .collect()
}

struct DfsWalkResult {
    path_indices: Vec<usize>,
    matched_full_text: String,
    regex_match_strings: Vec<String>,
}

/// DFS walk to find text nodes matching the regex.
/// Returns (path_indices, full_text, regex_match_strings) for each match.
fn find_matches(
    node: &SimpleNode,
    regex: &Regex,
    path: &mut Vec<usize>,
    results: &mut Vec<DfsWalkResult>,
) {
    match &node.kind {
        NodeKind::Text(text) => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let matches: Vec<String> = regex
                    .find_iter(text)
                    .map(|m| m.as_str().to_string())
                    .collect();
                if !matches.is_empty() {
                    results.push(DfsWalkResult{path_indices: path.clone(), matched_full_text: text.clone(), regex_match_strings: matches});
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{NodeKind, SimpleNode};

    fn text_node(s: &str) -> SimpleNode {
        SimpleNode {
            kind: NodeKind::Text(s.to_string()),
            children: vec![],
        }
    }

    fn element(name: &str, attrs: Vec<(&str, &str)>, children: Vec<SimpleNode>) -> SimpleNode {
        SimpleNode {
            kind: NodeKind::Element {
                local_name: name.to_string(),
                attributes: attrs
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            },
            children,
        }
    }

    fn doc(children: Vec<SimpleNode>) -> SimpleNode {
        SimpleNode {
            kind: NodeKind::Document,
            children,
        }
    }

    #[test]
    fn test_simple_xpath() {
        let tree = crate::parsing::parse_html("<html><body><p>Hello World</p></body></html>");

        let regex = Regex::new("Hello").unwrap();
        let matches = xpath_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].xpath, "/html/body/p/text()");
        assert_eq!(matches[0].matched_text, "Hello World");
        assert_eq!(matches[0].regex_matches, vec!["Hello"]);
    }

    #[test]
    fn test_positional_index() {
        let tree = crate::parsing::parse_xml("<root><div>First</div><div>Second</div></root>").unwrap();

        let regex = Regex::new("Second").unwrap();
        let matches = xpath_for_regex(&tree, &regex);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].xpath, "/root/div[2]/text()");
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
    }
}
