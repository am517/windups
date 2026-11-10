//! Lauren (tags), stub. Lauren writes tags and embeds art from the database (takes over from Jack; Jack keeps a fallback).
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"lauren\""));
    }
}
