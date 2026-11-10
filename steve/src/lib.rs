//! Steve (search), stub. Steve searches the database (genres, people, dates, full text, meaning; graph later).
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"steve\""));
    }
}
