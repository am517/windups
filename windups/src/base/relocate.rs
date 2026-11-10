//! Paths in a config that point into a folder being migrated. `migrate` rewrites them in the NEW
//! copy's config only (the original is never touched); paths elsewhere are only warned about.

/// What to do with one config: the rewritten text (None = nothing to rewrite), each rewrite as
/// "key: old -> new", and a warning for each absolute path left alone.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub text: Option<String>,
    pub rewrites: Vec<String>,
    pub warnings: Vec<String>,
}

/// `moves` are (old prefix, new prefix) pairs, e.g. the absolute and `~/` spellings of the folder.
pub fn plan(text: &str, moves: &[(String, String)]) -> Plan {
    let Ok(orig) = toml::from_str::<toml::Table>(text) else {
        return Plan {
            warnings: vec!["config.toml isn't valid TOML; copied as is, paths not checked".into()],
            ..Default::default()
        };
    };
    let mut p = Plan::default();
    let mut swaps = Vec::new();
    let mut want = orig.clone();
    walk(&mut want, "", &mut |key, v| {
        let moved = moves.iter().find_map(|(from, to)| {
            let rest = v.strip_prefix(from.as_str())?;
            (rest.is_empty() || rest.starts_with('/')).then(|| format!("{to}{rest}"))
        });
        match moved {
            Some(n) => {
                p.rewrites.push(format!("{key}: {v} -> {n}"));
                swaps.push((v.clone(), n.clone()));
                *v = n;
            }
            None if v.starts_with('/') || v.starts_with("~/") => p
                .warnings
                .push(format!("{key} = {v} is outside the old folder; left as is")),
            None => {}
        }
    });
    if swaps.is_empty() {
        return p;
    }
    let mut out = text.to_string();
    for (o, n) in &swaps {
        let quoted = |s: &str| toml::Value::String(s.into()).to_string();
        out = out
            .replace(&quoted(o), &quoted(n))
            .replace(&format!("'{o}'"), &format!("'{n}'"));
    }
    // the rewrite must change exactly those values and nothing else
    if toml::from_str::<toml::Table>(&out).ok().as_ref() == Some(&want) {
        p.text = Some(out);
    } else {
        p.warnings.extend(
            p.rewrites
                .drain(..)
                .map(|r| format!("couldn't rewrite {r} automatically; edit it by hand")),
        );
    }
    p
}

fn walk(t: &mut toml::Table, at: &str, f: &mut dyn FnMut(&str, &mut String)) {
    for (k, v) in t.iter_mut() {
        let key = if at.is_empty() {
            k.clone()
        } else {
            format!("{at}.{k}")
        };
        value(v, &key, f);
    }
}

fn value(v: &mut toml::Value, key: &str, f: &mut dyn FnMut(&str, &mut String)) {
    match v {
        toml::Value::String(s) => f(key, s),
        toml::Value::Table(t) => walk(t, key, f),
        toml::Value::Array(a) => {
            for (i, x) in a.iter_mut().enumerate() {
                value(x, &format!("{key}[{i}]"), f);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moves() -> Vec<(String, String)> {
        vec![
            ("/h/.config/windups/toy".into(), "/h/windups/toy".into()),
            ("~/.config/windups/toy".into(), "~/windups/toy".into()),
        ]
    }

    #[test]
    fn inside_paths_are_rewritten_outside_ones_warned() {
        let t = "# see /h/.config/windups/toy/notes\n[archive]\ndatabase = \"/h/.config/windups/toy/jack.db\"\n\
                 [core.snapshots]\ndir = '~/.config/windups/toy/snaps'\n[x]\nlib = \"/srv/media\"\n\
                 near = \"/h/.config/windups/toyota\"\nword = \"hello\"\n";
        let p = plan(t, &moves());
        let out = p.text.unwrap();
        assert!(
            out.contains("database = \"/h/windups/toy/jack.db\""),
            "{out}"
        );
        assert!(out.contains("dir = '~/windups/toy/snaps'"), "{out}");
        assert!(
            out.starts_with("# see /h/.config/windups/toy/notes"),
            "comments stay"
        );
        assert_eq!(
            p.rewrites,
            [
                "archive.database: /h/.config/windups/toy/jack.db -> /h/windups/toy/jack.db",
                "core.snapshots.dir: ~/.config/windups/toy/snaps -> ~/windups/toy/snaps"
            ]
        );
        assert_eq!(p.warnings.len(), 2, "{:?}", p.warnings);
        assert!(p.warnings[0].starts_with("x.lib = /srv/media is outside the old folder"));
        assert!(p.warnings[1].starts_with("x.near = /h/.config/windups/toyota is outside"));
    }

    #[test]
    fn nothing_to_rewrite_and_bad_toml() {
        assert_eq!(plan("a = 1\n", &moves()), Plan::default());
        assert!(plan("a = \n", &moves()).warnings[0].contains("isn't valid TOML"));
    }
}
