//! Metadata exiftool can read from a local file, when exiftool is installed.
use crate::meta::Meta;
use crate::tools;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

pub enum Read {
    Meta(Meta),
    Missing(String),
    Failed(String),
}

pub fn read(path: &Path) -> Read {
    if !tools::installed("exiftool") {
        return Read::Missing(tools::missing("exiftool"));
    }
    let output = match Command::new("exiftool").arg("-j").arg(path).output() {
        Ok(output) => output,
        Err(error) => return Read::Failed(format!("exiftool: {error}")),
    };
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let line = err.lines().next().unwrap_or("exiftool failed");
        return Read::Failed(format!("exiftool: {line}"));
    }
    match serde_json::from_slice::<Value>(&output.stdout) {
        Ok(value) => Read::Meta(from_json(&value)),
        Err(error) => Read::Failed(format!("exiftool JSON: {error}")),
    }
}

/// The first object in `exiftool -j` output.
pub fn from_json(value: &Value) -> Meta {
    let object = value
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or(value);
    let mut meta = Meta::new();
    let put = |meta: &mut Meta, key: &str, names: &[&str]| {
        for name in names {
            if let Some(text) = object.get(*name).and_then(Value::as_str) {
                let text = text.trim();
                if !text.is_empty() {
                    meta.insert(key.into(), text.to_string());
                    return;
                }
            }
        }
    };
    put(&mut meta, "title", &["Title", "Headline"]);
    put(
        &mut meta,
        "description",
        &["Description", "ImageDescription", "Comment"],
    );
    put(
        &mut meta,
        "date",
        &["CreateDate", "DateTimeOriginal", "MediaCreateDate"],
    );
    put(&mut meta, "comment", &["Comment", "UserComment"]);
    if let Some(date) = meta.get("date").cloned() {
        if let Some(year) = date
            .get(..4)
            .filter(|year| year.bytes().all(|b| b.is_ascii_digit()))
        {
            meta.entry("year".into())
                .or_insert_with(|| year.to_string());
        }
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exiftool_json_fills_title_and_date() {
        let meta = from_json(&json!([{
            "Title": "Mountain",
            "CreateDate": "2019:04:01 12:00:00",
            "Comment": "https://x.com/nasa/status/42"
        }]));
        assert_eq!(meta["title"], "Mountain");
        assert_eq!(meta["year"], "2019");
        assert!(meta["comment"].contains("status/42"));
    }
}
