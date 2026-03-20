use std::fmt;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser};
use tantivy::schema::Value;
use tantivy::{Index, ReloadPolicy};

use crate::indexer::schema::IndexSchema;

/// Post-filter oversampling factor: fetch N times the limit when post-filters are active
const OVERSAMPLING_FACTOR: usize = 5;

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub query: String,
    pub tag: Option<String>,
    pub heading: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub path_prefix: Option<String>,
    pub file_type: Option<String>,
}

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
        let options = SearchOptions {
            query: query_str.to_string(),
            tag: None,
            heading: None,
            limit,
        };
        self.search_with_options(&options, &SearchFilters::default())
    }

    /// オプション付き検索
    pub fn search_with_options(
        &self,
        options: &SearchOptions,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>, ReaderError> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let searcher = reader.searcher();

        // Build BooleanQuery with sub-queries
        let mut sub_queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();

        // Main query across heading, body, tags
        let main_parser = QueryParser::for_index(
            &self.index,
            vec![self.schema.heading, self.schema.body, self.schema.tags],
        );
        let main_query = main_parser.parse_query(&options.query)?;
        sub_queries.push((Occur::Must, main_query));

        // Tag filter (search only in tags field)
        if let Some(ref tag) = options.tag {
            let tag_parser = QueryParser::for_index(&self.index, vec![self.schema.tags]);
            let tag_query = tag_parser.parse_query(tag)?;
            sub_queries.push((Occur::Must, tag_query));
        }

        // Heading filter (search only in heading field)
        if let Some(ref heading) = options.heading {
            let heading_parser = QueryParser::for_index(&self.index, vec![self.schema.heading]);
            let heading_query = heading_parser.parse_query(heading)?;
            sub_queries.push((Occur::Must, heading_query));
        }

        // Determine fetch limit with oversampling if post-filters are active
        let has_post_filter = filters.path_prefix.is_some() || filters.file_type.is_some();
        let fetch_limit = if has_post_filter {
            options.limit.saturating_mul(OVERSAMPLING_FACTOR)
        } else {
            options.limit
        };

        let boolean_query = BooleanQuery::new(sub_queries);
        let top_docs = searcher.search(&boolean_query, &TopDocs::with_limit(fetch_limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;
            let result = self.doc_to_search_result(&doc, score);

            // Post-filter: path prefix
            if let Some(ref prefix) = filters.path_prefix
                && !result.path.starts_with(prefix.as_str())
            {
                continue;
            }

            // Post-filter: file type
            if let Some(ref file_type) = filters.file_type
                && !matches_file_type(&result.path, file_type)
            {
                continue;
            }

            results.push(result);
            if results.len() >= options.limit {
                break;
            }
        }

        Ok(results)
    }

    /// TantivyDocument から SearchResult を生成するヘルパー
    fn doc_to_search_result(&self, doc: &tantivy::TantivyDocument, score: f32) -> SearchResult {
        SearchResult {
            path: Self::get_text(doc, self.schema.path),
            heading: Self::get_text(doc, self.schema.heading),
            body: Self::get_text(doc, self.schema.body),
            tags: Self::get_text(doc, self.schema.tags),
            heading_level: Self::get_u64(doc, self.schema.heading_level),
            line_start: Self::get_u64(doc, self.schema.line_start),
            score,
        }
    }

    /// ドキュメントからテキストフィールドを取得する
    fn get_text(doc: &tantivy::TantivyDocument, field: tantivy::schema::Field) -> String {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    /// ドキュメントからu64フィールドを取得する
    fn get_u64(doc: &tantivy::TantivyDocument, field: tantivy::schema::Field) -> u64 {
        doc.get_first(field).and_then(|v| v.as_u64()).unwrap_or(0)
    }
}

fn matches_file_type(path: &str, file_type: &str) -> bool {
    use crate::indexer::manifest::FileType;

    let ext = path.rsplit('.').next().unwrap_or("");

    match file_type {
        "markdown" | "md" => ext == "md",
        "typescript" | "ts" => ext == "ts" || ext == "tsx",
        "python" | "py" => ext == "py",
        "code" => {
            // All code file types (non-Markdown)
            FileType::from_extension(ext).is_some_and(|ft| ft.is_code())
        }
        _ => false,
    }
}
