# Universal commands

Every windup answers these, so one client can talk to any windup: `<windup> <command> [args]`.
They live in the shared core (`windups::base::commands`); a windup adds only what is special to
it (its status lines, how it syncs and exports, and its own actions). This is a living list:
when a command is added or changes, update this file in the same commit (`base::commands` tests
fail if a universal command is missing here).

| Command | What it does | Notes |
| --- | --- | --- |
| `status` | What I hold and how far along I am | Core: schema version, recent events, last snapshot; plus the windup's own lines |
| `health` | Integrity check, schema version, backup age | `PRAGMA quick_check`; stale after `[core.snapshots] stale_after_days` |
| `sync` | Send records the sync target doesn't have yet | Target chosen in config; marked synced only on the target's ack |
| `backup` | Take a checked snapshot of my database | Online backup API + `integrity_check` on the copy; then event-log compaction |
| `export` | Write my records out: `export [-\|DIR]` | `-` = stdout (JSONL); DIR = one file |
| `version` | My version and schema version | |
| `config` | Where my config is and what it says | Secret values (keys, tokens, PINs, passwords) are always hidden |
| `log` | Recent events: `log [N]` (default 20) | From the append-only event log |
| `help` | This list, plus what only I can do | |
| `talk` | Plain words to a real command: `talk WORDS...` | Keyword matcher; says what it ran; unknown words go to a helper (later Flora or a local model) |

## Windup-specific actions

| Windup | Action | What it does |
| --- | --- | --- |
| Jack | `tag` | Fetch, verify and tag files: same as `jack PATH...` (talk words: tag, label, identify, metadata) |

## Hooks for later

- `talk` helper: `base::talk::Helper` (Flora, or a local model on small devices).
- Off-device snapshot copies: `base::snapshot::OffDevice` (Walter or a later windup).
- Sync transport: `base::sync::SyncTarget` (library call today; a socket later).
- Audit: `base::audit::audit` + `Check` (Walter reads any windup's DB, read-only).
