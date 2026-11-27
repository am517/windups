mod archive;
mod art;
mod ask;
mod askart;
mod candidates;
mod cli;
mod completeness;
mod config;
mod conflicts;
mod decide;
mod escalate;
mod explain;
mod facts;
mod fetch;
mod filemeta;
mod handoff;
mod matcher;
mod meta;
mod objective;
mod online;
mod print;
mod process;
mod profile;
mod providers;
mod resolver;
mod reuse;
mod review;
mod run;
mod scan;
mod secrets;
mod setcfg;
mod setflags;
mod settle;
mod sorter;
mod sources;
mod stage;
mod sync;
mod tagger;
#[cfg(test)]
mod testutil;
mod to_daniel;
mod universal;

use clap::Parser;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if universal::is_command(&args) {
        std::process::exit(universal::main(&args));
    }
    let mut cli = cli::Cli::parse();
    if !cli.capabilities {
        universal::ensure_folder(cli.config.as_deref());
    }
    crate::handoff::set_caller(cli.caller.clone());
    if cli.capabilities {
        if cli.windup {
            println!("{}", profile::machine());
        } else {
            println!("{}", profile::human());
        }
        return;
    }
    let secret_err = if cli.windup {
        secrets::resolve_secrets(&mut cli, &mut |_| {
            Err("cannot prompt in --windup mode; pass the key as --flag VALUE".into())
        })
    } else {
        secrets::resolve_secrets(&mut cli, &mut secrets::read_secret)
    };
    if let Err(e) = secret_err {
        if cli.windup {
            println!("{}", windups::base::flags::error_json(&e));
        } else {
            eprintln!("jack: {e}");
        }
        std::process::exit(2);
    }
    if !cli.windup && !cli.paths.is_empty() && !(cli.set && secrets::has_secret(&cli)) {
        let listed = cli
            .paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let kind = if cli.dry_run {
            "dry run"
        } else if cli.download_only && cli.print {
            "download and print"
        } else if cli.download_only {
            "download"
        } else if cli.print {
            "print"
        } else {
            "tag"
        };
        eprintln!("jack: {kind}: {listed}");
        let _ = std::io::Write::flush(&mut std::io::stderr());
    }
    match run::run(&cli, &providers::Ureq) {
        Ok((s, printed, failed)) => {
            if cli.windup {
                // machine-readable result or failure only
                print!("{printed}");
            } else if s.trim().is_empty() && printed.trim().is_empty() {
                eprintln!("jack: finished without saying what happened");
                std::process::exit(1);
            } else if printed.is_empty() && cli.emit_json.as_deref() != Some("-") {
                print!("{s}");
            } else {
                eprint!("{s}");
                print!("{printed}");
            }
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let _ = std::io::Write::flush(&mut std::io::stderr());
            if failed > 0 {
                std::process::exit(1);
            }
        }
        Err(e) => {
            if cli.windup {
                println!("{}", windups::base::flags::error_json(&e));
            } else {
                eprintln!("jack: {e}");
            }
            std::process::exit(2);
        }
    }
}
