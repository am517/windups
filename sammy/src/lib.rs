//! Sammy (thinking), stub. Sammy is the shared LLM thinking service (local or remote models): when evidence alone can't settle it, Sammy weighs it and decides.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

/// A question that needs judgment, and the options.
pub trait Judge {
    /// The chosen option index and why, or None when Sammy can't decide either.
    fn decide(&self, question: &str, options: &[String]) -> Option<(usize, String)>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"sammy\""));
    }
}
