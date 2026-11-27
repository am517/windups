//! When Jack cannot decide and Sammy is not available.
//! A person at a terminal is asked. Anyone else gets the reason and exit code 1.
use std::fmt::Write as _;
use std::io::{IsTerminal, Write};

/// Asks "see candidates (y/n)?" first (decided Nov 27). Prints it immediately: the report is
/// shown only at the end of the run.
pub fn wants_list(
    list: &crate::pick::List,
    read: &mut dyn FnMut() -> String,
    out: &mut String,
) -> bool {
    let question = "Would you like to see candidates (y/n)?";
    eprintln!("{question}");
    let _ = std::io::stderr().flush();
    let _ = writeln!(out, "    {question}");
    let answer = read();
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return false;
    }
    if list.is_empty() {
        eprintln!("I don't have any candidates to show.");
        let _ = writeln!(out, "    I don't have any candidates to show.");
        return false;
    }
    true
}

/// Shows the list once, on the terminal (not repeated in the report), and reads the picks.
/// `more`: "more" searches again with more results.
pub fn choose(
    list: &crate::pick::List,
    more: bool,
    read: &mut dyn FnMut() -> String,
    out: &mut String,
) -> String {
    for line in list.lines() {
        eprintln!("{line}");
    }
    eprintln!(
        "Pick at most one per source (numbers separated by spaces; Enter for none{}):",
        if more {
            "; \"more\" for more results"
        } else {
            ""
        }
    );
    let _ = std::io::stderr().flush();
    let picks = read();
    let _ = writeln!(out, "    picked: {}", picks.trim());
    picks
}

/// After Jack chose a match himself: "see the candidates (y/n)?", then the list with the
/// chosen work's rows marked `*`. Read-only: nothing is picked here.
pub fn show_chosen(lines: &[String], read: &mut dyn FnMut() -> String) -> bool {
    if lines.is_empty() {
        return false;
    }
    eprintln!("Would you like to see the candidates I chose from (y/n)?");
    let _ = std::io::stderr().flush();
    if !matches!(read().trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return false;
    }
    for line in lines {
        eprintln!("{line}");
    }
    eprintln!("(* = the sources that agree on my choice)");
    true
}

pub fn person_is_here() -> bool {
    std::io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::Meta;

    fn list() -> crate::pick::List {
        let f: Vec<(String, Meta)> = vec![
            (
                "tmdb:search".into(),
                [("title", "The Office"), ("year", "2005")]
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .into(),
            ),
            (
                "thetvdb:search".into(),
                [("title", "The Office"), ("year", "2001")]
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .into(),
            ),
        ];
        crate::pick::list("The Office", &f)
    }

    #[test]
    fn a_person_sees_the_list_only_after_yes() {
        let mut shown = String::new();
        assert!(!wants_list(&list(), &mut || "n".into(), &mut shown));
        assert!(shown.contains("Would you like to see candidates (y/n)?"));
        let mut shown = String::new();
        assert!(wants_list(&list(), &mut || "y".into(), &mut shown));
        let mut shown = String::new();
        let picks = choose(&list(), true, &mut || "1 2".into(), &mut shown);
        assert_eq!(picks, "1 2");
        // the list went to the terminal once; the report only records the picks
        assert!(!shown.contains("The Office"), "{shown}");
        assert!(shown.contains("picked: 1 2"), "{shown}");
    }

    #[test]
    fn after_jack_chooses_the_list_is_shown_only_on_yes() {
        assert!(!show_chosen(&["tmdb:".into()], &mut || "n".into()));
        assert!(show_chosen(&["tmdb:".into()], &mut || "y".into()));
        assert!(!show_chosen(&[], &mut || "y".into()));
    }
}
