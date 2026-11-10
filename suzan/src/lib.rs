//! Suzan (write-to-file), stub. Suzan writes pending changes to the file; at most one open Suzan task per entry.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"suzan\""));
    }
}
