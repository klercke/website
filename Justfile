set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Run the shell in this terminal
tui:
    cargo run -p app --example local

# Build the browser shell and copy it into the Zola static dir
build-web:
    cd web && trunk build --release
    rm -rf site/static/shell
    cp -r web/dist site/static/shell

# Build the whole site into site/public
build-site: build-web
    cd site && zola build

# Serve the Zola site (run build-web first if you want /shell to work)
serve:
    cd site && zola serve

# Serve the browser shell alone, with live reload
serve-web:
    cd web && trunk serve

# Run the SSH server locally
ssh-dev:
    cargo run -p ssh

# fmt, clippy (native and wasm), tests
check:
    cargo fmt --all --check
    cargo clippy --workspace --exclude web --all-targets -- -D warnings
    cargo clippy -p web --target wasm32-unknown-unknown -- -D warnings
    cargo test --workspace --exclude web

