# klercke.com

Source for [klercke.com](https://klercke.com):
A static site built with [Zola](https://www.getzola.org/), plus a terminal version of it you can use [in your browser](https://klercke.com/shell/) or over SSH (`ssh klercke.com`).

## Layout

| Path    | What it is                                                        |
|---------|-------------------------------------------------------------------|
| `site/` | Zola site: templates, Sass, content                               |
| `app/`  | Shared Ratatui shell UI, reads its content from `site/`           |
| `web/`  | Browser shell (Ratzilla, WebGL2), built with trunk into `/shell/` |
| `ssh/`  | SSH server for the shell (work in progress)                       |

## Development

Everything comes from the Nix dev shell (`nix develop`, or direnv with `use flake`).
It also installs the pre-commit hooks.

Most development operations can be done with `just` commands:

```
just tui
just serve-web
just build-site
just serve
just check
```

## Deployment notes

For the 404 page to work, Caddy has to serve it with a custom error handler inside the site block:

```caddyfile
handle_errors {
    @404 expression `{err.status_code} == 404`
    handle @404 {
        rewrite * /404.html
        templates {
            between [[ ]]
        }
        file_server
    }
}
```

Without this, visitors just get a default 404 page.

For Plausible Analytics, Caddy needs this block:
```caddyfile
@plausible path /js/script.js /api/event
handle @plausible {
    rewrite /js/script.js /js/pa-Ol-QVpdHOyuKkkUSi7859.js
    reverse_proxy https://plausible.io {
        header_up Host {http.reverse_proxy.upstream.hostport}
    }
}
```

## License

This repository follows the [REUSE](https://reuse.software/) specification;
`REUSE.toml` has the details.

- **Code:**
    [AGPL-3.0-or-later](LICENSES/AGPL-3.0-or-later.txt)
- **Writing** in `site/content/`:
    [CC BY-SA 4.0](LICENSES/CC-BY-SA-4.0.txt).
    Code snippets within the writing are available under [0BSD](https://spdx.org/licenses/0BSD.html).
- **Data files** (lockfiles, PGP key, `security.txt`, hall of fame):
    [CC0 1.0](LICENSES/CC0-1.0.txt)
