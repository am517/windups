//! Personal records: with ingest off nothing is stored, and the source tree is only ever read.
use daniel::catalog::{self, Catalog};
use daniel::config::RecordsCfg;
use daniel::text::{Extract, add_extract};
use std::path::Path;

fn snapshot(dir: &Path) -> Vec<(String, u64, std::time::SystemTime, Vec<u8>)> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let m = e.metadata().unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                m.len(),
                m.modified().unwrap(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn a_record_fixture_is_read_only_and_not_ingested_while_ingest_is_off() {
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/records");
    let before = snapshot(&tree);
    let text = std::fs::read_to_string(tree.join("receipt.txt")).unwrap();
    assert!(text.contains("Receipt"));

    let d = tempfile::tempdir().unwrap();
    let c = catalog::open(Catalog::Records, &d.path().join("records.db")).unwrap();
    let e = add_extract(
        &c,
        Catalog::Records,
        &RecordsCfg::default(),
        &Extract {
            record_id: 1,
            file_hash: None,
            method: "pdf_text",
            language: None,
            quality: "ok",
            made_by: "test 1",
        },
    )
    .unwrap_err();
    assert!(e.starts_with("records_ingest_disabled"), "{e}");
    let rows: i64 = c
        .query_row(
            "SELECT (SELECT count(*) FROM records_extracts) + (SELECT count(*) FROM records_chunks) + (SELECT count(*) FROM records_items)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0);
    assert_eq!(
        snapshot(&tree),
        before,
        "no write, rename or sidecar in the tree"
    );
}
