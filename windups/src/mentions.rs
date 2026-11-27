//! The mention extractor interface: what an extractor (Sammy's, later) is given and what it returns.
//! Extracting is the extractor's job; storing is Daniel's. Daniel stores mentions only from an
//! extractor whose quality is `Final` (the quality rule for permanent records).
use serde::{Deserialize, Serialize};

/// `Final`: good enough to write permanent mentions. `Placeholder`: rule-based, partial or a test
/// stand-in; it may preview, never write.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Final,
    Placeholder,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ExtractorInfo {
    pub id: String,
    pub version: String,
    pub quality: Quality,
}

/// An entity already in the people catalog whose name or an alias appears in the chunk.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct KnownEntity {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub aliases: Vec<String>,
}

/// One chunk to read, with its place in the catalogs and the entities it may name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ChunkInput {
    pub catalog: String,
    pub record_id: i64,
    pub chunk_id: Option<i64>,
    pub locator: Option<String>,
    pub language: Option<String>,
    pub text: String,
    pub known: Vec<KnownEntity>,
}

/// What a mention points at: a known entity, or a proposed new one (never created silently).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Entity(i64),
    Proposed { name: String, kind: String },
}

/// A mention found in `ChunkInput::text`: byte offsets, the text as it appears there, and how sure.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FoundMention {
    pub start: usize,
    pub end: usize,
    pub surface: String,
    pub target: Target,
    pub confidence: f64,
    pub needs_review: bool,
}

pub trait MentionExtractor {
    fn info(&self) -> ExtractorInfo;
    fn extract(&self, input: &ChunkInput) -> Result<Vec<FoundMention>, String>;
}

/// Why a found mention can't be stored, if it can't: offsets outside the text or off a character
/// boundary, a surface that isn't the text at those offsets, or a confidence outside 0..=1.
pub fn invalid(input: &ChunkInput, m: &FoundMention) -> Option<&'static str> {
    if !(0.0..=1.0).contains(&m.confidence) {
        return Some("confidence_out_of_range");
    }
    match input.text.get(m.start..m.end) {
        None => Some("offsets_outside_text"),
        Some(s) if s != m.surface => Some("surface_differs_from_text"),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ChunkInput {
        ChunkInput {
            catalog: "media".into(),
            record_id: 1,
            chunk_id: Some(2),
            locator: Some("p. 3".into()),
            language: Some("en".into()),
            text: "Ada met Kyoto. 京都".into(),
            known: vec![],
        }
    }

    fn found(start: usize, end: usize, surface: &str, confidence: f64) -> FoundMention {
        FoundMention {
            start,
            end,
            surface: surface.into(),
            target: Target::Entity(1),
            confidence,
            needs_review: false,
        }
    }

    #[test]
    fn a_found_mention_must_match_the_text_at_its_offsets() {
        let i = input();
        assert_eq!(invalid(&i, &found(0, 3, "Ada", 0.9)), None);
        assert_eq!(invalid(&i, &found(15, 21, "京都", 0.9)), None);
        assert_eq!(
            invalid(&i, &found(0, 3, "Bob", 0.9)),
            Some("surface_differs_from_text")
        );
        assert_eq!(
            invalid(&i, &found(16, 21, "京都", 0.9)),
            Some("offsets_outside_text"),
            "off a character boundary"
        );
        assert_eq!(
            invalid(&i, &found(0, 3, "Ada", 1.5)),
            Some("confidence_out_of_range")
        );
    }

    #[test]
    fn the_interface_round_trips_as_json_for_sammy() {
        let m = FoundMention {
            target: Target::Proposed {
                name: "Kyoto".into(),
                kind: "place".into(),
            },
            ..found(8, 13, "Kyoto", 0.6)
        };
        let s = serde_json::to_string(&(input(), &m)).unwrap();
        let back: (ChunkInput, FoundMention) = serde_json::from_str(&s).unwrap();
        assert_eq!(back.1, m);
        assert_eq!(back.0, input());
        assert!(serde_json::to_string(&Quality::Placeholder).unwrap() == "\"placeholder\"");
    }
}
