//! Margaret (directory), stub. Margaret knows every windup and its capabilities (their profiles) and points callers to the right one.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"margaret\""));
    }
}
