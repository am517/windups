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
| `caller` | Who asked for this run, if a windup did (e.g. `"walter"`). `null` if none. The loop guard. |

Jack keeps every handoff in his own `jack.db` (the `records.handoff` column, under a stable `id`) and sends Walter the same object; he no longer writes it beside the media. He can also `--emit-json -` (stdout) or `--emit-json DIR`. Daniel reads it with `daniel ingest <file.json ... | ->` (`accept` is an alias) into `media.db`.
