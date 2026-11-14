//! Content for the terminal UI.
//! The actual copy is loaded from Zola so the different interfaces stay in sync.

pub struct File {
    pub name: &'static str,
    pub body: &'static str,
}

pub const FILES: &[File] = &[
    File {
        name: "about",
        body: include_str!("../../site/content/_index.md"),
    },
    File {
        name: "projects",
        body: include_str!("../../site/content/projects.md"),
    },
    File {
        name: "links",
        body: include_str!("../../site/content/links.md"),
    },
    File {
        name: "security.txt",
        body: include_str!("../../site/static/.well-known/security.txt"),
    },
    File {
        name: "hall-of-fame.txt",
        body: include_str!("../../site/static/hall-of-fame.txt"),
    },
    File {
        name: "pgp-key.txt",
        body: include_str!("../../site/static/pgp-key.txt"),
    },
];

/// Pages `open` knows about, as (name, path on the site).
pub const PAGES: &[(&str, &str)] = &[
    ("home", "/"),
    ("projects", "/projects/"),
    ("links", "/links/"),
    ("contact", "/contact/"),
];

pub const SITE: &str = "https://klercke.com";

#[must_use]
pub fn file(name: &str) -> Option<&'static File> {
    FILES.iter().find(|f| f.name == name)
}

/// Drop a leading `+++ ... +++` Zola front matter block, if there is one.
#[must_use]
pub fn strip_front_matter(body: &str) -> &str {
    let Some(rest) = body.strip_prefix("+++\n") else {
        return body;
    };
    rest.find("\n+++\n")
        .map_or(body, |end| rest[end + 5..].trim_start_matches('\n'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_front_matter() {
        assert_eq!(
            strip_front_matter("+++\ntitle = \"x\"\n+++\n\nhi\n"),
            "hi\n"
        );
        assert_eq!(strip_front_matter("no front matter"), "no front matter");
    }

    #[test]
    fn every_file_has_content() {
        for f in FILES {
            assert!(!strip_front_matter(f.body).trim().is_empty(), "{}", f.name);
        }
    }
}
