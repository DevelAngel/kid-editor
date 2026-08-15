use recipe::{RecipeFile, RecipeName};

use clap::{Arg, ArgMatches, Command, ValueHint};

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let file_path = preparse_file_arg(&args[1..]);

    let file = match RecipeFile::load(&file_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    // clap's `Command`/`Arg` are fully owned (`'static`), but recipe names
    // and args are only known once the file is loaded. Leaking once here,
    // for a short-lived CLI process that exits right after, is simpler and
    // cheaper than cloning every recipe name/description/help string
    // individually into the builder.
    let file: &'static RecipeFile = Box::leak(Box::new(file));

    let matches = build_cli(file).get_matches();

    match matches.subcommand() {
        Some(("list", _)) => {
            list(file);
            ExitCode::SUCCESS
        }
        Some(("run", run_matches)) => {
            let cwd = run_matches
                .get_one::<PathBuf>("cwd")
                .cloned()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let Some((name, recipe_matches)) = run_matches.subcommand() else {
                unreachable!("`run` declares subcommand_required(true)");
            };
            run(file, name, recipe_matches, &cwd)
        }
        _ => unreachable!("top-level `Command` declares subcommand_required(true)"),
    }
}

/// Scans argv for a top-level `--file`/`--file=VALUE` occurring before the
/// first subcommand token (`list` or `run`). Must run before the recipe
/// file is loaded, since building the full `Command` below needs that file
/// to generate per-recipe subcommands and their args.
///
/// A `--file` appearing after `run <recipe-name>` belongs to that recipe's
/// own generated args (a recipe may itself declare a parameter named
/// `file`), not to this flag — stopping the scan at the subcommand token
/// keeps that distinction intact, matching clap's own non-global-arg
/// scoping rather than special-casing it.
fn preparse_file_arg(args: &[String]) -> PathBuf {
    let mut file = PathBuf::from("recipes.toml");
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "list" | "run" => break,
            "--file" => {
                if let Some(value) = iter.next() {
                    file = PathBuf::from(value);
                }
            }
            arg => {
                if let Some(value) = arg.strip_prefix("--file=") {
                    file = PathBuf::from(value);
                }
            }
        }
    }
    file
}

/// Builds the full CLI, including one `run` subcommand per declared
/// recipe with `--<param>` flags generated from `recipe.args`. Recipe help
/// text (`--help` on a specific recipe) therefore reflects the actual
/// loaded `recipes.toml`, not a static description.
fn build_cli(file: &'static RecipeFile) -> Command {
    let mut run_cmd = Command::new("run")
        .about("Run one recipe by name.")
        .arg(
            Arg::new("cwd")
                .long("cwd")
                .value_name("DIR")
                .value_hint(ValueHint::DirPath)
                .value_parser(clap::value_parser!(PathBuf))
                .help("Directory the recipe runs in. Defaults to the current working directory."),
        )
        .subcommand_required(true)
        .arg_required_else_help(true);

    for (name, recipe) in file.iter() {
        let mut sub = Command::new(name.as_str());
        if !recipe.description.is_empty() {
            sub = sub.about(recipe.description.clone());
        }
        for (arg_name, arg) in &recipe.args {
            let mut a = Arg::new(arg_name.as_str())
                .long(arg_name.as_str())
                .required(true);
            if !arg.help.is_empty() {
                a = a.help(arg.help.clone());
            }
            sub = sub.arg(a);
        }
        run_cmd = run_cmd.subcommand(sub);
    }

    Command::new("kid-recipes")
        .version(env!("CARGO_PKG_VERSION"))
        .about(
            "Runs recipes declared in a `recipes.toml` file — a minimal, \
             shell-free alternative to `just`.",
        )
        .arg(
            Arg::new("file")
                .long("file")
                .value_name("FILE")
                .value_hint(ValueHint::FilePath)
                .value_parser(clap::value_parser!(PathBuf))
                .default_value("recipes.toml")
                .help("Path to the recipe file."),
        )
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("list").about("List every recipe this file declares."))
        .subcommand(run_cmd)
}

fn list(file: &RecipeFile) {
    for (name, recipe) in file.iter() {
        let params = recipe
            .args
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        match params.as_str() {
            "" => println!("{name}"),
            params => println!("{name} {params}"),
        }
        if !recipe.description.is_empty() {
            println!("    {}", recipe.description);
        }
    }
}

fn run(file: &RecipeFile, name: &str, matches: &ArgMatches, cwd: &std::path::Path) -> ExitCode {
    let Some(recipe) = file.get(&RecipeName::from(name)) else {
        eprintln!("{name}: no such recipe");
        return ExitCode::FAILURE;
    };

    // clap enforces `required(true)` on every generated arg above, so each
    // key is guaranteed present; `unwrap_or_default` is only a defensive
    // fallback, never expected to trigger.
    let provided: Vec<String> = recipe
        .args
        .keys()
        .map(|arg_name| {
            matches
                .get_one::<String>(arg_name)
                .cloned()
                .unwrap_or_default()
        })
        .collect();

    match recipe.execute(&provided, cwd) {
        Ok(output) => {
            print!("{}", String::from_utf8_lossy(&output.stdout));
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            match output.status.code() {
                Some(code) => ExitCode::from(code as u8),
                None => ExitCode::FAILURE,
            }
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
