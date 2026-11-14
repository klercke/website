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
build-site: build-web git-info
    cd site && zola build

# Serve the Zola site (run build-web first if you want /shell to work)
serve: git-info
    cd site && zola serve

# Serve the browser shell alone, with live reload
serve-web:
    cd web && trunk serve

# Run the SSH server locally on port 2222
ssh-dev:
    WEBSSH_LISTEN='[::]:2222' cargo run -p ssh

# fmt, clippy (native and wasm), tests
check:
    cargo fmt --all --check
    cargo clippy --workspace --exclude web --all-targets -- -D warnings
    cargo clippy -p web --target wasm32-unknown-unknown -- -D warnings
    cargo test --workspace --exclude web

# Write commit info for each content file to site/data/git.toml
git-info:
    #!/usr/bin/env bash
    set -euo pipefail
    {
      printf 'head = "%s"\n' "$(git rev-parse HEAD)"
      printf 'head_short = "%s"\n' "$(git rev-parse --short HEAD)"
      if git diff --quiet HEAD; then echo 'dirty = false'; else echo 'dirty = true'; fi
      printf '\n[pages]\n'
      git ls-files site/content | while read -r f; do
        git log -1 --format="\"${f#site/content/}\" = { hash = \"%H\", short = \"%h\", date = \"%cs\" }" -- "$f"
      done
    } > site/data/git.toml
