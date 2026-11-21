//! Metadata from yt-dlp, when it is installed.
//! YouTube's own API is preferred when a key is set. X has no free read API, so yt-dlp is the source.
use super::info_json;
use crate::meta::Meta;
use crate::tools;
use serde_json::Value;
use std::process::Command;

pub enum Read {
    Meta(Meta),
    /// The tool is not on this computer. Say so; do not fail the run for that alone.
    Missing(String),
    Failed(String),
}

/// `yt-dlp --dump-json` for one page. Does not download the media.
pub fn dump(url: &str) -> Read {
    if !tools::installed("yt-dlp") {
        return Read::Missing(tools::missing("yt-dlp"));
    }
    let output = match Command::new("yt-dlp")
        .args([
            "--dump-json",
            "--no-download",
            "--no-warnings",
            "--no-playlist",
            url,
        ])
        .output()
    {
        Ok(output) => output,
        Err(error) => return Read::Failed(format!("yt-dlp: {error}")),
    };
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let line = err
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("yt-dlp failed");
        return Read::Failed(format!("yt-dlp: {line}"));
    }
    match serde_json::from_slice::<Value>(&output.stdout) {
        Ok(value) => from_json(&value).map_or(
            Read::Failed("yt-dlp returned JSON I could not read as a post".into()),
            Read::Meta,
        ),
        Err(error) => Read::Failed(format!("yt-dlp JSON: {error}")),
    }
}

pub fn from_json(value: &Value) -> Option<Meta> {
    info_json::parse(value).map(|label| label.info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dump_json_for_an_x_post_is_metadata() {
        let meta = from_json(&json!({
            "extractor_key": "Twitter",
            "id": "42",
            "title": "A post",
            "uploader": "NASA",
            "upload_date": "20200102",
            "webpage_url": "https://x.com/nasa/status/42"
        }))
        .unwrap();
        assert_eq!(meta["title"], "A post");
        assert_eq!(meta["channel"], "NASA");
        assert_eq!(meta["x_video_id"], "42");
        assert_eq!(meta["date"], "2020-01-02");
    }
}
