//! OPML import and export.
//!
//! Import rules (spec: "OPML import"): nested outlines are flattened into their top-level
//! Folder, and a feed URL that appears more than once keeps its first occurrence.

use std::collections::HashSet;

use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpmlFeed {
    pub url: String,
    pub title: String,
    pub site_url: Option<String>,
    /// Top-level Folder name; `None` for Feeds at the top level.
    pub folder: Option<String>,
}

pub fn parse(xml: &str) -> Result<Vec<OpmlFeed>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // One entry per open <outline>: Some(name) for Folders, None for anything else.
    let mut stack: Vec<Option<String>> = Vec::new();
    let mut seen = HashSet::new();
    let mut feeds = Vec::new();
    let mut saw_opml = false;

    loop {
        let event = reader
            .read_event()
            .map_err(|e| Error::Opml(e.to_string()))?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let is_empty = matches!(event, Event::Empty(_));
                match e.local_name().as_ref() {
                    "opml" => saw_opml = true,
                    "outline" => {
                        let attrs = Attrs::read(e)?;
                        if let Some(url) = attrs.xml_url.filter(|u| !u.trim().is_empty()) {
                            let url = url.trim().to_owned();
                            if seen.insert(url.clone()) {
                                feeds.push(OpmlFeed {
                                    title: attrs.title.unwrap_or_else(|| url.clone()),
                                    url,
                                    site_url: attrs.html_url.filter(|u| !u.trim().is_empty()),
                                    folder: stack.iter().flatten().next().cloned(),
                                });
                            }
                            if !is_empty {
                                stack.push(None);
                            }
                        } else if !is_empty {
                            stack.push(attrs.title.filter(|t| !t.trim().is_empty()));
                        }
                    }
                    _ => {}
                }
            }
            Event::End(ref e) if e.local_name().as_ref() == "outline" => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !saw_opml {
        return Err(Error::Opml("no <opml> element".into()));
    }
    Ok(feeds)
}

struct Attrs {
    xml_url: Option<String>,
    html_url: Option<String>,
    title: Option<String>,
}

impl Attrs {
    fn read(e: &BytesStart) -> Result<Self> {
        let (mut xml_url, mut html_url, mut text, mut title) = (None, None, None, None);
        for attr in e.attributes().flatten() {
            let value = attr
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|e| Error::Opml(e.to_string()))?
                .into_owned();
            match attr.key.local_name().as_ref() {
                "xmlUrl" | "xmlurl" => xml_url = Some(value),
                "htmlUrl" | "htmlurl" => html_url = Some(value),
                "text" => text = Some(value),
                "title" => title = Some(value),
                _ => {}
            }
        }
        Ok(Self {
            xml_url,
            html_url,
            title: title.filter(|t| !t.trim().is_empty()).or(text),
        })
    }
}

/// A Folder with its Feeds, for export.
pub struct ExportFolder<'a> {
    pub name: &'a str,
    pub feeds: Vec<ExportFeed<'a>>,
}

pub struct ExportFeed<'a> {
    pub title: &'a str,
    pub url: &'a str,
    pub site_url: Option<&'a str>,
}

pub fn export(folders: &[ExportFolder], top_level: &[ExportFeed]) -> String {
    fn esc(s: &str) -> String {
        quick_xml::escape::escape(s).into_owned()
    }
    fn feed(out: &mut String, f: &ExportFeed, indent: &str) {
        out.push_str(&format!(
            "{indent}<outline type=\"rss\" text=\"{t}\" title=\"{t}\" xmlUrl=\"{u}\"",
            t = esc(f.title),
            u = esc(f.url)
        ));
        if let Some(site) = f.site_url {
            out.push_str(&format!(" htmlUrl=\"{}\"", esc(site)));
        }
        out.push_str("/>\n");
    }
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n  <head><title>Feedrico</title></head>\n  <body>\n",
    );
    for folder in folders {
        out.push_str(&format!(
            "    <outline text=\"{n}\" title=\"{n}\">\n",
            n = esc(folder.name)
        ));
        for f in &folder.feeds {
            feed(&mut out, f, "      ");
        }
        out.push_str("    </outline>\n");
    }
    for f in top_level {
        feed(&mut out, f, "    ");
    }
    out.push_str("  </body>\n</opml>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<opml version="2.0">
  <head><title>Subs</title></head>
  <body>
    <outline text="Tech" title="Tech">
      <outline type="rss" text="A &amp; B" xmlUrl="https://a.example/feed" htmlUrl="https://a.example"/>
      <outline text="Nested">
        <outline type="rss" text="Deep" xmlUrl="https://deep.example/rss"/>
      </outline>
    </outline>
    <outline type="rss" text="Loose" xmlUrl="https://loose.example/atom.xml"/>
    <outline text="Dup folder">
      <outline type="rss" text="A again" xmlUrl="https://a.example/feed"/>
    </outline>
    <outline text="Empty folder"/>
  </body>
</opml>"#;

    #[test]
    fn flattens_nested_and_dedupes() {
        let feeds = parse(SAMPLE).unwrap();
        assert_eq!(feeds.len(), 3);
        assert_eq!(feeds[0].title, "A & B");
        assert_eq!(feeds[0].folder.as_deref(), Some("Tech"));
        assert_eq!(feeds[0].site_url.as_deref(), Some("https://a.example"));
        assert_eq!(feeds[1].url, "https://deep.example/rss");
        assert_eq!(
            feeds[1].folder.as_deref(),
            Some("Tech"),
            "nested → top-level Folder"
        );
        assert_eq!(feeds[2].folder, None);
    }

    #[test]
    fn rejects_non_opml() {
        assert!(parse("<rss><channel/></rss>").is_err());
        assert!(parse("not xml at all <<<").is_err());
    }

    #[test]
    fn export_round_trips() {
        let xml = export(
            &[ExportFolder {
                name: "Tech & Co",
                feeds: vec![ExportFeed {
                    title: "A",
                    url: "https://a.example/feed?x=1&y=2",
                    site_url: None,
                }],
            }],
            &[ExportFeed {
                title: "Loose",
                url: "https://loose.example/rss",
                site_url: Some("https://loose.example"),
            }],
        );
        let feeds = parse(&xml).unwrap();
        assert_eq!(feeds.len(), 2);
        assert_eq!(feeds[0].url, "https://a.example/feed?x=1&y=2");
        assert_eq!(feeds[0].folder.as_deref(), Some("Tech & Co"));
        assert_eq!(feeds[1].folder, None);
    }
}
