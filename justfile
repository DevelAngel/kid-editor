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
