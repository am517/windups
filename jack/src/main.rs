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
        print!("{}", profile::PROFILE);
        return;
    }
    let secret_err = if cli.windup {
        secrets::resolve_secrets(&mut cli, &mut |_| {
            Err("cannot prompt in --windup mode; pass the key as --flag=VALUE".into())
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
    match run::run(&cli, &providers::Ureq) {
        Ok((s, printed, failed)) => {
            if cli.windup {
                // machine-readable result or failure only
                print!("{printed}");
            } else if printed.is_empty() && cli.emit_json.as_deref() != Some("-") {
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
            if cli.windup {
                println!("{}", windups::base::flags::error_json(&e));
            } else {
                eprintln!("jack: {e}");
            }
            std::process::exit(2);
        }
    }
}
