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

## 3. Warrants (PROPOSED)
An item is fetched only while it has an outstanding **warrant**. The warrant's source is recorded on
the task. A warrant is one of:
- a **project** that calls for it (for example, research for a writing project: get and index the
  works it mentions);
- a **standing user rule** (for example, always retrieve the mentions tied to a named topic);
- a **direct user request**;
- **another windup** working on a project that would benefit.

## 4. Settings (PROPOSED)
- Download location: set by the user; default inside Charles's own folder. Once Daniel has
  ingested an item, it moves out of that folder promptly.
- Search: **OPEN** (the owner is undecided). Suggested: a self-hosted SearXNG by default (it
  aggregates several engines), with an option to drive the user's own Chrome for signed-in sites.

## 5. Open questions
1. **Search** (§4): SearXNG, the user's Chrome, both, or something else?
2. **Tools**: which open-source downloaders (for example `ia` for archive.org)?
3. **Where items go after ingest**: into Daniel's storage, or handed to George?
4. **Warrant expiry**: when does a warrant end (project closed, rule removed)?
