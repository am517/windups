# Universal commands

Every windup answers these, so one client can talk to any windup: `<windup> <command> [args]`.
Answering them is one of the society rules in [`CULTURE.md`](CULTURE.md). They live in the shared core (`windups::base::commands`); a windup adds only what is special to
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
| Jack | `setup` | Make `~/windups/jack/` and the `~/.local/bin/jack` link (the first run does it too) |
| Walter | `audit` | Read every windup's DB read-only and print each finding with the fix to run (talk: audit, check on everyone, gaps) |
| Daniel | `serve` | `daniel serve [--socket PATH] [--db PATH]`: answer sync batches on a Unix socket (below) |

`walter audit` scans `[audit] roots` (default `~/windups`, `~/.config/windups`) for
`<name>/<name>.db`, opens each read-only and prints:

```text
audited 3 windup(s): 2 finding(s)
  jack: 2 record(s) not yet synced -> run: jack sync
  jack: no backup yet -> run: jack backup
  all good: larry
```

He writes only his own `walter.db` (event log); he never fixes another windup's data himself.

## Sync socket protocol (Daniel, `base::wire`)

Unix domain socket (`[serve] socket`, default `~/windups/daniel/daniel.sock`), std only. Each frame
is a 4-byte big-endian length then that many bytes of UTF-8 JSON; frames over `max_frame_bytes`
(16 MiB) are refused. Any number of requests per connection, one response each, in order:

```text
request   {"v":1,"op":"sync","records":[{"id":"<stable id>","record":"<handoff JSON>"}, ...]}
response  {"v":1,"results":[{"id":"<id>","ok":true,"detail":""}, ...]}   one per record, same order
          {"v":1,"error":"<why>"}                                          whole batch failed
```

`ok:false` (with `detail`) is a rejected record; the sender keeps it and leaves it unsynced. An
`error` response, a timeout or a dropped connection means "unreachable": nothing is marked synced
and everything is resent later. Daniel's receive is idempotent, so a resend after a lost ack adds
nothing. Jack picks the transport with `[archive] sync_transport = "library" | "socket"`.

## Hooks for later

- `talk` helper: `base::talk::Helper` (Flora, or a local model on small devices).
- Off-device snapshot copies: `base::snapshot::OffDevice`, unassigned: a handoff for a future backup
  windup. Walter only flags stale or missing copies.
- Sync transport: `base::sync::SyncTarget`: a library call to Daniel, or `base::sync_socket::SocketTarget`
  (the protocol above). Daniel answers per batch with one result per record (`send_batch`).
- Audit: `base::audit::audit` + `Check`, used by `walter audit` (read-only). Walter's regular
  schedule is not decided yet; for now he audits when asked and when Daniel alerts him.
- Personality: `base::personality::prompt_line` builds "Your name is X and you are <Trait>. Your job is Y."
