# Windup folder layout

A windup folder holds three things, together:

```text
<folder>/
  <name>          the binary
  <name>.db       its one SQLite database (archive, event log, snapshot list)
  config.toml     its config, editable by the user or the windup
  backups/        its checked snapshots (the windup's own copies)
```

A windup is responsible only for its own folder (decided by Adam, Nov 10, 2026), so its snapshots
live inside it: `[core.snapshots] dir`, default `<folder>/backups/`.
Copies off the device are not any current windup's job: the `OffDevice` hook stays unassigned, a
handoff for a future backup windup. Walter only flags stale or missing copies (`walter audit`).

`~/.local/bin/<name>` is a symlink to `<folder>/<name>`.

## Where the folder is

The default data folder depends on the operating system. One directory holds every windup, and each
windup gets a subfolder:

| System | Data root | Example for Jack |
| --- | --- | --- |
| macOS | `~/Library/Application Support/Windups` | `"~/Library/Application Support/Windups/jack"` |
| Linux | `$XDG_DATA_HOME/windups`, or `~/.local/share/windups` when `XDG_DATA_HOME` is unset | `~/.local/share/windups/jack` |

The macOS path contains a space (`Application Support`). Build it with path APIs. When a shell
command or a symlink is written by hand, quote it:
`"~/Library/Application Support/Windups/jack"`.

There is no automatic move from an old `~/windups/<name>/`. That location is not created anymore.
Point `--config` or `<NAME>_HOME` at a folder you still have.

## How a windup finds its folder (`windups::base::layout`)

1. `--config PATH`: the folder is that file's folder (the DB sits beside it).
2. `<NAME>_HOME` (e.g. `JACK_HOME=/srv/jack`).
3. The binary's own folder, if it already holds `config.toml` or `<name>.db`. `current_exe()`
   follows symlinks on Linux, so `/usr/bin/jack -> ".../Windups/jack/jack"` finds that folder.
4. The data folder above, if it exists.
5. Legacy split install: `~/.config/windups/<name>/`, if it holds a config or DB and the data
   folder does not exist yet. Old installs keep working and are never moved automatically;
   `<name> migrate` moves one when asked (copy, verify, switch; the old folder stays, with a
   `MIGRATED.txt`; see [COMMANDS.md](COMMANDS.md)).
6. Otherwise the data folder, which the first run sets up (below).

Inside the config, `[archive] database` (Jack) can still move the DB.

## First run and packaging

Debian packages must not write into users' home folders (maintainer scripts run as root, once, for
every user), so a .deb can't make the data folder. The clean equivalent:

- The .deb (and Homebrew) ship only the binary (`/usr/bin/jack`) and the example config
  (`/usr/share/windups-jack/config.toml.example`).
- The first run as each user, when rule 6 picked the data folder, makes that folder, links
  `<folder>/<name>` to the real binary, writes a short starter `config.toml` (the full defaults
  are built in) and links `~/.local/bin/<name>` to `<folder>/<name>`
  (`windups::base::install::first_run`). `jack setup` does the same on demand. The link is a
  symlink syscall, so the space in `Application Support` stays intact.
- It never overwrites anything: an existing file or a link it didn't make is left alone and said so.
  It does nothing with `--config`, `<NAME>_HOME`, a binary-folder install or a legacy folder.
- A per-user tarball install is the folder itself: unpack into the data folder and link
  `~/.local/bin/<name>`.

## How windups behave

These rules apply to every windup, not only Jack.

### Flags

Prefer `--flag value` (a space). That form must work with no extra prompt. `--flag=value` may also
work. Help text and docs show the space form. A credential flag with no value still prompts
(hidden), and only when a person is running it.

### How windups talk

- **Human mode** (the default, a person at the terminal) errs on the verbose side. That is not a
  wall of text. It always says what took place: what it did, what it skipped and why, or why it
  did nothing. A windup must never finish silently. If nothing was processed, the exit code is
  nonzero (1). Usage and config errors stay exit code 2.
- **Windup mode** (`-w` / `--windup`) is how one windup calls another. Beyond the exit code and the
  returned item, it is quiet: no prompts, no menus, no human commentary. Stdout is structured data
  (JSON) with reasons in their own fields, never inside a value. `jack --capabilities` is prose;
  `jack -w --capabilities` is that JSON.
- Windups may later talk to each other in natural language through Sammy (the LLM windup). That
  is not built. Human text and the machine result stay on separate channels so a language layer
  can sit beside them without changing either one.

### What `writes` means

A profile's `writes` list is the set of things that windup itself changes, so others know its side
effects. It is not a dump of another windup's tables. Jack writes `jack.db`, handoff records
(Daniel stores the shared catalog), file tags, and sorted folders.
