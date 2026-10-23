//! NestJS-style project and resource generation commands.

mod generate;

use std::{env, path::PathBuf};

fn run() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h" | "help") {
        println!(
            "Xecute — Rust modules, controllers and injectable services\n\n  xecute new <project> [--runtime-path <cli/xecute>]\n  xecute generate <resource|module|controller|service> <name>\n  xecute g <res|mo|co|s> <name>\n\nRun generate from a project created with xecute new. Resources include a GET endpoint; individual generators create standalone building blocks."
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
                return Err("usage: xecute new <project> [--runtime-path <cli/xecute>]".into());
            }
            let project = args.remove(0);
            let runtime = match args.as_slice() {
                [] => None,
                [flag, path] if flag == "--runtime-path" => Some(PathBuf::from(path)),
                _ => return Err("usage: xecute new <project> [--runtime-path <cli/xecute>]".into()),
            };
            generate::new_project(&project, runtime.as_deref())
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
