# Read straight from the manifest so this never drifts from the real MSRV.
msrv := `grep -m1 '^rust-version' Cargo.toml | cut -d'"' -f2`
sample := "tests/fixtures/no-intro/virtual-boy.dat"

# List available recipes.
default:
    @just --list

# Build the library with all features.
build:
    cargo build --all-features

# Run the test suite. Extra args are passed to cargo test.
test *ARGS:
    cargo test --all-features {{ ARGS }}

# Type-check everything without producing binaries.
check:
    cargo check --all-targets --all-features

# Format the source in place.
fmt:
    cargo fmt --all

# Verify formatting without changing anything.
fmt-check:
    cargo fmt --all --check

# Run clippy over the library, tests and examples.
clippy:
    cargo clippy --all-targets --all-features

# Build the documentation, denying warnings. Pass --open to view it.
doc *ARGS:
    RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps {{ ARGS }}

# Check every supported combination of feature flags.
features:
    cargo check --lib --no-default-features
    cargo check --lib --no-default-features --features index
    cargo check --lib --no-default-features --features verify
    cargo check --all-targets --all-features

# Check against the minimum supported Rust version.
msrv:
    @rustup toolchain list | grep -q '{{ msrv }}' \
        || (echo "installing Rust {{ msrv }}..." && rustup toolchain install {{ msrv }} --profile minimal)
    cargo +{{ msrv }} check --all-features

# Everything CI runs, cheapest checks first.
ci: fmt-check clippy test features doc msrv
    @echo "\nall checks passed"

# Build the examples.
examples:
    cargo build --examples --all-features

# Summarise a datafile. Defaults to a bundled fixture.
info FILE=sample:
    cargo run --quiet --example info -- {{ FILE }}

# Check a directory of ROMs against a datafile.
scan DAT DIR:
    cargo run --quiet --release --example scan -- {{ DAT }} {{ DIR }}

# Dry-run the crates.io release.
publish-dry:
    cargo publish --dry-run --all-features

# Publish to crates.io. Runs the full CI suite first.
publish: ci
    cargo publish

# Update dependencies within their semver ranges.
update:
    cargo update

# Remove build artifacts.
clean:
    cargo clean
