# Jack — functional spec
Jack is the metadata windup.

## 1. Purpose
- Name: Jack
- Personality: Curiosity
- Function: Given a media file or folder, he finds out exactly what each file is from user input (flags), the file's own facts, and free online sources. He replaces the lookup half of iVI4 Pro.
- Workspace: He keeps everything in `jack.db`, and syncs records to Daniel.
- Constraints:
 - Jack returns facts only
 - Jack only gets metadata
 - He doesn't second-guess it or act on it
 - He uses global functions like saving to his own DB and answering requests.
 - He distinguishes facts (title, year, ids, cast) from truths (subjective text such as summaries)
 - Jack syncs facts with Daniel
 - Jack records truths for future reference but doesn't sync them outside his personal database

## 2. Identification rules
- Search context
 - The user can provide flags such as `--name` and `--year`
 - The file's embedded tags (i.e., id, name, date, etc.) count as user input and can be used to "fill in" the flags using the file's metadata.
 - Id flags can take a URL or an id and intelligently use either
- Fact
 - Saved in jack.db and synced with Daniel
 - Must be agreed upon by all sources
 - Flags such as `--name` and `--year` only filter the results but do not determine facts on their own.
 - An id flag (`--tmdb`, `--thetvdb`/`--tvdb`, `--imdb`, `--wikidata`, `--wikipedia`) anchors identity; Jack still looks the work up on the other sources to fill in ids and facts.
 - Id kinds: when a source keeps movies and TV under separate lookups (TMDB `/movie` and `/tv`; TheTVDB `/movies` and `/series`), the source lists them in its definition (`Provider::id_kinds`). Jack tries a user-supplied id under each kind before treating it as a failure. The order is: a forced `--type` alone; otherwise the hinted kind first (a season or episode means TV); otherwise the source's own order (TheTVDB: series, then movies; TMDB: movie, then TV). He moves on only on a 404. When every kind is a 404, he reports it clearly (`id_not_found`) and does not fall back to a title search on any source.
 - `--year` from the user: a found record more than `[matching] user_year_tolerance` years away (default 1; 0 = exact) is a different work, and the file stays unresolved (`year_mismatch`). Search lists are only candidates; a wrong year there is dropped, not counted as dissent.
 - Decided (Nov 27): the `--year` tolerance is ±1 (`user_year_tolerance = 1`). The id-kind order is: a forced `--type` alone, then the hinted kind, then the source's own order (TheTVDB series then movies; TMDB movie then TV). When the same number is a record of more than one kind and the first is the wrong work, the answer is `--type movie` or `--type tv`.
 - Evidence ranking (Nov 27): ids the user gives > sources found from those ids > the file's embedded tags. With a user id, the file's tags fill nothing: they don't pick the jack.db record or a lookup, and they are never looked up. Tags that disagree with every found source holding the same field are reported as "file tags look wrong", and are not used or counted as a source. Without a user id, embedded ids still count, as the only ids there are. An earlier jack.db record is reused with a user id only if it settled and carries that id; otherwise Jack looks the file up again.
 - Consensus: a match is accepted when at least `[matching] min_confirming` found sources (default 1) confirm it and no found source dissents, i.e. names a different work (`sources_disagree`). Sources that found nothing don't count either way. This holds with a user id too.
 - Jack never writes to a media file. Writing tags is Lauren's job. She may write only an entry Jack marked resolved, never one that is `unresolved` or needs judgment. To repair a file an older build tagged wrongly, see [repair_wrong_tags.md](repair_wrong_tags.md).
- Truth
 - Can vary between sources (summaries, images, etc.)
 - Saved in jack.db but not synced with Daniel
- Process
 - Check `jack.db` and file metadata first
 - Use free sources only, as defined in [coding_guidelines.md §3](coding_guidelines.md#3-language-dependencies-performance). Tools like yt-dlp and exiftool reading a site's or file's own metadata are allowed; Jack doesn't write his own scrapers.
 - Uses fuzzy matching to sort results by similarity, considering available context from metadata and user flags
 - Fuzzy matching threshold goes in config
 - Every decision records its score
 - Use a well-made crate for fuzzy matching (strsim)
 - Use traditional algorithmic decision-making processes to determine the correct result, even if Sammy is available
 - If using Sammy, provide her with the algorithmic analysis
- Result evaluation
 - Act like an experienced adult: deduce, don't assume, don't give up.
 - Must be the only result above 90% similarity
 - Use intelligent algorithms in reasoning
 - If using Sammy, provide her with algorithmic reasoning as well as search context and results list

## 3. When Jack can't decide
| Caller | Sammy available | Behavior |
|---|---|---|
| another windup (over Noise) | no | Return the unresolved result (status 1 with reason fields); nothing else. |
| another windup (over Noise) | yes | Talk it through with the caller like a normal agent (via Sammy). |
| person | no | Ask: "Would you like to see candidates (y/n)?" On yes, list the candidates once, grouped by source and numbered across the list; the person picks at most one per source (numbers separated by spaces, Enter for none). Not on a terminal, the list is printed once with a hint to use `--pick`. |
| person | yes | Talk like a normal agent (via Sammy). |

- Candidate picks (Nov 27): picked candidates count as confirming sources; unpicked ones are ignored. The picks then go through the same consensus rule, so picks naming different works settle nothing. `--pick "N ..."` gives the picks without a prompt, using the numbers in the list. Config `[candidates]`: `ask` (default true; asks on a terminal only) and `per_source` (search hits kept per source, default 5).
- In the list, sources whose rows have no title and no id show as one `not found: ...` line, and they count neither way.

## 4. Responses
- Every response is concise but descriptive
- Jack says what was done and why
- Partial results are named, e.g. "I saved the TheTVDB key, but the TMDB key didn't work because TMDB rejected it (401)."
- With Sammy available, Jack can phrase this more fluidly by providing Sammy the full output
- Jack is never silent.
- Exit codes:
 - 0 ok
 - 1 failed / unresolved / nothing processed
 - 2 usage or config error

## 5. Proposed flags
Global flags and commands are in [windups_base_spec.md](windups_base_spec.md); these are Jack's own.

| Flag | Meaning |
|---|---|
| `--tmdb`, `--thetvdb` (`--tvdb`), `--imdb`, `--youtube`, `--omdb` ID\|URL | Trusted ids; every source flag takes a link or an id |
| `--wikidata`, `--wikipedia` ID\|URL | Trusted ids |
| `--x` ID\|URL | X (twitter) post: media downloaded from X is identified by its post id. Mostly social media, but keep it. Accepts a bare status id or any `x.com`/`twitter.com/<user or i>/status/<id>` link |
| `--name`, `--year`, `--type tv\|movie\|auto`, `--tv`, `--movie` | Search filters |
| `--pick "N ..."` | Candidate picks by list number, at most one per source; no prompt (see §3) |
| `--providers a,b,c` | Source order |
| `--tmdb-key` (alias `--tmdb-token`), `--tvdb-key`, `--tvdb-pin`, `--omdb-key` VALUE | Set keys (value required; several per command; per-key report). `--tmdb-key` takes either TMDB credential (v3 key or read token) |
| `-p, --print` | Show the results (merged facts). Without it Jack still always says what he did (a status), but not the results |
| `--recursive` / `--no-recursive` | Folder walk |
| `ask summaries\|images [--file F] [--id ID]` | Truths/subjective data (Jack's own command) |

## 6. Modules (`jack/src/`, one job per file)
- CLI / settings: `cli`, `config`, `setcfg`, `setflags`, `secrets`, `universal`, `main`
- Scan / file facts: `scan`, `filemeta`, `facts`, `online/{embedded,filename,info_json,urls,youtube_video,crossover}`
- Sources: `providers` (TMDB, TheTVDB, OMDb), `sources/{tvmaze,wikidata,wikipedia,itunes,youtube,enrich}`, `fetch`
- Matching / decision: `matcher`, `candidates`, `resolver`, `decide`, `settle`, `conflicts`, `completeness`, `stage`, `review`, `reuse`, `escalate`
- Records / sync: `archive`, `handoff`, `objective`, `sync`, `to_daniel` (the generic parts — own-DB save, sync, answering requests — belong in the shared core; Jack keeps only his record shape)
- Output: `print`, `explain`, `profile`
- Truths: `ask`
- Secrets: `secrets` (encrypted in `jack.db`, only this instance; see identity_and_security.md)

## 7. Config, data, keys
- Folder: macOS `~/Library/Application Support/Windups/jack`, Linux `~/.local/share/windups/jack`; holds `jack` and `jack.db`. Backups go to George, the backup windup. The master key is **not** in the folder: it's in the OS keychain or `~/.config/windups/keys/jack.key` (see [identity_and_security.md](identity_and_security.md)).
- Settings, relationships and secrets are stored in proper tables in `jack.db`, laid out in [windups_base_spec.md](windups_base_spec.md) §5 (the same for every windup).
- Jack-specific table: `records`, one per file (id, created_at, file, handoff JSON v1, synced_at). Only facts are synced to Daniel; how truths (summaries, images) are laid out is still to be designed.
- TMDB: one `--tmdb-key VALUE` takes either credential (v3 key = 32 hex characters; read token = long `eyJ…` JWT), with `--tmdb-token` as an alias. TMDB's docs say both give the same access, so only one is ever needed.

## 8. Interactions
- Startup lookup through assistant (Flora) → matchmaker (Oliver) → database (Daniel) + llm (Sammy), falling back to isolation.
- Daniel: objective-only (facts) sync, per-record acks.
- All windup-to-windup traffic (Daniel sync included) uses Noise, same machine too.
- Sammy: needs_judgment, the can't-decide conversations (§3) and fluid phrasing (§4).
- Lauren: takes over tagging, art and sorting later.
- Walter: audits `jack.db` read-only.
