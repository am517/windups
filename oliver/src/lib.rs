//! Oliver (matchmaker), stub. Oliver breaks messy requests down, decides which windups team up, and asks Ben to fetch and install a missing windup.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"oliver\""));
    }
}
