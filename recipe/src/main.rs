use recipe::{RecipeFile, RecipeName};

use anyhow::{Context, Result};
use clap::{
    Arg, ArgMatches, Command, CommandFactory, FromArgMatches, Parser, Subcommand, ValueHint,
};

use std::env;
use std::path::{Path, PathBuf};
use std::process;

/// Single source of truth for `--file`'s default, referenced both by
/// `Cli`'s derive attribute and by `preparse_file_arg` (which needs a
/// fallback before `Cli` itself can be built — see below).
const DEFAULT_RECIPE_FILE: &str = "recipes.toml";

/// Runs recipes declared in a `recipes.toml` file — a minimal, shell-free
/// alternative to `just`.
#[derive(Parser, Debug)]
#[command(name = "kid-recipes", version)]
struct Cli {
    /// Path to the recipe file.
    #[arg(long, default_value = DEFAULT_RECIPE_FILE, value_hint = ValueHint::FilePath)]
    file: PathBuf,

    #[command(subcommand)]
    command: TopCommand,
}

#[derive(Subcommand, Debug)]
enum TopCommand {
    /// List every recipe this file declares.
    List,
    /// Run one recipe by name.
    Run {
        /// Directory the recipe runs in. Defaults to the current working
        /// directory.
        #[arg(long, value_hint = ValueHint::DirPath)]
        cwd: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let file_path = preparse_file_arg(&env::args().collect::<Vec<_>>());
    let file = RecipeFile::load(&file_path).context("failed to load recipe file")?;

    let matches = augment_with_recipes(Cli::command(), &file).get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    match cli.command {
        TopCommand::List => list(&file),
        TopCommand::Run { cwd } => {
            let cwd =
                cwd.unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let Some((name, recipe_matches)) = matches
                .subcommand_matches("run")
                .and_then(ArgMatches::subcommand)
            else {
                unreachable!("`augment_with_recipes` sets subcommand_required(true) on `run`");
            };
            run(&file, name, recipe_matches, &cwd)?;
        }
    }
    Ok(())
}

/// Scans argv for a top-level `--file`/`--file=VALUE` occurring before the
/// first subcommand token (`list` or `run`). Must run before the recipe
/// file is loaded, since `augment_with_recipes` needs that file to
/// generate per-recipe subcommands and their `--<param>` flags — the
/// entire reason this CLI can't just call `Cli::parse()` directly.
///
/// A `--file` appearing after `run <recipe-name>` belongs to that
/// recipe's own generated flags (a recipe may itself declare a parameter
/// named `file`), not to this one — stopping the scan at the subcommand
/// token keeps that distinction intact, matching clap's own
/// non-global-arg scoping rather than special-casing it.
fn preparse_file_arg(args: &[String]) -> PathBuf {
    let mut file = PathBuf::from(DEFAULT_RECIPE_FILE);
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

/// Appends one subcommand per declared recipe under `run`, with
/// `--<param>` flags generated from that recipe's own `args` map (name
/// and `help` text included). This is the one part of the CLI that can't
/// come from derive: recipe names and their parameters are runtime data,
/// only known once `recipes.toml` is loaded. Everything else — `--file`,
/// `list`, `run --cwd` — stays declared on `Cli`/`TopCommand` above and
/// is read back through `Cli::from_arg_matches`.
fn augment_with_recipes(cli: Command, file: &RecipeFile) -> Command {
    cli.mut_subcommand("run", |run_cmd| {
        let mut run_cmd = run_cmd
            .subcommand_required(true)
            .arg_required_else_help(true);
        for (name, recipe) in file.iter() {
            let mut sub = Command::new(name.as_str().to_owned());
            if !recipe.description.is_empty() {
                sub = sub.about(recipe.description.clone());
            }
            for (arg_name, arg) in &recipe.args {
                let mut a = Arg::new(arg_name.clone())
                    .long(arg_name.clone())
                    .required(true);
                if !arg.help.is_empty() {
                    a = a.help(arg.help.clone());
                }
                sub = sub.arg(a);
            }
            run_cmd = run_cmd.subcommand(sub);
        }
        run_cmd
    })
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

fn run(file: &RecipeFile, name: &str, matches: &ArgMatches, cwd: &Path) -> Result<()> {
    // `matches` came from a subcommand `augment_with_recipes` built
    // directly out of `file.iter()`, so `name` is guaranteed present.
    let recipe = file
        .get(&RecipeName::from(name))
        .with_context(|| format!("{name}: no such recipe"))?;

    // clap enforces `required(true)` on every generated arg, so each key
    // is guaranteed present; `unwrap_or_default` is only a defensive
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

    let output = recipe
        .execute(&provided, cwd)
        .with_context(|| format!("failed to run recipe `{name}`"))?;

    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    // `run` never actually returns `Ok(())` — it either bubbles an error
    // via `?` or ends the process here with the recipe's own exit code,
    // which `Result<(), E>`'s `Termination` impl couldn't express (it
    // always exits 1 on `Err`). `Result<!, E>` would state that contract
    // in the signature, but `!` as a generic argument is still unstable
    // (E0658, the `never_type` feature) — `Result<()>` is the closest
    // stable equivalent, since `process::exit`'s `!` return coerces into
    // it here regardless.
    // A missing code (killed by signal) maps to 1, matching the failure
    // code `Result<(), E>` would use anyway.
    process::exit(output.status.code().unwrap_or(1));
}
