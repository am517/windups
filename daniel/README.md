# Daniel

The database windup: takes handoffs (`windups::handoff`), adds Larry's hash and Ruth's reference, stores them in media.db, then calls Walter if installed. See `PROFILE.toml` and `../windups/WINDUPS.md`.

The on-disk form is one versioned JSON object per entry (`"v": 1`; [`handoff_v1.md`](../windups/schema/handoff_v1.md)).

```zsh
daniel ingest [--db PATH] file.jack.json another.json
daniel ingest --db ~/.local/share/windups/media.db -
daniel accept file.jack.json   # alias of ingest
daniel --capabilities
```

`ingest` (and `accept`) print one line per file and exit non-zero if any file fails. Re-ingesting the same file updates its entry; it does not duplicate rows. A JSON whose `"v"` is not `1` is rejected.

```zsh
cargo test -p daniel --test ingest
cargo test --workspace
```
