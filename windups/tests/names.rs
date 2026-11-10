//! Crate folders and the name registry (`NAMES.md`) must agree.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cells(line: &str) -> Vec<String> {
    line.split('|')
        .map(|c| c.trim().replace('—', "-"))
        .filter(|c| !c.is_empty())
        .collect()
}

fn header_and_rows(md: &str, heading: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let rest = md
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing heading {heading}"))
        .1;
    let mut lines = rest.lines();
    let header = loop {
        let line = lines.next().expect("no table after heading");
        if line.starts_with('|') {
            break line;
        }
    };
    let head = cells(header);
    let _ = lines.next();
    let mut rows = Vec::new();
    for line in lines {
        if !line.starts_with('|') {
            break;
        }
        if line.contains("---") {
            continue;
        }
        let row = cells(line);
        assert_eq!(row.len(), head.len(), "row {row:?} vs {head:?}");
        rows.push(row);
    }
    (head, rows)
}

fn col(head: &[String], row: &[String], name: &str) -> String {
    let i = head
        .iter()
        .position(|h| h == name)
        .unwrap_or_else(|| panic!("no column {name} in {head:?}"));
    row[i].clone()
}

fn crate_folders(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for ent in std::fs::read_dir(root).unwrap().flatten() {
        if !ent.file_type().unwrap().is_dir() {
            continue;
        }
        let name = ent.file_name().to_string_lossy().into_owned();
        if name == "windups" || name.starts_with('.') {
            continue;
        }
        if ent.path().join("Cargo.toml").is_file() {
            out.insert(name);
        }
    }
    out
}

fn workspace_members(root: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let start = text.find("members = [").expect("members");
    let rest = &text[start..];
    let end = rest.find(']').expect("members close");
    rest[..end]
        .split(['"', '\''])
        .enumerate()
        .filter_map(|(i, s)| (i % 2 == 1).then_some(s))
        .filter(|s| *s != "windups" && !s.is_empty() && !s.contains('/') && !s.contains('.'))
        .map(str::to_string)
        .collect()
}

#[test]
fn crate_folders_match_the_name_registry() {
    let root = workspace_root();
    let names_md = std::fs::read_to_string(root.join("windups/NAMES.md")).unwrap();
    let (head, rows) = header_and_rows(&names_md, "## Registry");
    assert!(head.iter().any(|h| h == "Name"), "{head:?}");
    assert!(head.iter().any(|h| h == "Role"), "{head:?}");
    assert!(head.iter().any(|h| h == "Status"), "{head:?}");
    assert!(head.iter().any(|h| h == "Position"), "{head:?}");
    assert!(head.iter().any(|h| h == "Added"), "{head:?}");

    let mut registry = BTreeSet::new();
    let mut positions = BTreeMap::new();
    for row in &rows {
        let name = col(&head, row, "Name");
        let status = col(&head, row, "Status");
        let pos = col(&head, row, "Position");
        let added = col(&head, row, "Added");
        let role = col(&head, row, "Role");
        assert!(
            matches!(status.as_str(), "stub" | "active" | "done"),
            "{name}: status {status} is not stub/active/done"
        );
        assert!(!role.is_empty(), "{name}: empty role");
        assert!(
            added.chars().filter(|c| *c == '-').count() == 2 && added.len() == 10,
            "{name}: added date {added} is not YYYY-MM-DD"
        );
        assert!(
            pos == "-" || pos.parse::<u32>().is_ok(),
            "{name}: position {pos} is not a number or —"
        );
        let key = name.to_lowercase();
        assert!(
            registry.insert(key.clone()),
            "duplicate registry name {name}"
        );
        positions.insert(key, pos);
    }

    let crates = crate_folders(&root);
    let members = workspace_members(&root);
    let extra_reg: Vec<_> = registry.difference(&crates).cloned().collect();
    let extra_crate: Vec<_> = crates.difference(&registry).cloned().collect();
    assert!(
        extra_reg.is_empty() && extra_crate.is_empty(),
        "registry vs crate folders disagree: in registry only {extra_reg:?}; in crates only {extra_crate:?}"
    );
    let extra_mem: Vec<_> = members.difference(&registry).cloned().collect();
    let missing_mem: Vec<_> = registry.difference(&members).cloned().collect();
    assert!(
        extra_mem.is_empty() && missing_mem.is_empty(),
        "registry vs workspace members disagree: members only {extra_mem:?}; registry only {missing_mem:?}"
    );

    let windups = std::fs::read_to_string(root.join("windups/WINDUPS.md")).unwrap();
    let (whead, wrows) = header_and_rows(&windups, "## Roster");
    assert!(whead.iter().any(|h| h == "Windup"), "{whead:?}");
    assert!(
        whead.iter().any(|h| h == "Position"),
        "cast table needs a Position column: {whead:?}"
    );
    let mut cast = BTreeSet::new();
    for row in &wrows {
        let name = col(&whead, row, "Windup");
        let key = name.to_lowercase();
        assert!(cast.insert(key.clone()), "duplicate cast name {name}");
        if let Some(p) = positions.get(&key) {
            let got = col(&whead, row, "Position").replace('—', "-");
            assert_eq!(got, *p, "{name}: cast position {got} != registry {p}");
        }
    }
    let extra_cast: Vec<_> = cast.difference(&registry).cloned().collect();
    let missing_cast: Vec<_> = registry.difference(&cast).cloned().collect();
    assert!(
        extra_cast.is_empty() && missing_cast.is_empty(),
        "registry vs WINDUPS.md roster disagree: roster only {extra_cast:?}; registry only {missing_cast:?}"
    );
}

#[test]
fn every_windup_has_a_personality_matching_its_profile() {
    let root = workspace_root();
    let names_md = std::fs::read_to_string(root.join("windups/NAMES.md")).unwrap();
    assert!(names_md.contains("CULTURE.md"), "NAMES.md links CULTURE.md");
    let (head, rows) = header_and_rows(&names_md, "## Registry");
    for row in &rows {
        let name = col(&head, row, "Name");
        let cell = col(&head, row, "Personality");
        let (word, proposed) = match cell.strip_suffix(" (proposed)") {
            Some(w) => (w.to_string(), true),
            None => (cell.clone(), false),
        };
        assert!(!word.trim().is_empty(), "{name}: no personality");
        let profile =
            std::fs::read_to_string(root.join(name.to_lowercase()).join("PROFILE.toml")).unwrap();
        let (p, status) = windups::base::personality::from_profile(&profile)
            .unwrap_or_else(|| panic!("{name}: PROFILE.toml has no personality"));
        assert_eq!(p, word, "{name}: NAMES.md and PROFILE.toml disagree");
        assert_eq!(
            status == "proposed",
            proposed,
            "{name}: proposed in one place, set in the other"
        );
        assert!(
            matches!(
                status.as_str(),
                "set" | "set (work in progress)" | "proposed"
            ),
            "{name}: {status}"
        );
    }
}
