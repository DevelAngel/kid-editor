xdg_bin_home := env('XDG_BIN_HOME', env('HOME') + "/.local/bin")
arch := "aarch64"
libc := "musl"

# --- linting ---

# Full workspace build check • catches cross-crate issues
[group('lint')]
check:
    cargo check --all-targets

# Run Clippy
[group('lint')]
lint:
    cargo clippy

# --- test ---

# Run all tests
[group('test')]
test:
    cargo test

# Run a single test by name
[group('test')]
test-one name:
    cargo test -- {{name}}

# --- debug build ---

# Build with debug symbols
[group('build-debug')]
debug-native:
    cargo build

# --- release build ---

# Build a release
[group('build-release')]
release-native:
    cargo build --release --locked

# Build a release for a specific arch and libc
[private]
[group('build-release')]
release-cross:
    cross build --target {{arch}}-unknown-linux-{{libc}} --release --locked

# --- git ---

# Show the working tree status
[group('git')]
[arg('args', help="more arguments")]
git-status *args:
    @git status {{args}}

# Show changes between commits, commit and working tree, etc
[group('git')]
[arg('args', help="more arguments")]
git-diff *args:
    @git diff {{args}}

# Show commit logs
[group('git')]
[arg('args', help="more arguments")]
git-log *args:
    @git log --no-color --graph --pretty=format:'%h • %s (%(decorate:prefix=,suffix= • )%cr)' {{args}}

# Add file contents to the index
[group('git')]
[arg('args', help="pathspecs and more arguments")]
git-add +args:
    @git add {{args}}

# Record changes to the repository
[group('git')]
[arg('message', help="commit message")]
[arg('args', help="more arguments")]
git-commit message *args:
    @git commit --message="{{message}}" {{args}}

# Apply the changes introduced by an existing commit
[group('git')]
[arg('commit', help="commit to cherry-pick")]
git-cherry-pick commit:
    @git cherry-pick {{commit}}

# Continue a cherry-pick after resolving conflicts
[group('git')]
git-cherry-pick-continue:
    @git cherry-pick --continue

# Abort an in-progress cherry-pick
[group('git')]
git-cherry-pick-abort:
    @git cherry-pick --abort

# Skip the current commit during a cherry-pick
[group('git')]
git-cherry-pick-skip:
    @git cherry-pick --skip

# Rebase the range (base..HEAD) onto a new target
[confirm]
[group('git')]
[arg('target', help="new base to rebase onto")]
[arg('base', help="old base, exclusive start of the range")]
git-rebase-onto target base:
    @git rebase --onto {{target}} {{base}}

# Continue a rebase after resolving conflicts
[group('git')]
git-rebase-continue:
    @git rebase --continue

# Abort an in-progress rebase
[group('git')]
git-rebase-abort:
    @git rebase --abort

# Squash the last n commits into one, keeping changes staged
[confirm]
[group('git')]
[arg('n', help="number of commits to squash")]
[arg('message', help="commit message for the squashed commit")]
git-squash-last n message:
    @git reset --soft HEAD~{{n}}
    @git commit --message="{{message}}"
