xdg_bin_home := env('XDG_BIN_HOME', env('HOME') + "/.local/bin")
arch := "aarch64"
libc := "musl"

# --- linting ---

# Full workspace build check (catches cross-crate issues)
[group('lint')]
check:
    cargo check --all-targets

# Run Clippy
[group('lint')]
lint:
    cargo clippy

# --- test ---

# Run all tests (unit tests + Cucumber-BDD integration tests)
[group('test')]
test: test-unit

# Unit tests (model.rs, resources.rs)
[group('test')]
test-unit:
    cargo test --lib

# Run a single unit test by name
[group('test')]
test-one name:
    cargo test --lib -- {{name}}

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

[group('build-release')]
release-cross:
    cross build --target {{arch}}-unknown-linux-{{libc}} --release --locked
