use std::fmt;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::{Index, ReloadPolicy};

use crate::indexer::schema::IndexSchema;

#[derive(Debug)]
pub enum ReaderError {
    Tantivy(tantivy::TantivyError),
    Query(tantivy::query::QueryParserError),
}

impl fmt::Display for ReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReaderError::Tantivy(e) => write!(f, "Tantivy error: {e}"),
            ReaderError::Query(e) => write!(f, "Query parse error: {e}"),
        }
    }
}

impl std::error::Error for ReaderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReaderError::Tantivy(e) => Some(e),
            ReaderError::Query(e) => Some(e),
        }
    }
}

impl From<tantivy::TantivyError> for ReaderError {
    fn from(e: tantivy::TantivyError) -> Self {
        ReaderError::Tantivy(e)
    }
}

impl From<tantivy::query::QueryParserError> for ReaderError {
    fn from(e: tantivy::query::QueryParserError) -> Self {
        ReaderError::Query(e)
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: String,
    pub heading: String,
    pub body: String,
    pub tags: String,
    pub heading_level: u64,
    pub line_start: u64,
    pub score: f32,
}

pub struct IndexReaderWrapper {
    index: Index,
    schema: IndexSchema,
}

impl IndexReaderWrapper {
    /// ディスク上のインデックスを開く
    pub fn open(index_dir: &Path) -> Result<Self, ReaderError> {
        let schema = IndexSchema::new();
        let index = Index::open_in_dir(index_dir)?;
        IndexSchema::register_tokenizer(&index);
        Ok(Self { index, schema })
    }

    /// 既存のIndexオブジェクトから作成する（テスト用）
    pub fn from_index(index: Index) -> Self {
        let schema = IndexSchema::new();
        IndexSchema::register_tokenizer(&index);
        Self { index, schema }
    }

    /// クエリで検索し、上位N件を返す
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>, ReaderError> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.schema.heading, self.schema.body, self.schema.tags],
        );

        let query = query_parser.parse_query(query_str)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;

            let path = doc
                .get_first(self.schema.path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let heading = doc
                .get_first(self.schema.heading)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let body = doc
                .get_first(self.schema.body)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tags = doc
                .get_first(self.schema.tags)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let heading_level = doc
                .get_first(self.schema.heading_level)
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let line_start = doc
                .get_first(self.schema.line_start)
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            results.push(SearchResult {
                path,
                heading,
                body,
                tags,
                heading_level,
                line_start,
                score,
            });
        }

        Ok(results)
    }
}
