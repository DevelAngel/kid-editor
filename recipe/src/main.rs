use recipe::{RecipeFile, RecipeName};

use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command, CommandFactory, FromArgMatches, Parser, ValueHint};

use std::env;
use std::path::{Path, PathBuf};
use std::process;

/// Runs recipes declared in a `recipes.toml` file — a minimal, shell-free
/// alternative to `just`.
#[derive(Parser, Debug)]
#[command(name = "kid-recipes", version)]
struct Cli {
    /// Path to the recipe file.
    #[arg(long, default_value = "recipes.toml", value_hint = ValueHint::FilePath)]
    file: PathBuf,

    /// Directory the recipe runs in. Defaults to the current working
    /// directory.
    #[arg(long, value_hint = ValueHint::DirPath)]
    cwd: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = {
        let cli = Cli::command().disable_help_flag(true).ignore_errors(true);
        Cli::from_arg_matches(&cli.get_matches()).unwrap_or_else(|e| e.exit())
    };
    if !cli.file.exists() {
        let cli = Cli::command()
            .subcommand_required(true)
            .arg_required_else_help(true)
            .subcommand(Command::new("...").about("Commands from recipe file"));
        Cli::from_arg_matches(&cli.get_matches())?;
    }
    let file = RecipeFile::load(&cli.file)
        .context(format!("failed to load recipe file {}", cli.file.display()))?;
    let matches = Cli::command().augment_with_recipes(&file).get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    let cwd = cli
        .cwd
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let Some((name, recipe_matches)) = matches.subcommand() else {
        unreachable!("`augment_with_recipes` sets subcommand_required(true)");
    };
    run(&file, name, recipe_matches, &cwd)?;
    Ok(())
}

/// Extension trait so `augment_with_recipes` reads as part of the same
/// fluent builder chain as clap's own `Command` methods
/// (`Cli::command().augment_with_recipes(&file)`), the way `clap::Args`
/// extends `Command` with `augment_args`.
trait AugmentWithRecipes {
    /// Appends one subcommand per declared recipe, with `--<param>` flags
    /// generated from that recipe's own `args` map (name, `help` text,
    /// and `about` from the recipe's `description` included — `--help`
    /// doubles as the recipe listing, so there's no separate `list`
    /// command). This is the one part of the CLI that can't come from
    /// derive: recipe names and their parameters are runtime data, only
    /// known once `recipes.toml` is loaded. Everything else — `--file`,
    /// `--cwd` — stays declared on `Cli` and is read back through
    /// `Cli::from_arg_matches`.
    fn augment_with_recipes(self, file: &RecipeFile) -> Self;
}

impl AugmentWithRecipes for Command {
    fn augment_with_recipes(self, file: &RecipeFile) -> Self {
        let mut cli = self.subcommand_required(true).arg_required_else_help(true);
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
            cli = cli.subcommand(sub);
        }
        cli
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
