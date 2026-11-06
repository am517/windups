mod art;
mod askart;
mod candidates;
mod cli;
mod completeness;
mod config;
mod conflicts;
mod daniel;
mod decide;
mod escalate;
mod explain;
mod facts;
mod fetch;
mod filemeta;
mod matcher;
mod meta;
mod online;
mod print;
mod process;
mod profile;
mod providers;
mod resolver;
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
mod tagger;
#[cfg(test)]
mod testutil;

use clap::Parser;

fn main() {
    let mut cli = cli::Cli::parse();
    crate::daniel::set_caller(cli.caller.clone());
    if cli.capabilities {
        print!("{}", profile::PROFILE);
        return;
    }
    if let Err(e) = secrets::resolve_secrets(&mut cli, &mut secrets::read_secret) {
        eprintln!("jack: {e}");
        std::process::exit(1);
    }
    match run::run(&cli, &providers::Ureq) {
        Ok((s, printed, failed)) => {
            // with --print or --emit-json -, stdout carries only the data; the report goes to stderr
            if printed.is_empty() && cli.emit_json.as_deref() != Some("-") {
                print!("{s}");
            } else {
                eprint!("{s}");
                print!("{printed}");
            }
            if failed > 0 {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("jack: {e}");
            std::process::exit(1);
        }
    }
}
