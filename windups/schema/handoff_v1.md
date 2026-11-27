# Handoff JSON schema v1

One file holds one entry (one `Handoff`). The object is `"v": 1` plus the `Handoff` fields.
Unknown versions are rejected. Concatenated objects (JSONL) are allowed on stdin.

```json
{
  "v": 1,
  "file": "/media/The.Matrix.1999.mkv",
  "entry": {
    "kind": "movie",
    "lifecycle": "unverified",
    "title": "The Matrix",
    "year": "1999"
  },
  "candidates": [
    ["tmdb", {"title": "The Matrix", "year": "1999"}],
    ["embedded", {"title": "The.Matrix", "year": "1998"}]
  ],
  "conflicts": [
    ["year", [["tmdb", "1999"], ["embedded", "1998"]]]
  ],
  "from": "jack",
  "caller": null
}
```

| Field | Meaning |
|---|---|
| `v` | Schema version. Must be `1`. |
| `file` | Absolute or relative path of the media file the entry is about. |
| `entry` | The chosen fields (`string` → `string`), including `lifecycle` and any reasons. |
| `candidates` | Every source's answer, kept whole: `[source_name, fields]`. |
| `conflicts` | Disagreements the handing windup found: `[field, [[source, value], ...]]`. Recorded, not settled. |
| `from` | Who handed it over (e.g. `"jack"`). |
| `caller` | Who asked for this run, if a windup did (a name from `--caller`, e.g. `"flora"`). `null` if none. The loop guard. |

Jack keeps every handoff in his own `jack.db` (the `records.handoff` column, under a stable `id`) and syncs the same object to Daniel (`daniel::receive`); he no longer writes it beside the media. `jack export [-|DIR]` writes the kept records (JSONL). Daniel reads it with `daniel ingest <file.json ... | ->` (`accept` is an alias) into `media.db`.

Jack's `jack.db` stores the full handoff, including subjective fields (descriptions, posters). The copy he syncs to the database windup is the same object with those fields removed (see `jack/src/objective.rs`). Other windups ask Jack for the rest: `jack ask summaries`, `jack ask images`.

Jack only emits a settled `entry` when one identity agrees across sources, or when the caller gave an id flag (`--imdb`, `--tmdb`, `--thetvdb`/`--tvdb`; later `--wikidata` / `--wikipedia`). `--name` and `--year` only filter the search; they are not a trusted match. Even with an id he still looks up the same work on the other sources to fill in ids and facts. Unresolved runs keep candidates in `jack.db` and do not offer a list to the caller.
