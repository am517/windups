//! Sally (organizer), stub. Sally organizes files (rename, move, sort); she takes over Jack's built-in sorting (Jack keeps a fallback).
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"sally\""));
    }
}
