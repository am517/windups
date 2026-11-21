# Windup identity, secrets and communication

## Summary
- Each windup instance is born with its own identity at install: a keypair, like a passport and a signature.
- Its secrets sit in its own database, locked with its own data key.
- The data key is stored in the database only in wrapped (locked) form; the key that unlocks it never is.
- A small master key in the OS keychain (macOS Keychain, Linux keyring), or in a 0600 key file when there's no keychain, unlocks the windup's memory. The keychain holds only that one key, and a windup's other keys and facts stay in that windup's personal database.
- A copied database (to another Mac, another user, or a backup stolen alone) can't be opened without the master key.
- A windup can migrate (or exist in multiple places at once) with its database by adding its master key to the new device. The user does this by entering the recovery phrase once on that device.

## Pieces
| Piece | What it is | Where it lives |
|---|---|---|
| Recovery phrase (~24 words) | The user's one root secret for all their windups | Shown once at Flora's setup; kept offline by the user; never stored by any windup |
| Identity key (Ed25519) | Who this windup is; signs its records and requests | private half encrypted in its DB; public half shared |
| Talk key (X25519) | Sets up encrypted conversations with other windups | private half encrypted in its DB |
| Device key (Ed25519) | Which device this copy of the windup runs on (later, see Multi-device) | private half encrypted in its DB; signed by the identity key |
| Data key (random 256-bit) | Encrypts the secrets in the DB's `secrets` table (XChaCha20-Poly1305) | wrapped (locked) by the master key, stored in the DB |
| Master key | Unlocks the data key; derived from the recovery phrase and the windup's name/instance | Cached per device: OS keychain entry for this windup, or a 0600 key file when there's no keychain |

Ed25519 for identity and X25519 for key exchange are the same choices WireGuard, Signal and SSH use. Generate both at first run or `setup`. The public-key fingerprint becomes the instance id, e.g. `jack@a1b2c3`.

## Recovery phrase and master keys
- On Flora's setup the user gets one recovery phrase (about 24 words), shown once and kept offline. One phrase covers every windup, so a user with a thousand windups still keeps only one phrase.
- Each windup has its own master key, derived from that phrase plus the windup's name and instance (the same idea as an HD crypto wallet):
  1. The phrase is stretched with Argon2 into a root key (slow on purpose, so guessing is expensive).
  2. Each windup's master key is derived from the root key with HKDF, using the windup's name and instance as the label.
- The derivation only goes one way: a leaked windup master key doesn't expose the phrase or any other windup's key.
- The phrase handling is part of the shared core code in `windups::base`, not per-windup code.
- After derivation, the master key is cached on the device (keychain or key file) and the phrase is forgotten.
- The phrase is needed only to add a new device or to make new keys after a compromise.
- A passkey is possible as a convenience unlock (e.g. Touch ID); decide later. It would unlock the cached key, not be the key itself.

## Protecting the master key
- OS keychain first.
- If there's no keychain (e.g. a headless Pi), use an SSH-style key file: a private key file the windup creates once.
- If it exists, the windup uses it to unlock; if it's missing, the windup says so and won't open his secrets.
- Windup safety comes first. Copying a windup to another computer must not easily copy his unlock key.
- The key file belongs to the windup and is SSH-style, mode 0600.
- The key is stored separately from the windup's binary and data folder at `~/.config/windups/keys/<name>.key`, or in the keychain on both Mac and Linux if it's available.
- The key is excluded from backups of the data folder, and snapshots and `export` never include it.
- The key is not bound to one machine, so that a windup can easily move between one user's various devices.
- The key is bound to the user: whoever holds the recovery phrase can unlock the user's windups on a new device. On a device, keychain and file permissions bind the cached key to the user's login.
- Adding or moving a device = copy the folder, run `migrate`, and enter the phrase once on that device; the windup re-derives its master key and caches it there.
- No email binding: email needs a server, conflicts with windups working offline, and email accounts get hijacked.

## How windups trust each other
- When Flora installs a windup, she records its public key and gives it hers.
- Flora also gives the new windup Oliver's (the matchmaker) key.
- Each windup saves the public keys of its contacts in the `relationships` table of its database (see windups_base_spec.md §5).
- A windup only talks to other windups who match those exact keys (the same idea as SSH `known_hosts`).
- If a key changes without a signed announcement from Oliver (see Revocation), the windup refuses and tells the person.
- A windup signs each record it syncs to Daniel, so Daniel (and Walter) can prove which windup, and later which device, it came from.
- Only Flora or the user can introduce windups - no trust-on-first-use.
- An unknown key gets refused, and the windup tells its user who tried.

## Revocation and rotation
- Oliver (the matchmaker) revokes compromised keys and announces the revocation to all windups, which drop the old key from their contact tables. Flora does this only as a fallback when Oliver is unavailable.
- Oliver runs key rotation every 90 days: each windup's identity, talk and device keys are replaced with fresh random keys, signed with the old ones and announced to its contacts, so they update without a new introduction. No phrase is needed.
- After a compromise, new master keys are re-derived from the phrase (the user enters it once).

## Multi-device (saved for later; part of the global core)
- One windup can run on several devices (e.g. sorting pictures on both a phone and a computer). This is shared core functionality in `windups::base`, built later.
- Each device gets its own device key, signed by the windup's identity key, so Daniel can tell which device did what and a lost device can be revoked alone.
- The copies sync with each other over Noise. Records are append-only and signed, so merging is mostly adding what the other side doesn't have yet.
- Conflict rules (the same file acted on twice, long offline periods) are designed later.
- No blockchain: public chains make the data public, charge fees and add delays. Daniel is the shared record.

## What talking looks like
- All windup-to-windup communication uses Noise, on the same machine too (`snow` crate, using the pattern WireGuard uses).
- Why Noise: both sides prove they hold the exact key they were introduced to, and the whole conversation is encrypted. There are no certificates or certificate authority to run, the code is small, and it matches the identity keys windups already have. TLS would need certificate handling made for websites.
- Same machine: Noise over the Unix socket. OS file permissions stay as an extra layer. (The command line is for humans only.)
- Across machines (and over Tailscale): Noise over TCP.
- Message format unchanged: length-prefixed JSON v1, inside Noise.
- Windups are open about what they're doing, and with Sammy they talk freely to their own user (through Flora when working as a swarm).
- Toward outsiders (anything not introduced by Flora or the user) they're completely buttoned up and aim to reveal nothing.

## Crates (minimal, well-made, hard to reimplement)
`ed25519-dalek`, `x25519-dalek`, `chacha20poly1305`, `argon2` and `hkdf` (RustCrypto), `keyring`, `snow` (Noise, for all windup traffic), `zeroize` (wipes keys from memory), `bip39` (standard recovery-phrase word list). We never write our own cryptography.
