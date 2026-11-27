mod archive;
mod ask;
mod candidates;
mod cli;
mod completeness;
mod config;
mod conflicts;
mod credentials;
mod decide;
mod embedded_input;
mod escalate;
mod explain;
mod facts;
mod fetch;
mod filemeta;
mod handoff;
mod matcher;
mod meta;
mod objective;
mod offer;
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
mod similarity;
mod sorter;
mod source_id;
mod sources;
mod stage;
mod sync;
#[cfg(test)]
mod testutil;
mod to_daniel;
mod tools;
mod universal;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if universal::is_command(&args) {
        std::process::exit(universal::main(&args));
    }
    let cli = cli::Cli::read(std::env::args_os()).unwrap_or_else(|e| e.exit());
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
    if !cli.windup && !cli.paths.is_empty() && !(cli.set && secrets::has_secret(&cli)) {
        let listed = cli
            .paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let kind = if cli.dry_run {
            "dry run"
        } else if cli.print {
            "print"
        } else {
            "identify"
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
            } else if printed.is_empty() {
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
