//! Turn a flag value into a source id. A flag takes a link or an id.
use crate::online::urls;

/// `Q12345`, or a wikidata.org wiki/entity link.
pub fn wikidata(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(rest) = value.split("wikidata.org/").nth(1) {
        return rest
            .split(|c: char| !c.is_ascii_alphanumeric())
            .find_map(qid);
    }
    qid(value)
}

pub fn require_wikidata(value: &str) -> Result<String, String> {
    wikidata(value).ok_or_else(|| {
        format!("--wikidata: {value} is not a Wikidata id (Q123) or a wikidata.org link")
    })
}

/// A Wikipedia article title, or an en.wikipedia.org/wiki/ link (any language host).
pub fn wikipedia(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(rest) = value.split("wikipedia.org/wiki/").nth(1) {
        let title = rest.split(['?', '#']).next().unwrap_or(rest);
        let title = decode(title);
        return (!title.is_empty()).then_some(title);
    }
    if value.contains("://") {
        return None;
    }
    Some(value.replace('_', " "))
}

pub fn require_wikipedia(value: &str) -> Result<String, String> {
    wikipedia(value).ok_or_else(|| {
        format!("--wikipedia: {value} is not an article title or a wikipedia.org/wiki link")
    })
}

/// Digits, or an x.com / twitter.com status link.
pub fn x_status(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("--x needs a status id or an x.com / twitter.com status link".into());
    }
    if let Some((site, id)) = urls::parse(value) {
        if site == "x" {
            return Ok(id);
        }
        return Err(format!(
            "--x: {value} is a {site} link, not an X status link"
        ));
    }
    if value.chars().all(|c| c.is_ascii_digit()) {
        return Ok(value.to_string());
    }
    Err(format!(
        "--x: {value} is not a status id (digits) or an x.com / twitter.com status link"
    ))
}

fn qid(value: &str) -> Option<String> {
    let digits = value
        .strip_prefix('Q')
        .or_else(|| value.strip_prefix('q'))?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("Q{digits}"))
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wikidata_and_wikipedia_take_a_link_or_an_id() {
        assert_eq!(wikidata("q83495").as_deref(), Some("Q83495"));
        assert_eq!(
            wikidata("https://www.wikidata.org/wiki/Q83495").as_deref(),
            Some("Q83495")
        );
        assert_eq!(
            wikidata("https://www.wikidata.org/entity/Q83495").as_deref(),
            Some("Q83495")
        );
        assert!(wikidata("The Matrix").is_none());
        assert_eq!(
            wikipedia("https://en.wikipedia.org/wiki/The_Matrix").as_deref(),
            Some("The Matrix")
        );
        assert_eq!(
            wikipedia("https://en.wikipedia.org/wiki/The_Matrix_(film)#Cast").as_deref(),
            Some("The Matrix (film)")
        );
        assert_eq!(wikipedia("The Matrix").as_deref(), Some("The Matrix"));
        assert!(require_wikidata("nope").is_err());
    }

    #[test]
    fn x_status_is_digits_or_a_status_link() {
        assert_eq!(x_status("1234567890").unwrap(), "1234567890");
        assert_eq!(x_status("https://x.com/nasa/status/42").unwrap(), "42");
        assert_eq!(x_status("https://twitter.com/i/status/99").unwrap(), "99");
        assert!(x_status("https://x.com/nasa").is_err());
        assert!(x_status("not-an-id").is_err());
        assert!(x_status("https://youtu.be/dQw4w9WgXcQ").is_err());
    }
}
