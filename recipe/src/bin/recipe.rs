use recipe::cli::{Cli, Command};
use recipe::{RecipeFile, RecipeName};

use clap::Parser;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();

    let file = match RecipeFile::load(&cli.file) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let cwd = cli.cwd.unwrap_or_else(|| default_cwd(&cli.file));

    match cli.command {
        Command::List => {
            list(&file);
            ExitCode::SUCCESS
        }
        Command::Run { name, args } => run(&file, &name, &args, &cwd),
    }
}

/// Recipes run relative to the recipe file's own directory when `--cwd`
/// isn't given — `recipes.toml` in a project root should behave like
/// running commands from that root.
fn default_cwd(recipe_file: &std::path::Path) -> PathBuf {
    recipe_file
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
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

fn run(file: &RecipeFile, name: &str, args: &[String], cwd: &std::path::Path) -> ExitCode {
    let Some(recipe) = file.get(&RecipeName::from(name)) else {
        eprintln!("{name}: no such recipe");
        return ExitCode::FAILURE;
    };

    match recipe.execute(args, cwd) {
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
