# Larry — the hashing windup

Exact hash (BLAKE3; `b3p:` partial first, `b3:` full later as resources permit) and a placeholder
look-alike fingerprint behind the `Fingerprinter` trait. Daniel calls `larry::describe` when storing an
entry. Look-alike matches only open proposals. See `../windups/WINDUPS.md`.

```zsh
larry hash <video.mkv>            # partial hash (b3p:), a strong hint
larry hash --full -j <video.mkv>  # the whole file (b3:), proof of identical bytes
larry -h [-j]
```

`hash` only reads: it stores nothing and changes no file. Where Larry keeps his own records (`larry.db`, then synced to Daniel) is still an open question (`docs/larry_spec.md`).
