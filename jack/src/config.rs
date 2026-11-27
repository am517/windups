//! Config loading: built-in defaults < main config < CLI flags. No sidecars (the database is the source of truth).
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Project keys embedded at build time (never committed). Config values win over these.
pub const EMBED_TMDB_TOKEN: Option<&str> = option_env!("JACK_TMDB_TOKEN");
pub const EMBED_TVDB_KEY: Option<&str> = option_env!("JACK_TVDB_KEY");
pub const EMBED_OMDB_KEY: Option<&str> = option_env!("JACK_OMDB_KEY");

/// Fill empty TMDB/TheTVDB keys from the env vars named in config (config value > env var > embedded key).
pub fn apply_env(p: &mut crate::providers::ProvidersCfg, get: impl Fn(&str) -> Option<String>) {
    if p.omdb.api_key.is_empty() && !p.omdb.api_key_env.is_empty() {
        p.omdb.api_key = get(&p.omdb.api_key_env).unwrap_or_default();
    }
    if p.youtube.api_key.is_empty() && !p.youtube.api_key_env.is_empty() {
        p.youtube.api_key = get(&p.youtube.api_key_env).unwrap_or_default();
    }
    let t = &mut p.tmdb;
    if t.read_token.is_empty() && t.api_key.is_empty() {
        let env = |n: &str| {
            if n.is_empty() {
                String::new()
            } else {
                get(n).unwrap_or_default()
            }
        };
        t.read_token = env(&t.read_token_env);
        t.api_key = env(&t.api_key_env);
    }
    if p.thetvdb.api_key.is_empty() && !p.thetvdb.api_key_env.is_empty() {
        p.thetvdb.api_key = get(&p.thetvdb.api_key_env).unwrap_or_default();
    }
}

/// Fill empty provider keys from embedded build-time keys (config file value > embedded key).
pub fn apply_embedded(
    p: &mut crate::providers::ProvidersCfg,
    tmdb: Option<&str>,
    tvdb: Option<&str>,
    omdb: Option<&str>,
) {
    if p.omdb.api_key.is_empty() {
        p.omdb.api_key = omdb.unwrap_or_default().to_string();
    }
    if p.tmdb.read_token.is_empty() && p.tmdb.api_key.is_empty() {
        p.tmdb.read_token = tmdb.unwrap_or_default().to_string();
    }
    if p.thetvdb.api_key.is_empty() {
        p.thetvdb.api_key = tvdb.unwrap_or_default().to_string();
    }
}

pub const DEFAULT_CONFIG: &str = include_str!("../config/jack.toml");

/// Options (main config top level, flags); all optional so layers merge.
#[derive(Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
    pub dry_run: Option<bool>,
    /// Print composite metadata to stdout.
    pub print: Option<bool>,
    pub imdb: Option<String>,
    pub thetvdb: Option<String>,
    pub tmdb: Option<String>,
    pub youtube: Option<String>,
    pub x: Option<String>,
    pub wikidata: Option<String>,
    pub wikipedia: Option<String>,
    /// Title override (narrows the filename guess; used as the search query).
    pub name: Option<String>,
    /// Year override (search filter).
    pub year: Option<String>,
    /// "tv", "movie" or "auto": forces the matcher and providers.
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

impl Settings {
    /// The user gave an id flag. A given id outranks everything the file's own tags say.
    pub fn has_id(&self) -> bool {
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
        .any(|v| v.as_ref().is_some_and(|v| !v.is_empty()))
    }
    /// Fill unset fields of `self` from `lower`.
    pub fn over(self, lower: &Settings) -> Settings {
        let l = lower.clone();
        Settings {
            extensions: self.extensions.or(l.extensions),
            recursive: self.recursive.or(l.recursive),
            dry_run: self.dry_run.or(l.dry_run),
            print: self.print.or(l.print),
            imdb: self.imdb.or(l.imdb),
            thetvdb: self.thetvdb.or(l.thetvdb),
            tmdb: self.tmdb.or(l.tmdb),
            youtube: self.youtube.or(l.youtube),
            x: self.x.or(l.x),
            wikidata: self.wikidata.or(l.wikidata),
            wikipedia: self.wikipedia.or(l.wikipedia),
            name: self.name.or(l.name),
            year: self.year.or(l.year),
            kind: self.kind.or(l.kind),
        }
    }
    pub fn flag(v: Option<bool>) -> bool {
        v.unwrap_or(false)
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub base: Settings,
    pub providers: crate::providers::ProvidersCfg,
    pub merge: crate::sorter::MergeCfg,
    pub matching: crate::matcher::MatchCfg,
    pub store: daniel::StoreCfg,
    /// Jack's own database (his archive); see archive.rs.
    pub archive: crate::archive::ArchiveCfg,
    /// Who this windup knows. Names only, never hardcoded defaults.
    pub relationships: windups::base::relationships::Relationships,
    /// The shared core: event log limits, snapshots, talk synonyms (`[core]`).
    pub core: windups::base::config::BaseCfg,
    /// `-j` / `--json` (or `-w`): the result as JSON on stdout, no prompts; not a config file key.
    #[serde(skip)]
    pub windup: bool,
    pub online: crate::online::OnlineCfg,
    pub embedded: crate::filemeta::EmbeddedCfg,
    pub candidates: crate::pick::CandidatesCfg,
    /// `--pick N...`: this run's candidate picks; not a config file key.
    #[serde(skip)]
    pub picks: Option<String>,
    pub explain: crate::explain::ExplainCfg,
    pub review: crate::review::ReviewCfg,
    pub completeness: crate::completeness::CompletenessCfg,
}

/// Jack's folder: `--config`, then `JACK_HOME`, then the binary's folder when it already holds
/// `config.toml` or `jack.db`, then the data folder. The legacy `~/.config/windups/jack/` is used
/// only when that is all that exists. See `windups::base::layout`.
pub fn layout(explicit: Option<&Path>) -> windups::base::layout::Layout {
    use windups::base::layout::{Probe, resolve};
    resolve("jack", &Probe::real("jack", explicit))
}

pub fn expand(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match (s.strip_prefix('~'), std::env::var_os("HOME")) {
        (Some(rest), Some(h)) if rest.is_empty() || rest.starts_with('/') => {
            PathBuf::from(format!("{}{rest}", h.to_string_lossy()))
        }
        _ => p.to_path_buf(),
    }
}

fn expand_s(s: &mut String) {
    *s = expand(Path::new(s)).to_string_lossy().into_owned();
}

fn merge(a: &mut toml::Value, b: toml::Value) {
    match (a, b) {
        (toml::Value::Table(a), toml::Value::Table(b)) => {
            for (k, v) in b {
                match a.get_mut(&k) {
                    Some(x) => merge(x, v),
                    None => {
                        a.insert(k, v);
                    }
                }
            }
        }
        (a, b) => *a = b,
    }
}

fn read(p: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(p) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

/// Main config deep-merged over the built-in defaults.
pub fn load(explicit: Option<&Path>) -> Result<Config, String> {
    let mut v: toml::Value =
        toml::from_str(DEFAULT_CONFIG).map_err(|e| format!("built-in config: {e}"))?;
    let path = match explicit {
        Some(p) => Some(expand(p)),
        None => Some(layout(None).config),
    };
    if let Some(p) = &path {
        match read(p)? {
            Some(t) => merge(
                &mut v,
                toml::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?,
            ),
            None if explicit.is_some() => return Err(format!("{}: not found", p.display())),
            None => {}
        }
    }
    let mut c: Config = v.try_into().map_err(|e| format!("config: {e}"))?;
    // tests never touch the real shared media.db (store tests use their own temp DB)
    #[cfg(test)]
    if c.store.database.is_empty() {
        c.store.enabled = false;
    }
    expand_s(&mut c.store.database);
    expand_s(&mut c.archive.database);
    // tests never touch a real jack.db unless they point at their own
    #[cfg(test)]
    if c.archive.database.is_empty() && explicit.is_none() {
        c.archive.database = ":memory:".into();
    }
    if c.archive.database.is_empty() {
        c.archive.database = match &path {
            Some(p) => crate::archive::default_path(p)
                .to_string_lossy()
                .into_owned(),
            None => crate::archive::DB_NAME.into(),
        };
    }
    apply_env(&mut c.providers, |n| {
        std::env::var(n).ok().filter(|v| !v.is_empty())
    });
    apply_embedded(
        &mut c.providers,
        EMBED_TMDB_TOKEN,
        EMBED_TVDB_KEY,
        EMBED_OMDB_KEY,
    );
    windups::base::relationships::migrate_database(&mut c.relationships, &c.archive.sync_target);
    Ok(c)
}

/// Write newly discovered relationship names into the user config, keeping comments.
pub fn save_relationships(path: &Path, changed: &[(String, String)]) -> Result<(), String> {
    use toml_edit::{DocumentMut, Item, Value};
    if changed.is_empty() {
        return Ok(());
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if !doc.contains_key("relationships") {
        doc["relationships"] = Item::Table(toml_edit::Table::new());
    }
    let t = doc["relationships"]
        .as_table_mut()
        .ok_or_else(|| format!("{}: relationships is not a table", path.display()))?;
    for (k, v) in changed {
        t[k.as_str()] = Item::Value(Value::from(v.as_str()));
    }
    std::fs::write(path, doc.to_string()).map_err(|e| format!("{}: {e}", path.display()))
}

/// Effective settings for one file: flags > main config.
pub fn resolve(flags: &Settings, main: &Settings) -> Settings {
    flags.clone().over(main)
}

pub fn has_ext(p: &Path, exts: &[String]) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        exts.iter()
            .any(|x| x.trim_start_matches('.').eq_ignore_ascii_case(e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_sync_target_becomes_relationships_database() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("config.toml");
        std::fs::write(&p, "[archive]\nsync_target = \"daniel\"\n").unwrap();
        let c = load(Some(&p)).unwrap();
        assert_eq!(c.relationships.database, "daniel");
        std::fs::write(
            &p,
            "[relationships]\ndatabase = \"other\"\n[archive]\nsync_target = \"daniel\"\n",
        )
        .unwrap();
        let c = load(Some(&p)).unwrap();
        assert_eq!(
            c.relationships.database, "other",
            "existing relationship wins"
        );
    }

    #[test]
    fn archive_config_loads_with_jack_db_beside_the_config() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("config.toml");
        std::fs::write(&p, "[archive]\nsync_batch = 5\n").unwrap();
        let c = load(Some(&p)).unwrap();
        assert_eq!(c.archive.sync_batch, 5);
        assert_eq!(c.archive.busy_timeout_secs, 5, "built-in default");
        assert_eq!(
            PathBuf::from(&c.archive.database),
            d.path().join(crate::archive::DB_NAME)
        );
        let other = d.path().join("elsewhere.db");
        std::fs::write(
            &p,
            format!("[archive]\ndatabase = {:?}\n", other.to_str().unwrap()),
        )
        .unwrap();
        assert_eq!(
            PathBuf::from(load(Some(&p)).unwrap().archive.database),
            other
        );
    }

    #[test]
    fn config_key_beats_embedded_key() {
        let mut p = crate::providers::ProvidersCfg::default();
        p.thetvdb.api_key = "from-config".into();
        apply_embedded(&mut p, Some("emb-tmdb"), Some("emb-tvdb"), Some("emb-omdb"));
        assert_eq!(p.omdb.api_key, "emb-omdb");
        assert_eq!(p.thetvdb.api_key, "from-config");
        assert_eq!(p.tmdb.read_token, "emb-tmdb");
        let mut p = crate::providers::ProvidersCfg::default();
        p.tmdb.api_key = "v3".into();
        apply_embedded(&mut p, Some("emb-tmdb"), None, None);
        assert_eq!(
            (p.tmdb.read_token.as_str(), p.thetvdb.api_key.as_str()),
            ("", "")
        );
    }

    #[test]
    fn tvdb_key_from_named_env_var_unless_config_has_one() {
        let mut p = crate::providers::ProvidersCfg::default();
        p.thetvdb.api_key_env = "TVDB_ENV".into();
        apply_env(&mut p, |n| {
            (n == "TVDB_ENV").then(|| "from-env".to_string())
        });
        assert_eq!(p.thetvdb.api_key, "from-env");
        p.thetvdb.api_key = "from-config".into();
        apply_env(&mut p, |_| Some("other".into()));
        assert_eq!(p.thetvdb.api_key, "from-config");
        p.omdb.api_key_env = "OM".into();
        p.youtube.api_key_env = "YT".into();
        p.tmdb.read_token_env = "TOK".into();
        p.tmdb.api_key_env = "KEY".into();
        apply_env(&mut p, |n| Some(format!("env-{n}")));
        assert_eq!(
            (p.tmdb.read_token.as_str(), p.tmdb.api_key.as_str()),
            ("env-TOK", "env-KEY")
        );
        assert_eq!(p.omdb.api_key, "env-OM");
        assert_eq!(p.youtube.api_key, "env-YT");
        p.tmdb.read_token = "cfg".into();
        p.tmdb.api_key = String::new();
        apply_env(&mut p, |_| Some("x".into()));
        assert_eq!(
            (p.tmdb.read_token.as_str(), p.tmdb.api_key.as_str()),
            ("cfg", "")
        );
    }
}
