-- v2: a proposed entity keeps its kind (person, place, ...), so a reviewer sees what was proposed.
ALTER TABLE people_mentions ADD COLUMN proposed_kind TEXT;
