//! Embedded file tags counted as user input, the same as flags.
//! An embedded id is trusted the way an id flag is. A name or a date only fills the search.
use crate::config::Settings;
use crate::meta::Meta;
use crate::online::urls;
use crate::providers::{self, Query};
use crate::source_id;

/// What the file itself said, in the same shape as flags.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Heard {
    pub name: Option<String>,
    pub year: Option<String>,
    pub imdb: Option<String>,
    pub tmdb: Option<String>,
    pub thetvdb: Option<String>,
    pub wikidata: Option<String>,
    pub wikipedia: Option<String>,
    pub youtube: Option<String>,
    pub x: Option<String>,
}

impl Heard {
    #[cfg(test)]
    pub fn trusted_id(&self) -> bool {
        [
            &self.imdb,
            &self.tmdb,
            &self.thetvdb,
            &self.wikidata,
            &self.wikipedia,
            &self.youtube,
            &self.x,
        ]
        .iter()
        .any(|v| v.as_ref().is_some_and(|s| !s.is_empty()))
    }
}

/// Ids, name and date from the file's tags. URLs in any tag are read the same way as a flag.
pub fn hear(tags: &Meta) -> Heard {
    let get = |key: &str| {
        tags.get(key)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let mut heard = Heard {
        name: get("show").or_else(|| get("title")),
        year: get("year").or_else(|| get("date").as_deref().and_then(year_of)),
        imdb: get("imdb_id").and_then(|v| providers::imdb_id(&v)),
        tmdb: get("tmdb_id").or_else(|| get("tmdb_series_id")),
        thetvdb: get("thetvdb_id").or_else(|| get("thetvdb_series_id")),
        wikidata: get("wikidata_id").and_then(|v| source_id::wikidata(&v)),
        wikipedia: get("wikipedia").and_then(|v| source_id::wikipedia(&v)),
        youtube: get("youtube"),
        x: get("x").and_then(|v| source_id::x_status(&v).ok()),
    };
    for value in tags.values() {
        fill_from_text(&mut heard, value);
    }
    heard
}

/// Embedded values fill blanks left by the filename. Callers apply explicit flags afterwards.
pub fn fill(query: &mut Query, heard: &Heard) {
    if let Some(name) = &heard.name {
        query.title = name.clone();
    }
    if let Some(year) = &heard.year {
        query.year = Some(year.clone());
    }
    if query.imdb.is_none() {
        query.imdb = heard.imdb.clone();
    }
    if query.tmdb.is_none() {
        query.tmdb = heard.tmdb.clone();
    }
    if query.thetvdb.is_none() {
        query.thetvdb = heard.thetvdb.clone();
    }
    if query.wikidata.is_none() {
        query.wikidata = heard.wikidata.clone();
    }
    if query.wikipedia.is_none() {
        query.wikipedia = heard.wikipedia.clone();
    }
}

/// An explicit flag wins over the file. Empty flags leave the file's value in place.
pub fn flags_win(query: &mut Query, flags: &Settings) {
    if let Some(name) = flags.name.as_ref().filter(|v| !v.is_empty()) {
        query.title = name.clone();
    }
    if let Some(year) = flags.year.as_ref().filter(|v| !v.is_empty()) {
        query.year = Some(year.clone());
    }
    if let Some(value) = flags.imdb.as_ref().filter(|v| !v.is_empty()) {
        query.imdb = Some(value.clone());
    }
    if let Some(value) = flags.tmdb.as_ref().filter(|v| !v.is_empty()) {
        query.tmdb = Some(value.clone());
    }
    if let Some(value) = flags.thetvdb.as_ref().filter(|v| !v.is_empty()) {
        query.thetvdb = Some(value.clone());
    }
    if let Some(value) = flags.wikidata.as_ref().filter(|v| !v.is_empty()) {
        query.wikidata = source_id::wikidata(value).or_else(|| Some(value.clone()));
    }
    if let Some(value) = flags.wikipedia.as_ref().filter(|v| !v.is_empty()) {
        query.wikipedia = source_id::wikipedia(value).or_else(|| Some(value.clone()));
    }
}

fn fill_from_text(heard: &mut Heard, value: &str) {
    if heard.imdb.is_none() {
        heard.imdb = providers::imdb_id(value);
    }
    if heard.wikidata.is_none() && value.contains("wikidata.org/") {
        heard.wikidata = source_id::wikidata(value);
    }
    if heard.wikipedia.is_none() && value.contains("wikipedia.org/wiki/") {
        heard.wikipedia = source_id::wikipedia(value);
    }
    if let Some((site, id)) = urls::parse(value) {
        match site.as_str() {
            "youtube" if heard.youtube.is_none() => heard.youtube = Some(id),
            "x" if heard.x.is_none() => heard.x = Some(id),
            _ => {}
        }
    }
    if heard.tmdb.is_none() && value.contains("themoviedb.org/") {
        heard.tmdb = digits_after(value, "/movie/").or_else(|| digits_after(value, "/tv/"));
    }
    if heard.thetvdb.is_none() && value.contains("thetvdb.com/") {
        heard.thetvdb = digits_after(value, "/series/");
    }
}

fn digits_after(value: &str, marker: &str) -> Option<String> {
    let rest = value.split(marker).nth(1)?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    (!digits.is_empty()).then_some(digits)
}

fn year_of(date: &str) -> Option<String> {
    let year = date.get(..4)?;
    year.bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| year.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_name_and_date_fill_flags_and_an_id_is_trusted() {
        let tags: Meta = [
            ("title", "Pilot"),
            ("show", "Breaking Bad"),
            ("date", "2008-01-20"),
            ("imdb_id", "tt0903747"),
            ("comment", "https://www.wikidata.org/wiki/Q1079"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let heard = hear(&tags);
        assert_eq!(heard.name.as_deref(), Some("Breaking Bad"));
        assert_eq!(heard.year.as_deref(), Some("2008"));
        assert_eq!(heard.imdb.as_deref(), Some("tt0903747"));
        assert_eq!(heard.wikidata.as_deref(), Some("Q1079"));
        assert!(heard.trusted_id());
        let mut query = Query {
            title: "from the filename".into(),
            ..Default::default()
        };
        fill(&mut query, &heard);
        assert_eq!(query.title, "Breaking Bad");
        assert_eq!(query.year.as_deref(), Some("2008"));
        assert_eq!(query.imdb.as_deref(), Some("tt0903747"));
        let flags = Settings {
            name: Some("Better Call Saul".into()),
            ..Default::default()
        };
        flags_win(&mut query, &flags);
        assert_eq!(query.title, "Better Call Saul");
        assert_eq!(query.imdb.as_deref(), Some("tt0903747"));
    }

    #[test]
    fn a_name_without_an_id_is_not_trusted() {
        let tags: Meta = [("title".into(), "The Matrix".into())].into();
        let heard = hear(&tags);
        assert!(!heard.trusted_id());
        assert_eq!(heard.name.as_deref(), Some("The Matrix"));
    }
}
