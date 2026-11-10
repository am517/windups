//! John (devices), stub. John manages the device group (pair, list and remove devices, keys, trust checks). He comes just before Walter.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"john\""));
    }
}
