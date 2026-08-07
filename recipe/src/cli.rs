use clap::{Parser, Subcommand, ValueHint};

use std::path::PathBuf;

/// Runs recipes declared in a `recipes.toml` file — a minimal,
/// shell-free alternative to `just`.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path to the recipe file.
    #[clap(long, default_value = "recipes.toml", value_hint = ValueHint::FilePath)]
    pub file: PathBuf,

    /// Directory recipes run in. Defaults to the recipe file's own
    /// directory.
    #[clap(long, value_hint = ValueHint::DirPath)]
    pub cwd: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List every recipe this file declares.
    List,
    /// Run one recipe by name, passing its arguments in declared order.
    Run {
        name: String,
        #[clap(trailing_var_arg = true)]
        args: Vec<String>,
    },
}
