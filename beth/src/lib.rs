//! Beth (lora-trainer), stub. Beth trains LoRAs from each windup's training phrases and parameters (later; until then LoRAs are made manually).
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"beth\""));
    }
}
