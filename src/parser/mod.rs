pub mod code;
pub mod frontmatter;
pub mod ignore;
pub mod link;
pub mod markdown;
pub mod python;
pub mod typescript;

pub use frontmatter::Frontmatter;
pub use link::{Link, LinkType};
pub use markdown::{MarkdownDocument, Section};

use std::path::Path;
use walkdir::WalkDir;

use crate::parser::markdown::ParseError;

/// 指定ディレクトリ配下の `.md` ファイルを再帰的に列挙し、パースする
pub fn parse_directory(root: &Path) -> Result<Vec<MarkdownDocument>, ParseError> {
    let mut documents = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        let doc = markdown::parse_file(entry.path())?;
        documents.push(doc);
    }

    Ok(documents)
}
