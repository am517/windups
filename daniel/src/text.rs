//! Writing a catalog's text layer: extracts, and the chunks made from usable ones.
use crate::catalog::Catalog;
use crate::config::RecordsCfg;
use rusqlite::{Connection, OptionalExtension, params};

/// Derived text of one record (subtitles, OCR, page text, ...).
pub struct Extract<'a> {
    pub record_id: i64,
    pub file_hash: Option<&'a str>,
    pub method: &'a str,
    pub language: Option<&'a str>,
    /// ok | held_needs_ocr | garbled | empty
    pub quality: &'a str,
    pub made_by: &'a str,
}

/// A searchable span of an extract. `level`: 1 overview, 2 structure, 3 passage.
pub struct Chunk<'a> {
    pub extract_id: i64,
    pub parent_id: Option<i64>,
    pub level: u8,
    pub locator: Option<&'a str>,
    pub language: Option<&'a str>,
    pub text: &'a str,
}

/// Personal records stay out until `[records] ingest` is turned on; other catalogs need a text layer.
fn allowed(catalog: Catalog, records: &RecordsCfg) -> Result<(), String> {
    if !catalog.has_text_layer() {
        return Err(format!(
            "no_text_layer: the {} catalog holds no text",
            catalog.name()
        ));
    }
    if catalog == Catalog::Records && !records.ingest {
        return Err("records_ingest_disabled: personal records are not ingested until Daniel is final ([records] ingest = false)".into());
    }
    Ok(())
}

pub fn add_extract(
    c: &Connection,
    catalog: Catalog,
    records: &RecordsCfg,
    e: &Extract,
) -> Result<i64, String> {
    allowed(catalog, records)?;
    c.execute(
        &format!(
            "INSERT INTO {}_extracts(record_id, file_hash, method, language, quality, made_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            catalog.name()
        ),
        params![e.record_id, e.file_hash, e.method, e.language, e.quality, e.made_by],
    )
    .map_err(|err| err.to_string())?;
    Ok(c.last_insert_rowid())
}

/// Adds a chunk, word-indexed by trigger. Only text from an `ok` extract is chunked, so empty or
/// garbled text is never indexed as if it were clean.
pub fn add_chunk(
    c: &Connection,
    catalog: Catalog,
    records: &RecordsCfg,
    ch: &Chunk,
) -> Result<i64, String> {
    allowed(catalog, records)?;
    let name = catalog.name();
    let quality: Option<String> = c
        .query_row(
            &format!("SELECT quality FROM {name}_extracts WHERE id = ?1"),
            [ch.extract_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match quality.as_deref() {
        None => return Err(format!("extract_not_found: extract {}", ch.extract_id)),
        Some("ok") => {}
        Some(q) => {
            return Err(format!(
                "extract_not_usable: extract {} is {q}; only ok text is chunked and indexed",
                ch.extract_id
            ));
        }
    }
    c.execute(
        &format!(
            "INSERT INTO {name}_chunks(extract_id, parent_id, level, locator, language, text, content_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        ),
        params![
            ch.extract_id,
            ch.parent_id,
            ch.level,
            ch.locator,
            ch.language,
            ch.text,
            larry::hash::text_hash(ch.text)
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(c.last_insert_rowid())
}

/// Marks a chunk's mentions as stored by a final extractor.
pub fn mark_mentions_done(c: &Connection, catalog: Catalog, chunk_id: i64) -> Result<(), String> {
    c.execute(
        &format!(
            "UPDATE {}_chunks SET mentions = 'done' WHERE id = ?1",
            catalog.name()
        ),
        [chunk_id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn extract(quality: &str) -> Extract<'_> {
        Extract {
            record_id: 1,
            file_hash: None,
            method: "subtitle",
            language: Some("en"),
            quality,
            made_by: "test 1",
        }
    }

    fn chunk(extract_id: i64, text: &str) -> Chunk<'_> {
        Chunk {
            extract_id,
            parent_id: None,
            level: 3,
            locator: Some("00:01:02"),
            language: Some("en"),
            text,
        }
    }

    #[test]
    fn ok_text_is_chunked_and_found_by_word_and_unusable_text_is_not() {
        let d = tempfile::tempdir().unwrap();
        let c = catalog::open(Catalog::Media, &d.path().join("media.db")).unwrap();
        let off = RecordsCfg::default();
        let ok = add_extract(&c, Catalog::Media, &off, &extract("ok")).unwrap();
        let id = add_chunk(
            &c,
            Catalog::Media,
            &off,
            &chunk(ok, "The café on Mountain Road"),
        )
        .unwrap();
        let hits: i64 = c
            .query_row(
                "SELECT count(*) FROM media_chunks_fts WHERE media_chunks_fts MATCH 'cafe mountain'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1, "diacritics folded, words matched");
        let garbled = add_extract(&c, Catalog::Media, &off, &extract("garbled")).unwrap();
        let e = add_chunk(&c, Catalog::Media, &off, &chunk(garbled, "x#@!")).unwrap_err();
        assert!(e.starts_with("extract_not_usable"), "{e}");
        assert!(
            add_chunk(&c, Catalog::Media, &off, &chunk(99, "x"))
                .unwrap_err()
                .starts_with("extract_not_found")
        );
        mark_mentions_done(&c, Catalog::Media, id).unwrap();
        let (state, hash): (String, String) = c
            .query_row(
                "SELECT mentions, content_hash FROM media_chunks WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, "done");
        assert!(hash.starts_with("b3:"));
    }

    #[test]
    fn personal_records_are_refused_while_ingest_is_off() {
        let d = tempfile::tempdir().unwrap();
        let c = catalog::open(Catalog::Records, &d.path().join("records.db")).unwrap();
        let e =
            add_extract(&c, Catalog::Records, &RecordsCfg::default(), &extract("ok")).unwrap_err();
        assert!(e.starts_with("records_ingest_disabled"), "{e}");
        let n: i64 = c
            .query_row("SELECT count(*) FROM records_extracts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
        let maps = catalog::open(Catalog::Maps, &d.path().join("maps.db")).unwrap();
        assert!(
            add_extract(&maps, Catalog::Maps, &RecordsCfg::default(), &extract("ok"))
                .unwrap_err()
                .starts_with("no_text_layer")
        );
    }
}
