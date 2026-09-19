mod add;
mod commit;
mod diff;
mod log;
mod mv;
mod pull;
mod push;
mod reset_soft;
mod restore;
mod rm;
mod shared;
mod status;
mod switch;

use super::McpService;
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
pub(super) fn service(root: &Path) -> McpService {
    McpService::new(
        root.to_path_buf(),
        vec![],
        recipe::RecipeFile::default(),
        None,
        false,
    )
}

#[cfg(test)]
pub(super) fn git(root: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(status.status.success(), "git failed: {:?}", status);
}
