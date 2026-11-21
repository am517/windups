# Windups roster

Drafted Nov 20, 2026 by Grok Bot. Every name and role is taken from the repo's name registry (`windups/NAMES.md` and each `PROFILE.toml`, `main` @ bd74f45), the handoffs, and Adam's notes. No role is invented. The grouping into categories is my proposal, for Adam to adjust. Build position is from NAMES.md (— means outside the numbered sequence).

## Core / infrastructure (the critical windups named in the coding guidelines)
| Name | Role | Personality | Status | Pos. |
|---|---|---|---|---|
| Flora | The only windup that talks to humans when windups work as a swarm; receptionist. The windup tells Flora, and Flora tells the user. She installs other windups and introduces them. | Kindness | stub | 18 |
| Oliver | Matchmaker: breaks requests down and decides which windups team up; tells windups where the database and LLM windups are; asks Ben to fetch a missing windup; revokes compromised keys and runs the 90-day key rotation (identity_and_security.md) | Friendly | stub | 17 |
| Margaret | Directory of windups and their capabilities | Matchmaker | stub | 16 |
| Daniel | Record keeper: owns the shared catalog (media.db), its schema and all writes to it | Meticulous | active | 2 |
| Walter | Auditor: reads every windup's DB read-only and sends windups back to fix gaps, unsynced records and stale backups | Data Fiend | active | 11 |
| Sammy | Shared LLM thinking service: critical thinking, talking with other windups, answering a person's direct requests | Smart | stub | 12 |
| *(backup windup)* | **Name TBD; Adam picks a traditional first name.** Receives each windup's checked snapshots and is in charge of organizing and caring for backups, stored outside the windup folders | TBD | not created | TBD |

## Core support (infrastructure, not in the guidelines list)
| Name | Role | Personality | Status | Pos. |
|---|---|---|---|---|
| Ben | Rebuilds windups for new platforms (cross-compile, package); fetches and installs a windup when Oliver asks | Handy | stub | — |
| John | Device group: pair, list and remove devices, keys, trust checks | Guarded | stub | 10 |
| Rover | Refines other windups within their profiles; proposes and tests, never applies unreviewed | Restless | stub | — |
| Beth | Trains LoRAs from each windup's training phrases and parameters (later) | Empathetic | stub | — |

## Media
| Name | Role | Personality | Status | Pos. |
|---|---|---|---|---|
| Jack | Gets metadata (facts only) for a media file; keeps it in jack.db and syncs to Daniel | Curiosity | active | 1 |
| Lauren | Writes tags and embeds art from the database; gets Jack's file-editing code later | Artistic | stub | 3 |
| Suzan | Writes pending changes to the file; at most one open task per entry | Persistent | stub | 4 |
| Mary | Short summaries of entries; runs before Tom, never blocks Larry | Articulate | stub | 5 |
| Kate | Image conversion: JPEG to embed, JPEG XL to archive | Utilitarian | stub | 6 |
| Tom | Vectors/embeddings for meaning search | Analytical | stub | 7 |
| Larry | Hashing: exact BLAKE3 plus a perceptual fingerprint | Exacting | active (library) | 8 |
| Julie | Verifies entries against evidence; calls Sammy for judgment | Skeptical | stub | 9 |
| Ruth | APA 7 work reference and work key | Academic | active (library) | — |
| Steve | Searches the database (genres, people, dates, full text, meaning) | Eager | stub | 13 |
| Sally | Organizes files (rename, move, sort); per her profile, takes over Jack's sorting | Orderly | stub | 14 |
| Tammy | Super windup: ingests new media by orchestrating the others | Media Buff | stub | 15 |

## Counts and TBDs
- 22 named windups (matching NAMES.md), plus 1 planned and unnamed backup windup = 23.
- TBD: the backup windup's name, personality and build position. Categories are a proposal; Steve, Ruth and Tammy could arguably be "data" rather than "media".
- Not windups: Sampy/Dory (a display model). Unnamed ideas (a refusal monitor, a profile-managing windup) stay out until Adam names them (NAMES.md).
- All personalities: set by Adam Nov 10, "work in progress" (he may switch to Inside Out-style emotion names).
