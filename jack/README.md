# Jack

Rust CLI that tags and sorts movie and TV files (replaces iVI4 Pro). First app in the Windups group of small named apps.

- Requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Config: `config/jack.toml` (keep tweakable values here, not hard-coded)

Status: phases 1-4 done: CLI, config, scan, tag writing, TMDB/TheTVDB lookup, filename matching, composite merge, sort/rename.

## Install

```zsh
brew install am517/tap/windups-jack
mkdir -p ~/.config/windups/jack && cp "$(brew --prefix)/share/windups-jack/config.toml.example" ~/.config/windups/jack/config.toml
```

## Build from source (macOS, zsh)

```zsh
brew install atomicparsley exiftool mkvtoolnix
cargo build --release
mkdir -p ~/.config/windups/jack && cp config/jack.toml ~/.config/windups/jack/config.toml
# then add your keys under [providers.tmdb] and [providers.thetvdb]
```

## Usage

```zsh
jack ~/Movies/Inbox --dry-run -o ~/Media          # preview tags + renames; changes nothing
jack ~/Movies/the.matrix.1999.mp4 --imdb tt0133093 # tag in place
jack "Dune (1984).mkv" --name "Dune" --year 1984  # override the filename guess
jack "Dune (1984).mkv" --art landscape             # backdrop / episode still instead of poster
jack --set --art landscape --providers tmdb,omdb      # save flags as defaults in the main config (no paths: save only)
jack ~/TV/saiki-k/ --thetvdb https://www.thetvdb.com/series/the-disastrous-life-of-saiki-k -o ~/Media
jack ~/Movies/Inbox -d                             # download metadata + poster only (<file>.meta.json, <file>.poster.jpg)
jack Cosmos.mkv --tv                               # force TV (or --movie / --type auto|tv|movie; config: type = "tv")
jack ~/Movies --no-print --no-recursive             # every on/off flag has a --no-X form (last wins); --set saves false too
jack movie.mp4 -p                                   # print merged metadata (ids, ratings, art URL); changes/writes nothing
jack movie.mp4 -p --print-format json > m.json     # JSON array on stdout (report/warnings on stderr); -d -p = save files and print
```

## Config

- Main config: `~/.config/windups/jack/config.toml` or `--config <path>`; deep-merged over the built-in defaults in `config/jack.toml`, which documents every key.
- No sidecars: per-file values (`--name`, `--year`, ids, `--grouping`) are flags; jack never reads `<file>.*` files. `-d` output is for you; jack doesn't read it back. Art is a temp file, deleted after embedding (kept beside the file only with `-d`). The one exception is the versioned handoff JSON (`<file>.jack.json`) Jack writes when there is no database, so Daniel can ingest it later.
- Precedence: flags > main config > built-in defaults. `~` is expanded in paths.
- Tweakable there: extensions, tag tool and paths, provider order, keys and base URLs, per-field merge priority, filename regexes, sort templates, move/copy.

Packaging: see `packaging/homebrew/` (mirror of the `am517/tap` formula).

Database ids (IMDb/TMDB/TheTVDB, plus series/episode ids for TV) are written into the file
(MP4 iTunes freeform atoms, MKV global tags) and read back on later runs, so re-tagging skips
the filename guess. Tag names are under `[tagger.mp4_id_tags]` / `[tagger.mkv_id_tags]`.

OMDb (optional, needs a key) adds ratings (IMDb, Rotten Tomatoes, Metacritic), rated and awards,
written with the same tag maps (MP4 freeform atoms, e.g. `IMDB_RATING`; MKV simple tags, e.g.
`LAW_RATING`), plus the iTunes content rating (iTunEXTC) for `rated`. Without a key it is skipped
with a warning.

### Languages and naming

- `[providers] language = "en"` sets the metadata/tag language (TMDB `language`, TheTVDB translations via `language_codes`, e.g. en -> eng).
- `keep_original = true` also stores original-language names (`original_title`, `original_show`, `original_episode_title`) in meta.json, `--print`, MP4 freeform atoms (`ORIGINAL_TITLE`, ...) and MKV tags; `original_in_description` appends the original title to the description. Templates can use `{original_title}` / `{original_show}`.
- `[naming] file_language` (default `"original"`) and `folder_language` (default `"en"`) pick names for files and folders, e.g. `TV/The Apothecary Diaries/Season 03/薬屋のひとりごと - S03E01 - 蝗.mp4`. Missing names fall back to the other language. Save with `--set --file-language en --folder-language original`.
- Names are NFC-normalized, illegal/control characters replaced, trailing dots/spaces trimmed, and each file/folder name capped at `filename_max` (255; `filename_unit = "auto"`: characters on macOS, bytes on Linux).
- `tv_show_year = true` adds `(YYYY)` to show names; `[matching] episode_title_from_filename` uses the filename's episode title when providers have none.
- `image_base` should be the bare TMDB image root (`https://image.tmdb.org/t/p`); a trailing `/original` or `/wNNN` is stripped with a warning.

### Genres and grouping

- Descriptions: the longest one wins (`[merge] longest`), Jack's fallback until Sammy picks.
- Genres from every provider are merged, deduped, alphabetized and written as one string (`genre_case = "lower"`, `genre_separator = ","` under `[providers]`), e.g. `animation,comedy,family`.
- `--grouping <text>` (or `grouping = "..."` in config) writes iTunes Grouping (MP4 ©grp, MKV GROUPING).

### Sorting / renaming

**Files are only moved when sorting is on:** `-o DIR` (or `output` in config), or `[sort] enabled = true`. Otherwise jack tags in place. `--dry-run` prints the target (`move: ...`) without touching anything. Default templates:

- TV: `{tv_root}/{show_en} ({show_year})/Season {season}/{show_file} S{season:02}E{episode:02} - {episode_title_file}.{ext}` → `~/TV/The Apothecary Diaries (2023)/Season 3/薬屋のひとりごと S03E01 - 蝗.mp4`
- Movie: `{movie_root}/{title_en}/{title_file} ({year}).{ext}` → `~/Movies/Evolution/Evolution (2001).mp4`

`tv_root` / `movie_root` default to `~/TV` / `~/Movies`; `-o` replaces both. `_en` = primary language, `_file` / `_folder` = `[naming]` languages; `{x:02}` zero-pads.

### Choosing art (`--ask-art`)

`--ask-art` fetches the full image lists (TMDB posters/backdrops/season posters/stills in your language, the original language and untagged; TheTVDB artworks; the OMDb poster), orders them by art style, `[merge]` provider preference, language and resolution, and downloads up to `ask_art_max` into a folder named after the media file (e.g. `Evolution.2001/01.jpg`, `02.jpg`, ...), lists them on stderr (provider, kind, language, width x height), opens the folder (default on macOS only; `ask_art_open_folder`), and asks which to embed: a number, Enter for #1, or `s` to skip. Without a terminal it uses #1. The folder is deleted afterwards unless `-d` or `ask_art_keep = true`. Save as default with `--set --ask-art`; undo with `--no-ask-art`.

### Handoff JSON (Daniel)

Jack hands each entry to Daniel in-process (`daniel::Store::accept`). He can also write a versioned handoff JSON (`"v": 1`; see `../windups/schema/handoff_v1.md`):

- `[store] handoff_json` in `config/jack.toml`: `auto` (default) writes `<file>.jack.json` beside the media only when the database is disabled or unavailable; `always` writes it as well as calling Daniel; `off` never writes the beside-media file.
- `--emit-json -` writes one JSON object per entry to stdout (the run report goes to stderr, so this can be piped). `--emit-json DIR` writes `<filename>.jack.json` into that directory.
- Do not combine `--emit-json -` with `--print` if you need a clean JSON stream.

```zsh
daniel ingest --db ~/.local/share/windups/media.db movie.mkv.jack.json
jack movie.mkv --emit-json - | daniel ingest --db ~/.local/share/windups/media.db -
```

Re-ingesting the same file updates its entry; it does not create duplicates.

## API keys

Keys come from the config file first, then from project keys embedded at build time:

```zsh
JACK_TMDB_TOKEN=... JACK_TVDB_KEY=... JACK_OMDB_KEY=... cargo build --release
```

Per run, `--tmdb-token`, `--tvdb-key`, `--tvdb-pin` and `--omdb-key` beat both; with `--set` they are saved into the config. Give them without a value (`jack --set --omdb-key`) to be prompted with hidden input (or pipe the key on stdin), keeping keys out of shell history; inline values need `=` (`--omdb-key=...`). Keys are never printed (masked in `--set` output).

Config `[providers.tmdb] read_token`/`api_key` `[providers.thetvdb] api_key` and `[providers.omdb] api_key` override embedded keys. The TheTVDB subscriber `pin` is config-only. With neither, Jack reports which key to set. Never commit keys (`.env`, `*.key` and `secrets*` are git-ignored). Keys embedded in a binary can be extracted from it.

## Attribution

- Metadata provided by TheTVDB: https://thetvdb.com
- This product uses the TMDB API but is not endorsed or certified by TMDB: https://www.themoviedb.org

- Ratings, rated and awards from OMDb (The Open Movie Database): https://www.omdbapi.com (courtesy credit; OMDb's terms require none, but limit use to personal, non-commercial purposes)

These lines are also shown in `jack --help`.

## License

MIT, see [LICENSE](LICENSE).
