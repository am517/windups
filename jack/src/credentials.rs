//! Key flags: a value is required, and a bad key does not throw away the good ones.
use crate::cli::Cli;

pub enum TmdbCredential {
    /// 32 hex characters. Stored as `[providers.tmdb] api_key`.
    ApiKey,
    /// Read-access token, a JWT (`eyJ…`). Stored as `[providers.tmdb] read_token`.
    ReadToken,
}

pub fn classify_tmdb(value: &str) -> Result<TmdbCredential, &'static str> {
    let value = value.trim();
    if value.len() == 32 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(TmdbCredential::ApiKey);
    }
    if is_jwt(value) {
        return Ok(TmdbCredential::ReadToken);
    }
    Err("it is not a 32-character hex key or a read-access token")
}

fn is_jwt(value: &str) -> bool {
    let mut parts = value.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(header), Some(payload), Some(signature), None) => {
            header.starts_with("eyJ")
                && !payload.is_empty()
                && !signature.is_empty()
                && [header, payload, signature].iter().all(|part| {
                    part.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                })
        }
        _ => false,
    }
}

/// One key that passed the check and can be stored.
pub struct Accepted {
    pub label: &'static str,
    pub config_key: &'static str,
    pub value: String,
    /// When this TMDB credential is saved, the other TMDB field is cleared.
    pub clear_key: Option<&'static str>,
}

pub struct Rejected {
    pub label: &'static str,
    pub reason: &'static str,
}

/// Split the key flags on this command into ones to save and ones to explain.
pub fn check(cli: &Cli) -> (Vec<Accepted>, Vec<Rejected>) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    if let Some(value) = &cli.tmdb_key {
        match classify_tmdb(value) {
            Ok(TmdbCredential::ApiKey) => accepted.push(Accepted {
                label: "the TMDB key",
                config_key: "providers.tmdb.api_key",
                value: value.trim().to_string(),
                clear_key: Some("providers.tmdb.read_token"),
            }),
            Ok(TmdbCredential::ReadToken) => accepted.push(Accepted {
                label: "the TMDB key",
                config_key: "providers.tmdb.read_token",
                value: value.trim().to_string(),
                clear_key: Some("providers.tmdb.api_key"),
            }),
            Err(reason) => rejected.push(Rejected {
                label: "the TMDB key",
                reason,
            }),
        }
    }
    for (value, label, key) in [
        (
            &cli.tvdb_key,
            "the TheTVDB key",
            "providers.thetvdb.api_key",
        ),
        (&cli.tvdb_pin, "the TheTVDB PIN", "providers.thetvdb.pin"),
        (&cli.omdb_key, "the OMDb key", "providers.omdb.api_key"),
    ] {
        let Some(value) = value else { continue };
        let value = value.trim();
        if value.is_empty() {
            rejected.push(Rejected {
                label,
                reason: "it was empty",
            });
        } else {
            accepted.push(Accepted {
                label,
                config_key: key,
                value: value.to_string(),
                clear_key: None,
            });
        }
    }
    (accepted, rejected)
}

pub fn any(cli: &Cli) -> bool {
    cli.tmdb_key.is_some()
        || cli.tvdb_key.is_some()
        || cli.tvdb_pin.is_some()
        || cli.omdb_key.is_some()
}

/// "I saved X, but Y didn't work because N." Nothing here is the key itself.
pub fn sentence(saved: &[&str], rejected: &[Rejected]) -> String {
    let saved = join_and(saved);
    let failed_parts: Vec<String> = rejected
        .iter()
        .map(|r| format!("{} didn't work because {}", r.label, r.reason))
        .collect();
    let failed_refs: Vec<&str> = failed_parts.iter().map(String::as_str).collect();
    let failed = join_and(&failed_refs);
    match (saved.is_empty(), failed.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!("I saved {saved}."),
        (true, false) => format!("{}.", capitalize(&failed)),
        (false, false) => format!("I saved {saved}, but {failed}."),
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

fn join_and(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_string(),
        [a, b] => format!("{a} and {b}"),
        [head @ .., last] => format!("{}, and {last}", head.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmdb_key_is_a_v3_key_or_a_read_token() {
        assert!(matches!(
            classify_tmdb("0123456789abcdef0123456789abcdef").unwrap(),
            TmdbCredential::ApiKey
        ));
        assert!(matches!(
            classify_tmdb("ABCDEF0123456789abcdef0123456789").unwrap(),
            TmdbCredential::ApiKey
        ));
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        assert!(matches!(
            classify_tmdb(jwt).unwrap(),
            TmdbCredential::ReadToken
        ));
        assert!(classify_tmdb("too-short").is_err());
        assert!(classify_tmdb("not.a.jwt").is_err());
    }

    #[test]
    fn the_sentence_names_each_key_and_never_the_secret() {
        let text = sentence(
            &["the TheTVDB key"],
            &[Rejected {
                label: "the TMDB key",
                reason: "it is not a 32-character hex key or a read-access token",
            }],
        );
        assert_eq!(
            text,
            "I saved the TheTVDB key, but the TMDB key didn't work because it is not a 32-character hex key or a read-access token."
        );
        assert!(!text.contains("SECRET"));
        assert_eq!(sentence(&["the OMDb key"], &[]), "I saved the OMDb key.");
        let only = sentence(
            &[],
            &[Rejected {
                label: "the TMDB key",
                reason: "it was empty",
            }],
        );
        assert!(only.starts_with("The TMDB key didn't work"), "{only}");
    }
}
