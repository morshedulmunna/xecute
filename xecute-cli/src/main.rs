//! Xecute project and resource generation commands.

mod dev;
mod generate;
mod library;
mod manage;
mod manifest;

use std::{env, path::PathBuf};

fn run() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h" | "help") {
        println!(
            "Xecute — Rust modules, controllers and injectable services\n\n  xecute new <project|.> [--workspace] [--name <package-name>] [--mvc | --cqrs] [--runtime-path <cli/xecute>]\n  xecute update\n  xecute uninstall\n  xecute run dev [--bin <name>] [--features <list>] [--watch <path>] [-- <app arguments>]\n  xecute generate <resource|module|controller|service> <name>\n  xecute generate worker <name> [--distributed] [--broker <postgres-outbox|rabbitmq|nats>]\n  xecute g <res|mo|co|s> <name>\n  xecute g wo <name> [--distributed] [--broker <postgres-outbox|rabbitmq|nats>]\n  xecute g apps <app_name> [--mvc | --cqrs] [--runtime-path <path>]\n  xecute g library <name> (alias: lib; run from workspace root)\n\nNew projects use MVC by default; --cqrs selects command/query handlers. Run generate from a project created with xecute new. Resources include a GET endpoint; individual generators create standalone building blocks or background workers. Workers are process-local by default; --broker selects a distributed transport and implies --distributed."
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
                    "usage: xecute new <project|.> [--workspace] [--name <package-name>] [--mvc | --cqrs] [--runtime-path <cli/xecute>]"
                        .into(),
                );
            }
            let project = args.remove(0);
            let mut workspace = false;
            let mut runtime = None;
            let mut name_override = None;
            let mut architecture = generate::Architecture::Mvc;
            let mut architecture_seen = false;
            let mut flags = args.into_iter();
            while let Some(flag) = flags.next() {
                match flag.as_str() {
                    "--workspace" if !workspace => workspace = true,
                    "--mvc" if !architecture_seen => {
                        architecture = generate::Architecture::Mvc;
                        architecture_seen = true;
                    }
                    "--cqrs" if !architecture_seen => {
                        architecture = generate::Architecture::Cqrs;
                        architecture_seen = true;
                    }
                    "--name" if name_override.is_none() => {
                        name_override = Some(
                            flags
                                .next()
                                .filter(|value| !value.starts_with('-') && !value.is_empty())
                                .ok_or("--name requires a package name")?,
                        );
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
            let create = if workspace { generate::new_workspace } else { generate::new_project };
            create(
                &project,
                runtime.as_deref(),
                architecture,
                name_override.as_deref(),
            )
        }
        "update" if args.is_empty() => manage::run(manage::Action::Update),
        "uninstall" if args.is_empty() => manage::run(manage::Action::Uninstall),
        "update" | "uninstall" => Err(format!("usage: xecute {command}")),
        "run" if args.first().is_some_and(|arg| arg == "dev") => {
            dev::run(dev::Options::parse(&args[1..])?)
        }
        "run" => {
            Err("usage: xecute run dev [--bin <name>] [--features <list>] [--watch <path>] [-- <app arguments>]".into())
        }
        "generate" | "g" if args.first().is_some_and(|a| a == "apps") => {
            let name = args.get(1).ok_or("usage: xecute g apps <app_name> [--mvc | --cqrs] [--runtime-path <path>]")?;
            let mut architecture = None;
            let mut runtime = None;
            let mut flags = args[2..].iter();
            while let Some(flag) = flags.next() {
                match flag.as_str() {
                    "--mvc" if architecture.is_none() => architecture = Some(generate::Architecture::Mvc),
                    "--cqrs" if architecture.is_none() => architecture = Some(generate::Architecture::Cqrs),
                    "--runtime-path" if runtime.is_none() => runtime = Some(PathBuf::from(flags.next().filter(|v| !v.starts_with('-')).ok_or("--runtime-path requires a path")?)),
                    _ => return Err(format!("unknown or duplicate option: {flag}")),
                }
            }
            generate::add_app(&env::current_dir().map_err(|e| e.to_string())?, name, architecture, runtime.as_deref())
        }
        "generate" | "g" if args.first().is_some_and(|arg| matches!(arg.as_str(), "library" | "lib")) => {
            if args.len() != 2 { return Err("usage: xecute g library <name> (run from workspace root)".into()); }
            library::generate(&env::current_dir().map_err(|e| e.to_string())?, &args[1])
        }
        "generate" | "g"
            if args
                .first()
                .is_some_and(|arg| matches!(arg.as_str(), "worker" | "wo")) =>
        {
            let usage = "usage: xecute g worker <name> [--distributed] [--broker <postgres-outbox|rabbitmq|nats>]";
            let name = args.get(1).filter(|name| !name.starts_with('-')).ok_or(usage)?;
            let mut distributed = false;
            let mut broker = None;
            let mut flags = args[2..].iter();
            while let Some(flag) = flags.next() {
                match flag.as_str() {
                    "--distributed" if !distributed => distributed = true,
                    "--broker" if broker.is_none() => {
                        let value = flags
                            .next()
                            .filter(|value| !value.starts_with('-'))
                            .ok_or("--broker requires postgres-outbox, rabbitmq, or nats")?;
                        broker = Some(manifest::WorkerBroker::parse(value).ok_or(
                            "--broker must be postgres-outbox, rabbitmq, or nats",
                        )?);
                    }
                    _ => return Err(format!("unknown or duplicate option: {flag}")),
                }
            }
            let broker = if distributed || broker.is_some() {
                Some(broker.unwrap_or(manifest::WorkerBroker::PostgresOutbox))
            } else {
                None
            };
            generate::worker(
                &env::current_dir().map_err(|e| e.to_string())?,
                name,
                broker,
            )
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
