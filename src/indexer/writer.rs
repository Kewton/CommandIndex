use std::fmt;
use std::path::Path;
use tantivy::{Index, IndexWriter as TantivyIndexWriter, Term, doc};

use crate::indexer::schema::IndexSchema;

const WRITER_HEAP_SIZE: usize = 50_000_000; // 50MB

#[derive(Debug)]
pub enum WriterError {
    Tantivy(tantivy::TantivyError),
    Io(std::io::Error),
}

impl fmt::Display for WriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriterError::Tantivy(e) => write!(f, "Tantivy error: {e}"),
            WriterError::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl std::error::Error for WriterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WriterError::Tantivy(e) => Some(e),
            WriterError::Io(e) => Some(e),
        }
    }
}

impl From<tantivy::TantivyError> for WriterError {
    fn from(e: tantivy::TantivyError) -> Self {
        WriterError::Tantivy(e)
    }
}

impl From<std::io::Error> for WriterError {
    fn from(e: std::io::Error) -> Self {
        WriterError::Io(e)
    }
}

pub struct SectionDoc {
    pub path: String,
    pub heading: String,
    pub body: String,
    pub tags: String,
    pub heading_level: u64,
    pub line_start: u64,
}

pub struct IndexWriterWrapper {
    writer: TantivyIndexWriter,
    schema: IndexSchema,
}

impl IndexWriterWrapper {
    /// ディスク上のインデックスを開き、writerを作成する
    pub fn open(index_dir: &Path) -> Result<Self, WriterError> {
        let schema = IndexSchema::new();
        std::fs::create_dir_all(index_dir)?;
        let index = Index::create_in_dir(index_dir, schema.schema.clone())?;
        IndexSchema::register_tokenizer(&index);
        let writer = index.writer(WRITER_HEAP_SIZE)?;
        Ok(Self { writer, schema })
    }

    /// 既存のインデックスを開く（上書きなし）
    pub fn open_existing(index_dir: &Path) -> Result<Self, WriterError> {
        let schema = IndexSchema::new();
        let index = Index::open_in_dir(index_dir)?;
        IndexSchema::register_tokenizer(&index);
        let writer = index.writer(WRITER_HEAP_SIZE)?;
        Ok(Self { writer, schema })
    }

    /// RAMベースのインデックスを作成する（テスト用）
    pub fn open_in_ram() -> Result<(Self, Index), WriterError> {
        let schema = IndexSchema::new();
        let index = Index::create_in_ram(schema.schema.clone());
        IndexSchema::register_tokenizer(&index);
        let writer = index.writer(WRITER_HEAP_SIZE)?;
        Ok((Self { writer, schema }, index))
    }

    /// ドキュメント（セクション）を追加する
    pub fn add_section(&mut self, section: &SectionDoc) -> Result<(), WriterError> {
        self.writer.add_document(doc!(
            self.schema.path => section.path.clone(),
            self.schema.heading => section.heading.clone(),
            self.schema.body => section.body.clone(),
            self.schema.tags => section.tags.clone(),
            self.schema.heading_level => section.heading_level,
            self.schema.line_start => section.line_start,
        ))?;
        Ok(())
    }

    /// path フィールド（STRING 型）をキーに、該当パスの全ドキュメントを削除する。
    /// 1ファイルに複数セクションがある場合も全て削除される。
    pub fn delete_by_path(&mut self, path: &str) -> Result<(), WriterError> {
        let term = Term::from_field_text(self.schema.path, path);
        self.writer.delete_term(term);
        Ok(())
    }

    /// 変更をコミットする
    pub fn commit(&mut self) -> Result<(), WriterError> {
        self.writer.commit()?;
        Ok(())
    }
}
