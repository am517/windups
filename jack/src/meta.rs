//! Metadata as a flat key/value map. Keys: kind (movie|episode), title, year, date, show,
//! season, episode, episode_title, description, genre, poster_url, poster_file, ids
//! (imdb_id, tmdb_id, thetvdb_id, tmdb_series_id, tmdb_episode_id, thetvdb_series_id, thetvdb_episode_id).
use std::collections::BTreeMap;

pub type Meta = BTreeMap<String, String>;
