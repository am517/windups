# PROPOSAL: how windups talk to each other (not implemented)

The asks: all windups can call all windups; a windup knows who to talk to without being told; no
piping; efficiency above all; a record (chain of custody) so tasks never get lost.

## Options

| | How | Efficiency | Chain of custody | Discovery |
|---|---|---|---|---|
| A. Trails of files | drop JSON files in spool folders, others watch | slow (polling/fs events), many files | the files themselves; messy to query | folder names |
| B. Direct calls | run the other windup's binary with a task | fast, no daemon | none unless logged | needs a registry |
| C. Message queue in media.db | `tasks` + `events` tables; windups claim tasks | fast enough (local SQLite, WAL) | built in, queryable | needs a registry |
| D. Broker daemon (NATS, socket server) | publish/subscribe | fastest for heavy traffic | needs extra logging | broker topics |

## Recommendation: B + C ("direct calls, written in the ledger")

1. **Registry (who to talk to).** Each windup answers `<name> --capabilities` with JSON, e.g.
   `{"name":"sammy","version":"0.1.0","can":["choose.description","summarize"]}`. A shared crate
   finds installed windups (`windups-*` on PATH plus Homebrew), caches the answers in a `windups`
   table, and resolves a capability to a binary. Jack asks for `choose.description`, gets Sammy if
   installed, else uses its own fallback (longest summary). Nobody passes names around.
2. **Ledger (chain of custody).** Tables in media.db:
   - `tasks(id, parent_id, root_id, from_windup, to_windup, capability, input_json, status, output_json, error, created_at, updated_at)`
   - `events(id, task_id, windup, at, kind, detail)`, append-only (created, started, delegated, done, failed).
   Every call is a task row; sub-calls get `parent_id`, so a Tammy ingest is one tree you can read
   end to end ("who did what to this file, when, and why").
3. **Call.** The caller inserts the task and runs `<windup> --task <id> --db <path>`; the callee
   reads the input from the row, writes output and status, exits. Big data (art, metadata) is
   already in media.db, so tasks carry ids, not payloads: cheap.
4. **Asynchronous later.** The same rows work as a queue: Tammy can queue tasks and a windup can
   run as `--serve` to claim pending ones. No broker until measurements say we need one.
5. **Shared crate.** `windups` (in this repo) provides the registry, ledger and schema migrations,
   so every windup talks the same way.

Lost tasks: anything `started` with no update past a timeout is visible (and retryable) in the
ledger. Failures stay as rows with errors; nothing is silently dropped.

Oliver (introducer/matchmaker, much later) would add fuzzy matching ("who can do X?") on top of the
registry.

## Questions
See QUESTIONS.md (communication section).
