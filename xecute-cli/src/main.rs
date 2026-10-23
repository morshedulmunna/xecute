//! NestJS-style project and resource generation commands.

mod generate;

use std::{env, path::PathBuf};

fn run() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h" | "help") {
        println!(
            "Xecute — Rust modules, controllers and injectable services\n\n  xecute new <project> [--mvc | --cqrs] [--runtime-path <cli/xecute>]\n  xecute generate <resource|module|controller|service> <name>\n  xecute g <res|mo|co|s> <name>\n\nNew projects use MVC by default; --cqrs selects command/query handlers. Run generate from a project created with xecute new. Resources include a GET endpoint; individual generators create standalone building blocks."
        );
        return Ok(());
    }
    if matches!(args[0].as_str(), "--version" | "-V") {
        println!("xecute {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let command = args.remove(0);
    match command.as_str() {
        "new" => {
            if args.is_empty() {
                return Err(
                    "usage: xecute new <project> [--mvc | --cqrs] [--runtime-path <cli/xecute>]"
                        .into(),
                );
            }
            let project = args.remove(0);
            let mut runtime = None;
            let mut architecture = generate::Architecture::Mvc;
            let mut architecture_seen = false;
            let mut flags = args.into_iter();
            while let Some(flag) = flags.next() {
                match flag.as_str() {
                    "--mvc" if !architecture_seen => {
                        architecture = generate::Architecture::Mvc;
                        architecture_seen = true;
                    }
                    "--cqrs" if !architecture_seen => {
                        architecture = generate::Architecture::Cqrs;
                        architecture_seen = true;
                    }
                    "--runtime-path" if runtime.is_none() => {
                        let path = flags
                            .next()
                            .filter(|value| !value.starts_with("--"))
                            .ok_or("--runtime-path requires a path")?;
                        runtime = Some(PathBuf::from(path));
                    }
                    _ => return Err(format!("unknown or duplicate option: {flag}")),
                }
            }
            generate::new_project(&project, runtime.as_deref(), architecture)
        }
        "generate" | "g" if args.len() == 2 => generate::component(
            &env::current_dir().map_err(|e| e.to_string())?,
            &args[0],
            &args[1],
        ),
        _ => Err("unknown command or arguments; run xecute --help".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
