//! Ben (builds), stub. Ben rebuilds the windups for new platforms, and fetches and installs a missing windup when Oliver asks.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"ben\""));
    }
}
