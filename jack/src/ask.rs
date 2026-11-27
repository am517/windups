//! Other windups ask Jack for subjective data he kept in jack.db (`jack ask summaries`,
//! `jack ask images`). The answer is JSON. Per-record filters: `--file` / `--id` or a trailing
//! path or record id.
use crate::archive::{Archive, Unsynced};
use crate::meta::Meta;
use crate::objective::is_subjective;
use std::path::Path;
use windups::handoff::Handoff;

const SUMMARY_KEYS: &[&str] = &[
    "description",
    "summary",
    "overview",
    "plot",
    "short_description",
];

fn is_image_key(k: &str) -> bool {
    let k = k.to_ascii_lowercase();
    k.contains("poster")
        || k.contains("artwork")
        || k.contains("thumbnail")
        || k.contains("backdrop")
        || k.contains("still")
        || k.contains("art_url")
        || k == "image"
        || k.starts_with("image_")
        || k.ends_with("_image")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub file: Option<String>,
    pub id: Option<String>,
}

impl Filter {
    /// Parses `summaries|images [--file PATH] [--id ID] [PATH_OR_ID]`.
    pub fn parse(words: &[String]) -> Result<(Kind, Self), String> {
        let (kind, rest) = words.split_first().ok_or_else(|| {
            "ask what? use: ask summaries|images [--file PATH] [--id ID]".to_string()
        })?;
        let kind = Kind::parse(kind)?;
        let mut f = Filter::default();
        let mut it = rest.iter();
        while let Some(a) = it.next() {
            let s = a.as_str();
            if s == "--file" {
                f.file = Some(it.next().ok_or("--file needs a path")?.clone());
            } else if let Some(v) = s.strip_prefix("--file=") {
                f.file = Some(v.to_string());
            } else if s == "--id" {
                f.id = Some(it.next().ok_or("--id needs a record id")?.clone());
            } else if let Some(v) = s.strip_prefix("--id=") {
                f.id = Some(v.to_string());
            } else if s.starts_with("jack-") {
                f.id = Some(s.to_string());
            } else {
                f.file = Some(s.to_string());
            }
        }
        Ok((kind, f))
    }

    fn matches(&self, id: &str, file: &str) -> bool {
        if let Some(want) = &self.id
            && id != want
        {
            return false;
        }
        if let Some(want) = &self.file
            && file != want
            && Path::new(file) != Path::new(want)
        {
            return false;
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Summaries,
    Images,
}

impl Kind {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "summaries" | "summary" | "descriptions" => Ok(Kind::Summaries),
            "images" | "image" | "posters" | "artwork" => Ok(Kind::Images),
            other => Err(format!("ask {other:?}: use summaries or images")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Kind::Summaries => "summaries",
            Kind::Images => "images",
        }
    }
}

fn collect_summaries(src: &str, m: &Meta, out: &mut Vec<serde_json::Value>) {
    for k in SUMMARY_KEYS {
        if let Some(t) = m.get(*k).filter(|v| !v.trim().is_empty()) {
            out.push(serde_json::json!({"source": src, "field": k, "text": t}));
        }
    }
}

fn collect_images(src: &str, m: &Meta, out: &mut Vec<serde_json::Value>) {
    for (k, v) in m {
        if is_image_key(k) && !v.trim().is_empty() && is_subjective(k) {
            let kind = if k.contains("file") { "file" } else { "url" };
            out.push(serde_json::json!({"source": src, "field": k, kind: v}));
        }
    }
}

fn item(kind: Kind, id: &str, h: &Handoff) -> serde_json::Value {
    let mut values = Vec::new();
    let gather = match kind {
        Kind::Summaries => collect_summaries,
        Kind::Images => collect_images,
    };
    gather("entry", &h.entry, &mut values);
    for (src, m) in &h.candidates {
        gather(src, m, &mut values);
    }
    serde_json::json!({
        "id": id,
        "file": h.file,
        kind.as_str(): values,
    })
}

/// Every matching record, as a v1 JSON object.
pub fn query(archive: &Archive, kind: Kind, filter: &Filter) -> Result<String, String> {
    let mut items = Vec::new();
    let mut after = 0;
    loop {
        let page = archive.page(after, 64)?;
        let Some(last) = page.last() else { break };
        after = last.seq;
        for Unsynced { id, handoff, .. } in page {
            if filter.id.is_some()
                && filter.id.as_deref() != Some(id.as_str())
                && filter.file.is_none()
            {
                // still need file filter only when set; id checked in matches
            }
            let h = match Handoff::read_json(handoff.as_bytes()) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let file = h.file.to_string_lossy();
            if !filter.matches(&id, file.as_ref()) {
                continue;
            }
            items.push(item(kind, &id, &h));
        }
    }
    Ok(serde_json::json!({
        "v": 1,
        "kind": kind.as_str(),
        "items": items,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{Archive, ArchiveCfg, tests::handoff};
    use crate::meta::Meta;

    fn open(d: &tempfile::TempDir) -> Archive {
        crate::archive::open(&ArchiveCfg {
            database: d.path().join("jack.db").to_string_lossy().into_owned(),
            ..Default::default()
        })
        .unwrap()
    }

    fn meta(kv: &[(&str, &str)]) -> Meta {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn ask_summaries_returns_every_source_and_filters_by_id() {
        let d = tempfile::tempdir().unwrap();
        let a = open(&d);
        let mut h = handoff("The Matrix");
        h.entry.insert("description".into(), "Merged plot.".into());
        h.candidates = vec![
            (
                "tmdb".into(),
                meta(&[("description", "TMDB plot"), ("title", "The Matrix")]),
            ),
            (
                "omdb".into(),
                meta(&[("plot", "OMDb plot"), ("poster_url", "http://x/p.jpg")]),
            ),
        ];
        let (id, _) = a.write(&h).unwrap();
        a.write(&handoff("Other")).unwrap();
        let (kind, f) = Filter::parse(&["summaries".into()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&query(&a, kind, &f).unwrap()).unwrap();
        assert_eq!(v["v"], 1);
        assert_eq!(v["kind"], "summaries");
        assert_eq!(v["items"].as_array().unwrap().len(), 2);
        let (kind, f) = Filter::parse(&["summaries".into(), "--id".into(), id.clone()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&query(&a, kind, &f).unwrap()).unwrap();
        assert_eq!(v["items"].as_array().unwrap().len(), 1);
        let texts: Vec<&str> = v["items"][0]["summaries"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x["text"].as_str())
            .collect();
        assert!(
            texts.contains(&"Merged plot.")
                && texts.contains(&"TMDB plot")
                && texts.contains(&"OMDb plot"),
            "{texts:?}"
        );
        let (kind, f) = Filter::parse(&["images".into(), id.clone()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&query(&a, kind, &f).unwrap()).unwrap();
        assert_eq!(v["kind"], "images");
        assert_eq!(v["items"][0]["images"][0]["url"], "http://x/p.jpg");
        assert!(Filter::parse(&[]).is_err());
        assert!(Filter::parse(&["widgets".into()]).is_err());
    }
}
