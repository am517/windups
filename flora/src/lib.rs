//! Flora (talker), stub. Flora is the user-facing talker (terminal, GUI app, OS assistant) and the receptionist: she understands via Sammy, asks Margaret who can help, hands the work off via Oliver, and reports back.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"flora\""));
    }
}
