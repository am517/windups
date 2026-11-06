//! The handoff to Daniel, the database windup: a windup that has fetched and verified what it can
//! about one file hands over one entry; Daniel alone writes it to the shared database (and asks
//! Larry, Tom, Mary or Kate for what they add). Windups call windups; nothing else schedules work.
//!
//! On disk this is one versioned JSON object per entry (`"v": 1`). See `schema/handoff_v1.md`.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::PathBuf;

pub type Fields = BTreeMap<String, String>;
/// (field, [(source, value)]): sources about the same work that disagree; recorded, not settled.
pub type Conflict = (String, Vec<(String, String)>);

/// Current on-disk handoff schema. Unknown versions are rejected.
pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Handoff {
    /// The file the entry is about.
    pub file: PathBuf,
    /// The chosen, verified entry (including `lifecycle` and the reasons for it).
    pub entry: Fields,
    /// Every source's answer, kept whole (source name, its fields).
    pub candidates: Vec<(String, Fields)>,
    /// Disagreements the handing windup found between sources (it verifies; Daniel only records).
    pub conflicts: Vec<Conflict>,
    /// Who hands it over (e.g. "jack").
    pub from: String,
    /// Who asked for this run, if a windup did (e.g. "walter"): Daniel doesn't alert Walter about
    /// work Walter himself started (the loop guard).
    pub caller: Option<String>,
}

#[derive(Serialize)]
struct Out<'a> {
    v: u32,
    #[serde(flatten)]
    h: &'a Handoff,
}

#[derive(Deserialize)]
struct In {
    v: Option<u32>,
    #[serde(flatten)]
    h: Handoff,
}

impl Handoff {
    /// Writes `{"v":1, ...fields}` with no extra buffering of the whole document.
    pub fn write_json(&self, w: impl Write) -> Result<(), String> {
        serde_json::to_writer(
            w,
            &Out {
                v: VERSION,
                h: self,
            },
        )
        .map_err(|e| e.to_string())
    }

    /// Parses exactly one handoff object.
    pub fn read_json(r: impl Read) -> Result<Self, String> {
        let mut all = Self::read_json_all(r)?;
        if all.len() == 1 {
            Ok(all.remove(0))
        } else {
            Err(format!("expected one handoff object, found {}", all.len()))
        }
    }

    /// Parses one or more concatenated JSON objects (JSONL or a single object).
    pub fn read_json_all(r: impl Read) -> Result<Vec<Self>, String> {
        let de = serde_json::Deserializer::from_reader(r);
        let mut out = Vec::new();
        for item in de.into_iter::<serde_json::Value>() {
            let v = item.map_err(|e| format!("invalid handoff JSON: {e}"))?;
            out.push(Self::from_json_value(v)?);
        }
        if out.is_empty() {
            return Err("no handoff JSON found".into());
        }
        Ok(out)
    }

    /// Checks `"v"` then deserializes the Handoff fields. Unknown versions are rejected.
    pub fn from_json_value(v: serde_json::Value) -> Result<Self, String> {
        let parsed: In =
            serde_json::from_value(v).map_err(|e| format!("invalid handoff JSON: {e}"))?;
        match parsed.v {
            Some(VERSION) => Ok(parsed.h),
            Some(other) => Err(format!(
                "unsupported handoff version {other} (want {VERSION})"
            )),
            None => Err(format!("handoff JSON missing \"v\" (want {VERSION})")),
        }
    }
}

pub trait Daniel {
    /// Stores the entry; returns a one-line receipt (e.g. "database: media #12 in ...").
    fn accept(&self, h: &Handoff) -> Result<String, String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample() -> Handoff {
        Handoff {
            file: PathBuf::from("/media/The.Matrix.1999.mkv"),
            entry: [("kind", "movie"), ("title", "The Matrix"), ("year", "1999")]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            candidates: vec![(
                "tmdb".into(),
                [("title", "The Matrix")]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            )],
            conflicts: vec![(
                "year".into(),
                vec![
                    ("tmdb".into(), "1999".into()),
                    ("embedded".into(), "1998".into()),
                ],
            )],
            from: "jack".into(),
            caller: None,
        }
    }

    #[test]
    fn round_trips_v1() {
        let h = sample();
        let mut buf = Vec::new();
        h.write_json(&mut buf).unwrap();
        let got = Handoff::read_json_all(&buf[..]).unwrap();
        assert_eq!(got, vec![h]);
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["v"], 1);
        assert_eq!(v["from"], "jack");
        assert_eq!(v["file"], "/media/The.Matrix.1999.mkv");
        assert_eq!(v["entry"]["title"], "The Matrix");
        assert_eq!(v["candidates"][0][0], "tmdb");
        assert_eq!(v["conflicts"][0][0], "year");
    }

    #[test]
    fn rejects_unknown_and_missing_version() {
        let bad = br#"{"v":99,"file":"/a","entry":{},"candidates":[],"conflicts":[],"from":"jack","caller":null}"#;
        let e = Handoff::read_json_all(&bad[..]).unwrap_err();
        assert!(e.contains("unsupported handoff version 99"), "{e}");
        let missing = br#"{"file":"/a","entry":{},"candidates":[],"conflicts":[],"from":"jack","caller":null}"#;
        let e = Handoff::read_json_all(&missing[..]).unwrap_err();
        assert!(e.contains("missing \"v\""), "{e}");
    }

    #[test]
    fn reads_concatenated_objects() {
        let h = sample();
        let mut buf = Vec::new();
        h.write_json(&mut buf).unwrap();
        buf.push(b'\n');
        h.write_json(&mut buf).unwrap();
        assert_eq!(Handoff::read_json_all(Cursor::new(buf)).unwrap().len(), 2);
    }
}
