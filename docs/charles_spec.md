# Charles — functional spec (PROPOSED stub)
Charles is the download windup. Everything here is **PROPOSED** (2026-11-27); nothing is built.

## 1. Purpose
- Sources and obtains material from archive.org, arXiv, PubMed Central, the Library of Congress and
  similar, with open-source tools, plus an optional internet search.
- Workspace: own folder, one binary, one encrypted DB (coding_guidelines §4).

## 2. Rules
- On demand only: nothing is downloaded until it is needed. Each source's terms are respected.
- He obtains; he doesn't identify. Metadata comes from Jack through Daniel (daniel_spec §8a).
- When he obtains an item, Larry hashes it, Tom vectorizes it, and Daniel upgrades the
  `not_obtained` record in place. A `paywalled` item is left as it is.
- For reference trees he fetches only the open copies (daniel_spec, "Reference trees").

## 3. Open questions
1. **Where downloads live**: his own folder, or handed to Daniel or George?
2. **Internet search**: which engine, and is it on by default?
3. **Who says "needed"**: a person, Sammy, or Daniel's queue?
4. **Tools**: which open-source downloaders (for example `ia` for archive.org)?
