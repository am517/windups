//! When Jack cannot decide and Sammy is not available.
//! A person at a terminal is asked. Anyone else gets the reason and exit code 1.
use std::fmt::Write as _;
use std::io::{IsTerminal, Write};

/// Prints the question immediately (the report is shown only at the end of the run). The list
/// goes to the terminal once and is not repeated in the report. Returns the picks typed, if any.
/// `interactive` is false for a pipe and for another windup: no question, no list.
pub fn ask(
    list: &crate::pick::List,
    interactive: bool,
    read: &mut dyn FnMut() -> String,
    out: &mut String,
) -> Option<String> {
    if !interactive {
        return None;
    }
    let question = "Would you like to see candidates (y/n)?";
    eprintln!("{question}");
    let _ = std::io::stderr().flush();
    let _ = writeln!(out, "    {question}");
    let answer = read();
    let yes = matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    if !yes {
        return None;
    }
    if list.is_empty() {
        eprintln!("I don't have any candidates to show.");
        let _ = writeln!(out, "    I don't have any candidates to show.");
        return None;
    }
    for line in list.lines() {
        eprintln!("{line}");
    }
    eprintln!("Pick at most one per source (numbers separated by spaces; Enter for none):");
    let _ = std::io::stderr().flush();
    let picks = read();
    let _ = writeln!(out, "    picked: {}", picks.trim());
    Some(picks)
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
    fn a_person_sees_the_list_only_after_yes_and_a_pipe_is_not_asked() {
        let mut shown = String::new();
        assert_eq!(ask(&list(), false, &mut || "y".into(), &mut shown), None);
        assert!(shown.is_empty(), "a pipe or a windup is not asked: {shown}");

        let mut shown = String::new();
        assert_eq!(ask(&list(), true, &mut || "n".into(), &mut shown), None);
        assert!(shown.contains("Would you like to see candidates (y/n)?"));

        let mut answers = vec!["1 2".to_string(), "y".to_string()];
        let mut shown = String::new();
        let picks = ask(&list(), true, &mut || answers.pop().unwrap(), &mut shown);
        assert_eq!(picks.as_deref(), Some("1 2"));
        // the list went to the terminal once; the report only records the picks
        assert!(!shown.contains("The Office"), "{shown}");
        assert!(shown.contains("picked: 1 2"), "{shown}");
    }
}
