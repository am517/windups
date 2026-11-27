//! George (backups), stub. George receives each windup's checked snapshots and organizes and cares for them outside the windup folders.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"george\""));
    }
}
