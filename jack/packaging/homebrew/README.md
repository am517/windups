# Homebrew

`jack.rb` mirrors `Formula/jack.rb` in [am517/homebrew-tap](https://github.com/am517/homebrew-tap).
Jack builds from the windups workspace tarball (`cargo install` with `std_cargo_args(path: "jack")`).

```zsh
brew install am517/tap/jack
```

(Tap-qualified, because homebrew-core's `jack` is the JACK audio server.)

## Embedded API keys (optional)

`jack` reads `JACK_TMDB_TOKEN`, `JACK_TVDB_KEY` and `JACK_OMDB_KEY` at compile time. The formula forwards
`HOMEBREW_JACK_TMDB_TOKEN` / `HOMEBREW_JACK_TVDB_KEY` / `HOMEBREW_JACK_OMDB_KEY` if they're set.
By default no keys are embedded, so put them in Jack's config
(`"~/Library/Application Support/Windups/jack/config.toml"` on macOS,
`~/.local/share/windups/jack/config.toml` on Linux).
Keys embedded in a binary can be extracted, so never ship private keys in a public bottle.

## Releasing a new version

1. Bump `version` in `jack/Cargo.toml`, commit, then `git tag -a vX.Y.Z && git push origin vX.Y.Z`.
2. `curl -sL https://github.com/am517/windups/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256`
3. Update `url` and `sha256` in the tap's `Formula/jack.rb`, copy it here, and update `../aur/jack/PKGBUILD` (`pkgver`, `sha256sums`) and `.SRCINFO`.
4. If brew is available: `brew audit --strict am517/tap/jack && brew test jack`.
