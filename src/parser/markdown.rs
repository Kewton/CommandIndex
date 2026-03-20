use std::fmt;
use std::path::{Path, PathBuf};

use crate::parser::frontmatter::{self, Frontmatter};
use crate::parser::link::{self, Link};

#[derive(Debug)]
pub enum ParseError {
    Io(std::io::Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseError::Io(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError::Io(e)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub heading: String,
    pub level: u8,
    pub body: String,
    pub line_start: usize,
}

#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub path: PathBuf,
    pub frontmatter: Option<Frontmatter>,
    pub sections: Vec<Section>,
    pub links: Vec<Link>,
}

/// Markdownファイルをパースする
pub fn parse_file(path: &Path) -> Result<MarkdownDocument, ParseError> {
    let content = std::fs::read_to_string(path)?;
    let mut doc = parse_content(&content);
    doc.path = path.to_path_buf();
    Ok(doc)
}

/// Markdown文字列をパースする
pub fn parse_content(content: &str) -> MarkdownDocument {
    let (fm_str, body, fm_lines) = frontmatter::extract_frontmatter(content);

    let frontmatter = fm_str.and_then(|s| frontmatter::parse_frontmatter(&s));
    let sections = parse_sections(body, fm_lines);
    let links = link::extract_links(body);

    MarkdownDocument {
        path: PathBuf::new(),
        frontmatter,
        sections,
        links,
    }
}

/// heading単位でセクションに分割する
fn parse_sections(content: &str, line_offset: usize) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current_heading = String::new();
    let mut current_level: u8 = 0;
    let mut current_body = String::new();
    let mut current_line_start = line_offset + 1;
    let mut in_section = false;

    for (i, line) in content.lines().enumerate() {
        let line_num = line_offset + i + 1;

        if let Some((level, heading)) = parse_heading_line(line) {
            // Save previous section
            if in_section {
                sections.push(Section {
                    heading: current_heading.clone(),
                    level: current_level,
                    body: current_body.trim_end().to_string(),
                    line_start: current_line_start,
                });
            }

            current_heading = heading;
            current_level = level;
            current_body = String::new();
            current_line_start = line_num;
            in_section = true;
        } else if in_section && (!current_body.is_empty() || !line.is_empty()) {
            if !current_body.is_empty() {
                current_body.push('\n');
            }
            current_body.push_str(line);
        }
        // Lines before first heading are ignored (no section)
    }

    // Save last section
    if in_section {
        sections.push(Section {
            heading: current_heading,
            level: current_level,
            body: current_body.trim_end().to_string(),
            line_start: current_line_start,
        });
    }

    sections
}

/// heading行をパースする。`# heading` → Some((1, "heading"))
fn parse_heading_line(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }

    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }

    let rest = &trimmed[hashes..];
    // Must have a space after # (or be just "#")
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }

    let heading = rest.trim().to_string();
    Some((hashes as u8, heading))
}
