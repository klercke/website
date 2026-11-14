# [z3r0d4y.dev](z3r0d4y.dev)
Personal website/ resume thingy

## 404 Page

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
