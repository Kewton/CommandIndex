use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum LinkType {
    WikiLink,
    MarkdownLink,
}

impl fmt::Display for LinkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkType::WikiLink => write!(f, "WikiLink"),
            LinkType::MarkdownLink => write!(f, "MarkdownLink"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub target: String,
    pub link_type: LinkType,
}

/// テキストからリンクを抽出する
pub fn extract_links(content: &str) -> Vec<Link> {
    let mut links = Vec::new();

    // WikiLink: [[target]]
    extract_wiki_links(content, &mut links);

    // MarkdownLink: [text](target)
    extract_markdown_links(content, &mut links);

    links
}

fn extract_wiki_links(content: &str, links: &mut Vec<Link>) {
    let mut rest = content;
    while let Some(start) = rest.find("[[") {
        let after_open = start + 2;
        let remaining = &rest[after_open..];
        if let Some(end) = remaining.find("]]") {
            let target = &remaining[..end];
            if !target.is_empty() && !target.contains('\n') {
                links.push(Link {
                    target: target.to_string(),
                    link_type: LinkType::WikiLink,
                });
            }
            rest = &remaining[end + 2..];
        } else {
            break;
        }
    }
}

fn extract_markdown_links(content: &str, links: &mut Vec<Link>) {
    let mut rest = content;
    while let Some(bracket_start) = rest.find('[') {
        let after_bracket = &rest[bracket_start + 1..];

        // Skip wiki links
        if rest[bracket_start..].starts_with("[[") {
            rest = &rest[bracket_start + 2..];
            continue;
        }

        if let Some(bracket_end) = after_bracket.find(']') {
            let after_close = &after_bracket[bracket_end + 1..];
            if after_close.starts_with('(')
                && let Some(paren_end) = after_close.find(')')
            {
                let target = &after_close[1..paren_end];
                if !target.is_empty() {
                    links.push(Link {
                        target: target.to_string(),
                        link_type: LinkType::MarkdownLink,
                    });
                }
                rest = &after_close[paren_end + 1..];
                continue;
            }
        }

        rest = &rest[bracket_start + 1..];
    }
}
