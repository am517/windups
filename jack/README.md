# Jack

Jack finds out what a movie or TV file is and reports the facts. He reads the file's own tags, then free online sources. He does not tag files, embed art, or sort them; that work belongs to Lauren later.

- Requirements: [REQUIREMENTS.md](REQUIREMENTS.md)
- Config: `config/jack.toml` (keep tweakable values here, not hard-coded)

## Install

```zsh
brew install am517/tap/windups-jack
jack setup   # or just run jack: makes the data folder and links ~/.local/bin/jack
```

## Build from source (macOS, zsh)

```zsh
cargo build --release
jack setup   # makes the data folder (config, jack.db, backups/) and links ~/.local/bin/jack
# optional, for online video metadata: yt-dlp and exiftool
# then add your keys: jack --set --tmdb-key KEY --tvdb-key KEY
```

## Usage

```zsh
jack ~/Movies/the.matrix.1999.mp4 --imdb tt0133093   # trust the id; still check the other sources
jack "Dune (1984).mkv" --name "Dune" --year 1984     # name and year only filter the search
jack ~/TV/saiki-k/ --thetvdb https://www.thetvdb.com/series/the-disastrous-life-of-saiki-k
jack --wikidata Q83495 movie.mp4                     # a link or an id; trusted alone
jack --x https://x.com/nasa/status/42 clip.mp4       # status id; yt-dlp reads it when installed
jack --set --tmdb-key KEY --tvdb-key KEY             # several keys; a bad one is named, the good ones are saved
jack Cosmos.mkv --tv                                 # force TV (or --movie / --type auto|tv|movie)
jack movie.mp4 -p                                    # print the facts; Jack always says what he did
```

## Config

- Main config: `"~/Library/Application Support/Windups/jack/config.toml"` on macOS, `~/.local/share/windups/jack/config.toml` on Linux (old installs: `~/.config/windups/jack/config.toml`) or `--config <path>`; deep-merged over the built-in defaults in `config/jack.toml`, which documents every key. Quote the macOS path; it contains a space.
- Embedded ids, names and dates count as flags when you give no id. An id you give outranks the file's tags; tags that disagree with it are reported as "file tags look wrong" and not used. `--name` and `--year` only filter.
- When Jack can't decide, he lists the candidates once, grouped by source. Pick at most one per source by number, at the prompt or with `--pick "1 6"`. Picks count as confirming sources.
- A title is accepted only when it is the single result at or above `[matching] fuzzy_threshold` (default 0.90). The score is stored with the decision.
- When Jack cannot decide and Sammy is not connected, a person is asked `Would you like to see candidates (y/n)?`. A pipe or another windup gets exit code 1 and the reason.
- Precedence: flags > main config > built-in defaults. `~` is expanded in paths.
- yt-dlp and exiftool are optional. Jack uses them when they are installed (YouTube prefers its API when a key is set; X has no free read API). A missing tool is a clear message, not a failure by itself.
- Jack writes nothing beside the media. Every record goes into `jack.db` first.

Packaging: see `packaging/homebrew/` (mirror of the `am517/tap` formula).

OMDb (optional, needs a key) adds ratings, rated and awards. Without a key it is skipped with a warning.

- `[providers] language = "en"` sets the metadata language.
- `keep_original = true` also stores original-language names.
- Descriptions: the longest one wins (`[merge] longest`) until Sammy picks.
- Genres are merged into one string (`genre_case`, `genre_separator`).
- `[matching] fuzzy_threshold` is the title score Jack requires (default 0.90).

### Records, sync and the universal commands

Jack's folder holds three things: the `jack` binary, his own database `jack.db`, and his config file. Every record Jack makes is written to `jack.db` first, as handoff JSON v1 (`"v": 1`; see `../windups/schema/handoff_v1.md`) under a stable id; that is his archive and source of truth. `[archive] database` in `config/jack.toml` moves it (empty = beside the config file).

- Each record carries `synced_at`: empty until the sync target acknowledges it. At the end of every run Jack syncs to Daniel, the record keeper (`[archive] sync_target = "daniel"`; `sync_transport = "library"` calls `daniel::receive` in-process, `"socket"` talks to a running `daniel serve` over `daniel_socket`, see `../windups/COMMANDS.md`), in batches with one result per record; a record is marked synced only after its ack, so a retry is safe. Jack keeps his records after syncing (on purpose). If media.db can't be opened, records just wait in `jack.db`. `sync_target = "none"` (or `[store] enabled = false`) keeps them local. No `<file>.jack.json` is written beside the media (`[store] handoff_json` and `fallback_suffix` are ignored).
- `[archive] id`: `content` (default; an identical re-run is the same record) or `per_run`.
- Universal commands (see `../windups/COMMANDS.md`): `jack status|health|sync|backup|export [-|DIR]|version|config|log [N]|help|talk WORDS...`, plus Jack's own `identify PATH...`, `ask` and `setup`. A real file or folder with one of those names is still treated as a path. `jack export` writes every record (JSONL).
- Jack's folder: see `../windups/LAYOUT.md` (`--config` > `JACK_HOME` > the binary's folder > the data folder > legacy `~/.config/windups/jack/`). The first run makes the data folder and links `~/.local/bin/jack`; snapshots go in `<folder>/backups/`.
- `-j` / `--json` prints the run's result as one JSON object on stdout (`{"v":1,"ok":..,"items":[{file, ok, lifecycle, reason, error, metadata}]}`); add `-p` for the facts in `metadata`. No prompts, no commentary. `--emit-json`, `--print-format` and the `-j` sidecar are gone; `jack export [-|DIR]` writes the kept records.

## API keys

Keys come from the config file first, then from project keys embedded at build time:

```zsh
JACK_TMDB_TOKEN=... JACK_TVDB_KEY=... JACK_OMDB_KEY=... cargo build --release
```

Per run, `--tmdb-key` (alias `--tmdb-token`), `--tvdb-key`, `--tvdb-pin` and `--omdb-key` beat both. The value is required (`--tvdb-key <KEY>`). `--tmdb-key` takes either TMDB credential: a v3 key (32 hex characters) or a read-access token (a JWT). Several keys can be set in one command. Jack saves the ones that check out and says which did not, for example `I saved the TheTVDB key, but the TMDB key didn't work because it is not a 32-character hex key or a read-access token.` He never prints the key. With `--set` he saves and does not scan.

Config `[providers.tmdb] read_token`/`api_key` `[providers.thetvdb] api_key` and `[providers.omdb] api_key` override embedded keys. The TheTVDB subscriber `pin` is config-only. With neither, Jack reports which key to set. Never commit keys (`.env`, `*.key` and `secrets*` are git-ignored). Keys embedded in a binary can be extracted from it.

## Attribution

- Metadata provided by TheTVDB: https://thetvdb.com
- This product uses the TMDB API but is not endorsed or certified by TMDB: https://www.themoviedb.org

- Ratings, rated and awards from OMDb (The Open Movie Database): https://www.omdbapi.com (courtesy credit; OMDb's terms require none, but limit use to personal, non-commercial purposes)

These lines are also shown in `jack --help`.

## License

MIT, see [LICENSE](LICENSE).
