//! Tammy (ingest), stub. Tammy is the super windup: she ingests new media by orchestrating the others.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"tammy\""));
    }
}
