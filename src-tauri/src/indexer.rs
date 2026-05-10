use std::path::Path;

use anyhow::Context;
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Field, STORED, STRING, Schema, TEXT},
    Index, IndexReader,
};

use crate::models::SearchResult;

#[derive(Clone)]
pub struct IndexManager {
    index_dir: std::path::PathBuf,
}

pub struct IndexFields {
    pub path: Field,
    pub category: Field,
    pub sub_category: Field,
    pub tags: Field,
    pub content: Field,
}

impl IndexManager {
    pub fn new(index_dir: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(index_dir)?;
        let manager = Self {
            index_dir: index_dir.to_path_buf(),
        };
        manager.open_or_create_index()?;
        Ok(manager)
    }

    fn schema() -> Schema {
        let mut builder = Schema::builder();
        builder.add_text_field("path", STRING | STORED);
        builder.add_text_field("category", STRING | STORED);
        builder.add_text_field("sub_category", STRING | STORED);
        builder.add_text_field("tags", TEXT | STORED);
        builder.add_text_field("content", TEXT | STORED);
        builder.build()
    }

    fn fields(schema: &Schema) -> IndexFields {
        IndexFields {
            path: schema.get_field("path").expect("path field exists"),
            category: schema.get_field("category").expect("category field exists"),
            sub_category: schema
                .get_field("sub_category")
                .expect("sub_category field exists"),
            tags: schema.get_field("tags").expect("tags field exists"),
            content: schema.get_field("content").expect("content field exists"),
        }
    }

    fn open_or_create_index(&self) -> anyhow::Result<Index> {
        match Index::open_in_dir(&self.index_dir) {
            Ok(index) => Ok(index),
            Err(_) => Index::create_in_dir(&self.index_dir, Self::schema())
                .context("failed creating tantivy index"),
        }
    }

    fn reader(index: &Index) -> anyhow::Result<IndexReader> {
        index.reader().context("failed creating index reader")
    }

    pub fn upsert_document(
        &self,
        path: &str,
        category: &str,
        sub_category: &str,
        tags: &[String],
        content_preview: &str,
    ) -> anyhow::Result<()> {
        let index = self.open_or_create_index()?;
        let schema = index.schema();
        let fields = Self::fields(&schema);
        let mut writer = index.writer(30_000_000)?;

        writer.delete_term(tantivy::Term::from_field_text(fields.path, path));
        writer.add_document(doc!(
            fields.path => path.to_string(),
            fields.category => category.to_string(),
            fields.sub_category => sub_category.to_string(),
            fields.tags => tags.join(" "),
            fields.content => content_preview.to_string(),
        ))?;
        writer.commit()?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
        let index = self.open_or_create_index()?;
        let reader = Self::reader(&index)?;
        let searcher = reader.searcher();
        let schema = index.schema();
        let fields = Self::fields(&schema);

        let query_parser = QueryParser::for_index(&index, vec![fields.content, fields.tags]);
        let parsed = query_parser.parse_query(query)?;
        let top_docs = searcher.search(&parsed, &TopDocs::with_limit(limit.max(1)))?;

        let mut results = Vec::new();
        for (score, doc_addr) in top_docs {
            let retrieved = searcher.doc::<tantivy::TantivyDocument>(doc_addr)?;
            let path = retrieved
                .get_first(fields.path)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let category = retrieved
                .get_first(fields.category)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let sub_category = retrieved
                .get_first(fields.sub_category)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let tags = retrieved
                .get_first(fields.tags)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            results.push(SearchResult {
                path,
                relevance_score: score,
                preview_metadata: format!("category={category}/{sub_category}; tags={tags}"),
            });
        }

        Ok(results)
    }
}
