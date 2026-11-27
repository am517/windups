# Charles — functional spec (PROPOSED stub)
Charles is the download windup. Everything here is **PROPOSED** (2026-11-27); nothing is built.

## 1. Purpose
- Sources and obtains material from archive.org, arXiv, PubMed Central, the Library of Congress and
  similar, with open-source tools, plus an optional internet search.
- Workspace: own folder, one binary, one encrypted DB (coding_guidelines §4).

## 2. Rules
- On demand only: nothing is downloaded until it is needed. Each source's terms are respected.
- He obtains; he doesn't identify. Metadata comes from Jack through Daniel (daniel_spec §8a).
- **Ingest (PROPOSED):** an obtained item goes into Daniel's storage and database first, before
  anything else; all first contact goes to Daniel's database. That triggers Jack and the others:
  metadata (Jack), hashing (Larry), vectors (Tom), and renaming and moving to a logical folder (the
  file-writing windups). Daniel upgrades the `not_obtained` record in place. A `paywalled` item is
  left as it is. George only backs up; he never receives ingested items.
- For reference trees he fetches only the open copies (daniel_spec, "Reference trees").

## 3. Warrants (PROPOSED)
An item is fetched only while it has an outstanding **warrant**. The warrant's source is recorded on
the task. A warrant is one of:
- a **project** that calls for it (for example, research for a writing project: get and index the
  works it mentions);
- a **standing user rule** (for example, always retrieve the mentions tied to a named topic);
- a **direct user request**;
- **another windup** working on a project that would benefit.

Each warrant carries its own **end rule**: never (indefinite), at project end, a time limit (for
example one month), a count (for example until 5 are obtained), and so on. If the rule is ambiguous,
the warrant does not end.

## 4. Settings (PROPOSED)
- Download location: set by the user; default inside Charles's own folder. Once Daniel has
  ingested an item, it moves out of that folder promptly.
- Tools: yt-dlp, aria2, wget and curl, plus the internetarchive `ia` CLI and rclone (suggested).
  The most efficient tool available is preferred; each is optional and detected at runtime.
- Search: **OPEN** (the owner is undecided; resource use matters). SearXNG is fairly light (a small
  Python service); driving a full browser per query is heavier. Suggested: use SearXNG when it is
  installed, with the user's own browser (for signed-in sites) as the fallback.

## 5. Open questions
1. **Search** (§4): confirm SearXNG-if-installed with the browser as fallback, or choose otherwise.
