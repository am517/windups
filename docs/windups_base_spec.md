# Windups base spec (`windups::base`)

The shared core every windup builds on. Rules and reasons are in [coding_guidelines.md](coding_guidelines.md); keys, phrase, trust and Noise are in [identity_and_security.md](identity_and_security.md). This spec lists only what the base provides. Per-windup status lives in each windup's own spec (e.g. [jack_spec.md](jack_spec.md)).

## 1. Global flags
The syntax is the same on every windup. Each windup implements it compatibly, and a windup may leave out a flag it has no use for. Users never learn different flags for the same idea.

| Flag | Meaning |
|---|---|
| `-h, --help` | Usage, the commands, and what I can and can't do (absorbs `--capabilities`). `-h -j` gives the same as JSON. |
| `-V, --version` | Version |
| `-j, --json` | Result as JSON on stdout, for people and scripts (replaces `--emit-json` and `--print-format json`) |
| `--set` | Save the other flags given as defaults in the windup's settings (each windup decides what that means; e.g. Jack saves ids, keys, defaults) |
| `--dry-run` | Change nothing; say what would happen. Decided (Nov 27): every windup has it, and the dry run is recorded on the task |
| `--no-X` | Off form of every on/off flag; last one wins |

Removed: `-w/--windup` (windups talk to each other directly over Noise, never through the command line), `--caller` (Noise identifies the caller), `--config PATH` (settings live encrypted in the windup's DB).
Flag form: `--flag value` (space form, shown in help); `--flag=value` may also work; no extra prompt, except a credential flag given without a value, which prompts hidden input, and only when a person is running it.

## 2. Commands
Plain-word commands, the same set on every windup, listed in `--help`:

| Command | Does |
|---|---|
| `status` | What I'm doing / last run |
| `health` | Self-check (DB integrity, keys unlockable, contacts reachable) |
| `sync` | Send unsynced records to Daniel |
| `backup` | Make a checked snapshot and hand it to George (the backup windup) |
| `export` | Readable copy of my records (never keys) |
| `version` | Version |
| `config` | View or change settings (the only way, besides `--set`; settings are encrypted in the DB) |
| `log` | Recent events |
| `help` | Same as `--help` |
| `talk` | Plain-language request, turned into a real command (via Sammy when available) |
| `setup` | First run: make my folder and DB, identity keys, unlock key; join via Flora |
| `migrate` | Move to a new device: copy the folder, run `migrate`, enter the recovery phrase; the master key is re-derived and cached on the new device |

A windup adds its own commands only for its one job (e.g. Jack's `ask`). A real file or folder with a command's name is still treated as a path.

## 3. Exit codes
0 ok · 1 failed, unresolved, or nothing processed · 2 usage or settings error.

## 4. Output
- Human (default): concise but descriptive. Always say what was done, what was skipped and why, or why nothing was done; never silent. Errors say what failed, why, and what to do next.
- `-j/--json`: JSON on stdout, reasons in their own fields, never inside values; no prompts or commentary.
- With Sammy: fluid, like a normal agent. In a swarm, a windup reports to Flora and Flora tells the user.
- Never print a secret.

## 5. Data, settings, backups (basics)
- Folder named after the windup, holding one binary and one encrypted SQLite DB. Found in this order (decided): (1) the binary's own folder, (2) `<NAME>_HOME` (e.g. `JACK_HOME`; one variable per windup, already used in the code) for unusual setups, (3) the default data root. Default root: `~/Library/Application Support/Windups/<name>` (macOS), `$XDG_DATA_HOME/windups/<name>` or `~/.local/share/windups/<name>` (Linux). Folders can be moved.
- All settings, keys and relationships are encrypted in the DB; seen or changed only through `config` / `--set`.
- Standard tables in every windup's DB (encrypted values, seen only through `config` / `--set`):

| Table | Columns |
|---|---|
| `settings` | one row per setting: key, value |
| `relationships` | role, windup name, public key, last seen |
| `secrets` | name, encrypted value, nonce, created, last verified |

  A windup adds its own tables only for its job (e.g. Jack's `records`).
- Own records first, kept after sync; synced to Daniel with stable ids and per-record acks.
- Append-only event log; schema version with ordered, transactional forward migrations.
- Backups: the windup makes its own checked snapshots and hands them to George, the backup windup, stored outside the folder.
- Outside the folder: only backups and the unlock key (see identity_and_security.md).

## 5a. Sammy's part in vectorizing (PROPOSED, 2026-11-27)
Sammy has no spec yet, so this lives here. Sammy writes a title and summary for each segment and
for each whole work; Tom vectorizes them alongside the original passages, never instead of them,
and never vectorizes a Sammy rewrite of the text itself. A vision model (through Sammy) writes
photo and video-frame descriptions. Details: `docs/tom_spec.md`, "Vectorizing strategy".

## 5b. Mention flow (PROPOSED, 2026-11-27)
A mention goes to Jack (metadata only, regardless of ownership) → Jack returns results to Daniel →
Daniel activates the other windups for refinements. For works not obtained or paywalled, Larry and
Tom wait; Charles obtains on demand. Details: daniel_spec §8a.

## 6. Base modules (today, `windups/src/base/`)
`layout`, `install`, `schema`, `events`, `snapshot`, `records`, `sync`, `sync_records`, `sync_socket`, `audit`, `commands`, `talk`, `migrate`, `relocate`, `config`, `relationships`, `personality`, `flags`, `help`, `wire`. To add: identity/keys, recovery phrase, Noise transport, encrypted settings, backup hand-off, multi-device (later). Function-level detail (inputs, outputs, errors, tests) comes next.
