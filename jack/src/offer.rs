//! When Jack cannot decide and Sammy is not available.
//! A person at a terminal is asked. Anyone else gets the reason and exit code 1.
use crate::meta::Meta;
use crate::similarity;
use std::fmt::Write as _;
use std::io::{IsTerminal, Write};

/// One line per candidate, with the score Jack recorded.
pub fn lines(query_title: &str, found: &[(String, Meta)]) -> Vec<String> {
    found
        .iter()
        .filter(|(source, _)| source != "embedded")
        .map(|(source, meta)| {
            let title = meta
                .get("show")
                .or(meta.get("title"))
                .map_or("?", |value| value.as_str());
            let year = meta.get("year").map_or("?", |value| value.as_str());
            let score = meta
                .get("match_score")
                .map(|value| value.as_str().to_string())
                .unwrap_or_else(|| format!("{:.2}", similarity::score(query_title, title)));
            format!("{source}: {title} ({year}), score {score}")
        })
        .collect()
}

/// Prints the question immediately (the report is shown only at the end of the run).
/// `interactive` is false for a pipe and for another windup: no question, no list.
pub fn ask(
    candidates: &[String],
    interactive: bool,
    read: &mut dyn FnMut() -> String,
    out: &mut String,
) {
    if !interactive {
        return;
    }
    let question = "Would you like to see candidates (y/n)?";
    eprintln!("{question}");
    let _ = std::io::stderr().flush();
    let _ = writeln!(out, "    {question}");
    let answer = read();
    let yes = matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    if !yes {
        return;
    }
    if candidates.is_empty() {
        eprintln!("I don't have any candidates to show.");
        let _ = writeln!(out, "    I don't have any candidates to show.");
        return;
    }
    for (index, candidate) in candidates.iter().enumerate() {
        eprintln!("{}. {candidate}", index + 1);
        let _ = writeln!(out, "    {}. {candidate}", index + 1);
    }
}

pub fn person_is_here() -> bool {
    std::io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_person_sees_the_list_only_after_yes_and_a_pipe_is_not_asked() {
        let mut shown = String::new();
        ask(
            &["tmdb: The Office (2005), score 1.00".into()],
            false,
            &mut || "y".into(),
            &mut shown,
        );
        assert!(shown.is_empty(), "a pipe or a windup is not asked: {shown}");

        let mut shown = String::new();
        ask(
            &["tmdb: The Office (2005), score 1.00".into()],
            true,
            &mut || "n".into(),
            &mut shown,
        );
        assert!(
            shown.contains("Would you like to see candidates (y/n)?"),
            "{shown}"
        );
        assert!(!shown.contains("The Office"), "{shown}");

        let mut shown = String::new();
        ask(
            &[
                "tmdb: The Office (2005), score 1.00".into(),
                "tmdb: The Office (2001), score 1.00".into(),
            ],
            true,
            &mut || "y".into(),
            &mut shown,
        );
        assert!(shown.contains("1. tmdb: The Office (2005)"), "{shown}");
        assert!(shown.contains("2. tmdb: The Office (2001)"), "{shown}");
    }
}
