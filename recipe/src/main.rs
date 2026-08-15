use recipe::{Recipe, RecipeFile, RecipeName};

use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command, Parser, Subcommand, ValueHint};

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

        /// Name of the recipe to run.
        name: String,

        /// Recipe-specific `--<param> <value>` pairs, declared by the
        /// recipe itself in `recipes.toml`. Captured raw here — which
        /// params a given recipe takes is runtime data `Cli` can't know
        /// about at parse time — and matched against that recipe's own
        /// `args` map afterward, once the file is loaded (see
        /// `parse_recipe_args`).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let file = RecipeFile::load(&cli.file).context("failed to load recipe file")?;

    match cli.command {
        TopCommand::List => list(&file),
        TopCommand::Run { cwd, name, args } => {
            let cwd =
                cwd.unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            run(&file, &name, &args, &cwd)?;
        }
    }
    Ok(())
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

fn run(file: &RecipeFile, name: &str, raw_args: &[String], cwd: &Path) -> Result<()> {
    let recipe = file
        .get(&RecipeName::from(name))
        .with_context(|| format!("{name}: no such recipe"))?;

    // A malformed `--<param>` invocation is a usage error, not one of
    // ours — let clap print its own usage message and pick the exit code.
    let matches = match parse_recipe_args(name, recipe, raw_args) {
        Ok(matches) => matches,
        Err(e) => e.exit(),
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

/// Builds a `Command` from `recipe`'s own `args` map, with `--<param>`
/// flags carrying that recipe's declared `help` text, then parses
/// `raw_args` (everything `TopCommand::Run::args` captured verbatim)
/// against it. This is the one part of the CLI that can't come from
/// `Cli`'s derive definition: a recipe's parameters are runtime data,
/// only known once `recipes.toml` is loaded.
fn parse_recipe_args(
    name: &str,
    recipe: &Recipe,
    raw_args: &[String],
) -> clap::error::Result<ArgMatches> {
    let mut cmd = Command::new(name.to_owned());
    for (arg_name, arg) in &recipe.args {
        let mut a = Arg::new(arg_name.clone())
            .long(arg_name.clone())
            .required(true);
        if !arg.help.is_empty() {
            a = a.help(arg.help.clone());
        }
        cmd = cmd.arg(a);
    }
    // clap expects the first item to be the program name; `name` itself
    // serves that purpose here, since it's already what `cmd` is named.
    cmd.try_get_matches_from(std::iter::once(name.to_owned()).chain(raw_args.iter().cloned()))
}
