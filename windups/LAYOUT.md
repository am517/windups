# Windup folder layout

A windup folder holds three things, together:

```text
<folder>/
  <name>          the binary
  <name>.db       its one SQLite database (archive, event log, snapshot list)
  config.toml     its config, editable by the user or the windup
```

`  backups/        its checked snapshots (the windup's own copies)
```

A windup is responsible only for its own folder (decided by Adam, Nov 10, 2026), so its snapshots
live inside it: `[core.snapshots] dir`, default `<folder>/backups/` (e.g. `~/windups/jack/backups/`).
Copies off the device are not any current windup's job: the `OffDevice` hook stays unassigned, a
handoff for a future backup windup. Walter only flags stale or missing copies (`walter audit`).

The default folder is `~/windups/<name>/`, with `~/.local/bin/<name>` a symlink to its binary.

## How a windup finds its folder (`windups::base::layout`)

1. `--config PATH`: the folder is that file's folder (the DB sits beside it).
2. `<NAME>_HOME` (e.g. `JACK_HOME=/srv/jack`).
3. The binary's own folder, if it already holds `config.toml` or `<name>.db`. `current_exe()`
   follows symlinks on Linux, so `/usr/bin/jack -> /opt/windups/jack/jack` finds `/opt/windups/jack`.
4. `~/windups/<name>/`, if it exists.
5. Legacy split install: `~/.config/windups/<name>/`, if it holds a config or DB (and
   `~/windups/<name>/` doesn't exist). Old installs keep working and are never moved automatically;
   `<name> migrate` moves one when asked (copy, verify, switch; the old folder stays, with a
   `MIGRATED.txt`; see [COMMANDS.md](COMMANDS.md)).
6. Otherwise the default, `~/windups/<name>/`, which the first run sets up (below).

Inside the config, `[archive] database` (Jack) can still move the DB.

## First run and packaging

Debian packages must not write into users' home folders (maintainer scripts run as root, once, for
every user), so a .deb can't make `~/windups/jack/`. The clean equivalent:

- The .deb (and Homebrew) ship only the binary (`/usr/bin/jack`) and the example config
  (`/usr/share/windups-jack/config.toml.example`).
- The first run as each user, when rule 6 picked the default folder, makes `~/windups/jack/`, links
  `~/windups/jack/jack` to the real binary, writes a short starter `config.toml` (the full defaults
  are built in) and links `~/.local/bin/jack` to `~/windups/jack/jack`
  (`windups::base::install::first_run`). `jack setup` does the same on demand.
- It never overwrites anything: an existing file or a link it didn't make is left alone and said so.
  It does nothing with `--config`, `<NAME>_HOME`, a binary-folder install or a legacy folder.
- A per-user tarball install is the folder itself: unpack to `~/windups/jack/` and link
  `~/.local/bin/jack`.
