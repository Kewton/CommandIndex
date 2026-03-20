use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera_tantivy::tokenizer::LinderaTokenizer;
use tantivy::Index;
use tantivy::schema::{Field, STORED, STRING, Schema, TextFieldIndexing, TextOptions};
use tantivy::tokenizer::TextAnalyzer;

pub const TOKENIZER_NAME: &str = "lang_ja";

#[derive(Clone)]
pub struct IndexSchema {
    pub schema: Schema,
    pub path: Field,
    pub heading: Field,
    pub body: Field,
    pub tags: Field,
    pub heading_level: Field,
    pub line_start: Field,
}

impl IndexSchema {
    pub fn new() -> Self {
        let mut schema_builder = Schema::builder();

        let ja_text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer(TOKENIZER_NAME)
                    .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        let path = schema_builder.add_text_field("path", STRING | STORED);
        let heading = schema_builder.add_text_field("heading", ja_text_options.clone());
        let body = schema_builder.add_text_field("body", ja_text_options.clone());
        let tags = schema_builder.add_text_field("tags", ja_text_options);
        let heading_level =
            schema_builder.add_u64_field("heading_level", tantivy::schema::INDEXED | STORED);
        let line_start = schema_builder.add_u64_field("line_start", STORED);

        let schema = schema_builder.build();

        Self {
            schema,
            path,
            heading,
            body,
            tags,
            heading_level,
            line_start,
        }
    }

    fn create_lindera_tokenizer() -> LinderaTokenizer {
        let dictionary = load_dictionary("embedded://ipadic")
            .expect("Failed to load embedded ipadic dictionary");
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        LinderaTokenizer::from_segmenter(segmenter)
    }

    /// lindera 日本語トークナイザーをインデックスに登録する
    pub fn register_tokenizer(index: &Index) {
        let tokenizer = Self::create_lindera_tokenizer();
        let analyzer = TextAnalyzer::builder(tokenizer).build();
        index.tokenizers().register(TOKENIZER_NAME, analyzer);
    }
}

impl Default for IndexSchema {
    fn default() -> Self {
        Self::new()
    }
}
