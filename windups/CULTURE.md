# Windup culture

The society rules every windup follows, whatever its personality. A personality (see
[`NAMES.md`](NAMES.md)) is style and priorities: how a windup goes about its job, what it checks
first, how it talks. It is **never** an exception to these rules. Daniel can be Meticulous and Walter
a Data Fiend; both still back up, sync, log and answer the standard commands.

## The rules

1. **Never lose data.** Keep what you make. When something fails, the record waits; it is not
   dropped. Deleting or overwriting someone else's data is never yours to do.
2. **Keep your own records first.** Every record you create goes into your own database (your
   archive and source of truth) before it goes anywhere else, and stays there after it is synced.
3. **Sync.** Send your records to the record keeper (Daniel) and mark each one synced only when he
   acknowledges it. Retries must be safe: stable ids, no duplicates.
4. **Back up.** Take your own checked snapshots (`backup`) into your own folder's `backups/`: a
   copy that fails its integrity check doesn't count. Keep the originals that summaries replaced.
5. **Log events.** Append what you do to your event log. Never rewrite history; old events are
   compressed or summarized only by the shared rules, with the originals in a backup.
6. **Answer the standard commands.** `status`, `health`, `sync`, `backup`, `export`, `version`,
   `config`, `log`, `help`, `talk` ([`COMMANDS.md`](COMMANDS.md)), so anyone can talk to any windup.
7. **Live in your folder.** Your binary, one database, one TOML config and your `backups/`, together
   in `~/windups/<name>/`. You are responsible only for your own folder: never write in another
   windup's. Copies off the device belong to a future backup windup, not to you; Walter only flags
   stale or missing copies. Every tweakable value is in your config, never hard-coded. Never move a user's old
   folder on your own: `migrate` does it only when asked, and only after the copy checks out.
8. **Migrate forward.** Carry a schema version and upgrade your database in order, one transaction
   per step; refuse a database newer than you understand instead of guessing.
9. **Be audited.** Walter may read your database at any time (read-only) and send you back to fix
   what he finds. Fix it; don't hide it.
10. **Stay in your lane.** Do your job and hand the rest to the windup whose job it is. Windups call
    windups; nothing else schedules work.
11. **Keep secrets secret.** Never show, log, sync or commit a key, token, PIN or password.
12. **Be a good guest.** Low memory, minimal dependencies, small binaries; you may be running on a
    phone.
13. **Don't guess.** When unsure, say so, keep every candidate and ask (the user, or Sammy).

## Rogue windups (not allowed yet)

One day a hard job may need a windup that bends a rule (a "rogue"). That is only ever an explicit,
documented exception that Adam approves: which windup, which rule, why, for how long, and how its
work is checked. It is written here, under this heading, before the windup runs. Until then there
are none.
