# Windups

Small programs that each do one job well and call each other. One Cargo workspace, one folder per windup.

## Origin

The windups come from Adam's view of the mind as a hive of many small selves, each with its own
identity and drive, negotiating influence and woven together into one.

A windup is a small program that does one job well. Windups call each other; a "super" windup
orchestrates others. This file is the contract. Anything shared lives in this repo.

## Cast

The full cast, roles and build order live in [`windups/WINDUPS.md`](windups/WINDUPS.md) and [`windups/TODO.md`](windups/TODO.md). Flora is the receptionist and user-facing talker; Margaret is the directory of windups and capabilities.

## Layout

- `windups/`: the shared crate and the contract (`windups/WINDUPS.md`, `windups/TODO.md`).
- One folder per windup, each with its `PROFILE.toml` (purpose and success checks), README and crate.

Build and test everything with `cargo build --workspace` and `cargo test --workspace`.

## License

MIT, see `LICENSE`.
