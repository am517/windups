# Windup folder layout

A windup folder holds three things, together:

```text
<folder>/
  <name>          the binary
  <name>.db       its one SQLite database (archive, event log, snapshot list)
  config.toml     its config, editable by the user or the windup
```

Snapshots go outside the folder so it stays three things: `[core.snapshots] dir`, default
`~/.local/share/windups/backups/<name>/`.

## How a windup finds its folder (`windups::base::layout`)

1. `--config PATH`: the folder is that file's folder (the DB sits beside it).
2. `<NAME>_HOME` (e.g. `JACK_HOME=/srv/jack`).
3. The binary's own folder, if it already holds `config.toml` or `<name>.db`. `current_exe()`
   follows symlinks on Linux, so `/usr/bin/jack -> /opt/windups/jack/jack` finds `/opt/windups/jack`.
4. Legacy split install: `~/.config/windups/<name>/` (config and DB there). This keeps today's
   .deb working unchanged.

Inside the config, `[archive] database` (Jack) can still move the DB.

## Conflict with today's packaging (not changed yet)

The Jack .deb installs the binary to `/usr/bin/jack` and the example config to
`/usr/share/windups-jack/`; the user's config lives in `~/.config/windups/jack/`. So the binary is
not in the windup folder, and `/usr/bin` can't hold a per-user DB anyway (root-owned, shared by
every user). Rule 4 above keeps that install working: the folder is `~/.config/windups/jack/`
(config + `jack.db`), with the binary on PATH.

**Proposal (needs Adam's OK; no packaging change made):**

- *Per-user install (preferred for phones and small devices):* everything in one user-owned folder,
  e.g. `~/windups/jack/{jack,jack.db,config.toml}`, with `~/.local/bin/jack` a symlink to it. Fits
  the rule exactly; Homebrew and a tarball can do this.
- *System .deb:* install the binary to `/opt/windups/jack/jack` with a `/usr/bin/jack` symlink. A
  system folder can't hold per-user data, so on first run the windup creates the user's folder
  (`~/windups/jack/`) and links its binary there, or `JACK_HOME` points at it. Until then rule 4
  applies.
