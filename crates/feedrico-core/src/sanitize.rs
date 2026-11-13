//! Turning untrusted feed HTML into something safe to render (spec: "Content safety and
//! privacy").
//!
//! - iframes, embeds and videos become link cards ("Embedded content on youtube.com").
//! - Everything executable or layout-breaking is removed by ammonia's allow-list.
//! - Relative URLs are resolved against the Article's URL.
//! - Image sources are rewritten to the local media cache, so rendering an Article never makes
//!   a network request. The original URLs are returned so the cache can download them.

use std::cell::RefCell;
use std::collections::HashSet;

use lol_html::html_content::ContentType;
use lol_html::{RewriteStrSettings, element, rewrite_str};
use url::Url;

pub struct Sanitized {
    pub html: String,
    /// Absolute image URLs referenced by the content, in order of appearance.
    pub images: Vec<String>,
}

/// Name of a cached media file: a hash of the source URL, so the reader can reference an image
/// before it is downloaded and the cache can fill in later.
pub fn media_key(source_url: &str) -> String {
    blake3::hash(source_url.as_bytes()).to_hex()[..32].to_owned()
}

/// The MathML Core element set (W3C), minus `annotation-xml` and `mglyph`: see the comment at
/// the allow-list in [`sanitize`] for why those two stay out.
const MATHML_TAGS: &[&str] = &[
    "math",
    "semantics",
    "annotation",
    "merror",
    "mfrac",
    "mi",
    "mmultiscripts",
    "mn",
    "mo",
    "mover",
    "mpadded",
    "mphantom",
    "mprescripts",
    "mroot",
    "mrow",
    "ms",
    "mspace",
    "msqrt",
    "mstyle",
    "msub",
    "msubsup",
    "msup",
    "mtable",
    "mlabeledtr",
    "mtd",
    "mtext",
    "mtr",
    "munder",
    "munderover",
    "none",
    "maction",
    "menclose",
    "mfenced",
    "mstack",
    "msrow",
    "msgroup",
    "msline",
    "mscarry",
    "mscarries",
];

/// `media_prefix` is where the shell serves cached media, e.g. `media://localhost/`.
pub fn sanitize(html: &str, base: Option<&Url>, media_prefix: &str) -> Sanitized {
    let images = RefCell::new(Vec::<String>::new());
    let resolve = |raw: &str| -> Option<Url> {
        let raw = raw.trim();
        match base {
            Some(base) => base.join(raw).ok(),
            None => Url::parse(raw).ok(),
        }
        .filter(|u| matches!(u.scheme(), "http" | "https"))
    };

    let embed_card = |src: Option<String>| -> String {
        match src.as_deref().and_then(resolve) {
            Some(url) => {
                let host = url
                    .host_str()
                    .unwrap_or("the web")
                    .trim_start_matches("www.");
                format!(
                    r#"<p class="embed-link"><a href="{}">Embedded content on {}: open in browser</a></p>"#,
                    escape_attr(url.as_str()),
                    escape_text(host)
                )
            }
            None => String::new(),
        }
    };

    // Pass 1: structural rewrites that an allow-list can't express.
    let rewritten = rewrite_str(
        html,
        RewriteStrSettings::new()
            .append_element_content_handler(element!("iframe, embed, object, video, audio", |el| {
                let src = el.get_attribute("src").or_else(|| el.get_attribute("data"));
                el.replace(&embed_card(src), ContentType::Html);
                Ok(())
            }))
            .append_element_content_handler(element!("img", |el| {
                let src = el
                    .get_attribute("src")
                    .or_else(|| el.get_attribute("data-src"));
                match src.as_deref().and_then(resolve) {
                    Some(url) => {
                        // Inert: the webview never loads data-* attributes. The reader links
                        // to it when the image isn't cached (spec: "stays a remote link").
                        if matches!(url.scheme(), "http" | "https") {
                            el.set_attribute("data-remote", url.as_str())?;
                        } else {
                            el.remove_attribute("data-remote");
                        }
                        let url = url.to_string();
                        el.set_attribute("src", &format!("{media_prefix}{}", media_key(&url)))?;
                        images.borrow_mut().push(url);
                    }
                    None => el.remove(),
                }
                for attr in ["srcset", "sizes", "data-src", "loading"] {
                    el.remove_attribute(attr);
                }
                Ok(())
            })),
    )
    .unwrap_or_default();

    // Pass 2: allow-list. Image sources now point at the media cache and must survive.
    let mut builder = ammonia::Builder::default();
    builder
        .add_url_schemes(["media"])
        .link_rel(Some("noopener noreferrer"))
        .add_allowed_classes("p", ["embed-link"])
        .rm_tags(["picture", "source"])
        .add_tag_attributes("img", ["alt", "title", "data-remote"])
        // Code blocks keep their GFM language class (issue #77, miniflux/v2#4587) so the reader
        // can highlight them locally. `class` must be whitelisted on the tag for ammonia to hand
        // it to the attribute filter at all; the filter then lets only language-*/lang-* tokens
        // through, on pre/code only. Every other class stays stripped as before.
        .add_tag_attributes("pre", ["class"])
        .add_tag_attributes("code", ["class"])
        .attribute_filter(|tag, attr, value| {
            if attr == "class" && matches!(tag, "pre" | "code") {
                let kept: Vec<&str> = value
                    .split_ascii_whitespace()
                    .filter(|c| is_language_class(c))
                    .collect();
                (!kept.is_empty()).then(|| kept.join(" ").into())
            } else {
                Some(value.into())
            }
        })
        // MathML (issue #78, ADR 0004): the MathML Core element set, minus `annotation-xml` (its
        // encoding="text/html" can smuggle in HTML) and `mglyph`. The set is inert — with no URL
        // attributes allowed, maction's actiontype/href and every event handler are stripped like
        // everywhere else. Only plain-text attributes survive: alttext (the accessible name screen
        // readers voice for role "math") and display (block vs inline) on math itself.
        .add_tags(MATHML_TAGS)
        .add_tag_attributes("math", ["alttext", "display"]);
    if let Some(base) = base {
        builder.url_relative(ammonia::UrlRelative::RewriteWithBase(base.clone()));
    }
    let html = builder.clean(&rewritten).to_string();

    let mut seen = HashSet::new();
    let images = images
        .into_inner()
        .into_iter()
        .filter(|u| seen.insert(u.clone()))
        .collect();
    Sanitized { html, images }
}

/// A GFM code-block language class token: `language-rust`, `lang-py`, `language-c++`, … The id is
/// limited to characters that are safe in a CSS class selector and to a sane length, so a class
/// value can never smuggle markup or an absurd string through the allow-list.
fn is_language_class(class: &str) -> bool {
    let Some(id) = class
        .strip_prefix("language-")
        .or_else(|| class.strip_prefix("lang-"))
    else {
        return false;
    };
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'+' | b'#' | b'.'))
}

/// Plain text of an HTML fragment, for search, excerpts and titles.
pub fn plain_text(html: &str) -> String {
    let stripped = ammonia::Builder::empty().clean(html).to_string();
    let decoded = decode_entities(&stripped);
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn excerpt(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let cut: String = text.chars().take(max_chars).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{}…", cut.trim_end_matches([',', '.', ';', ':']))
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        // Entities are short; look a few characters ahead (char-wise: text may be multi-byte).
        let Some(end) = rest
            .char_indices()
            .take(12)
            .find_map(|(i, c)| (c == ';').then_some(i))
        else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => entity[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFIX: &str = "media://localhost/";

    fn clean(html: &str) -> Sanitized {
        let base = Url::parse("https://blog.example/posts/1").unwrap();
        sanitize(html, Some(&base), PREFIX)
    }

    #[test]
    fn removes_scripts_handlers_and_forms() {
        let out = clean(
            r#"<p onclick="evil()">Hi<script>alert(1)</script></p><form action="/x"><input></form><a href="javascript:alert(1)">x</a>"#,
        );
        assert!(!out.html.contains("script"));
        assert!(!out.html.contains("onclick"));
        assert!(!out.html.contains("<form"));
        assert!(!out.html.contains("<input"));
        assert!(!out.html.contains("javascript:"));
        assert!(out.html.contains("Hi"));
    }

    #[test]
    fn iframes_become_link_cards() {
        let out = clean(r#"<iframe src="https://www.youtube.com/embed/abc"></iframe>"#);
        assert!(out.html.contains(r#"class="embed-link""#), "{}", out.html);
        assert!(out.html.contains("Embedded content on youtube.com"));
        assert!(out.html.contains("https://www.youtube.com/embed/abc"));
        assert!(!out.html.contains("<iframe"));
    }

    #[test]
    fn images_point_at_the_local_cache() {
        let out =
            clean(r#"<img src="/img/a.png" srcset="b.png 2x" alt="A"><img src="/img/a.png">"#);
        let url = "https://blog.example/img/a.png";
        assert_eq!(
            out.images,
            vec![url.to_owned()],
            "resolved and de-duplicated"
        );
        assert!(
            out.html.contains(&format!("{PREFIX}{}", media_key(url))),
            "{}",
            out.html
        );
        assert!(
            !without_remote(&out.html).contains("blog.example/img"),
            "no remote image URL left outside data-remote"
        );
        assert!(!out.html.contains("srcset"));
        assert!(out.html.contains(r#"alt="A""#));
    }

    /// Drops the inert `data-remote` attributes, the only place a remote image URL may stay.
    fn without_remote(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;
        while let Some(start) = rest.find(" data-remote=\"") {
            out.push_str(&rest[..start]);
            let after = &rest[start + " data-remote=\"".len()..];
            rest = after.find('"').map_or("", |end| &after[end + 1..]);
        }
        out.push_str(rest);
        out
    }

    // Spec: an image that isn't cached (over 5 MB, evicted) "stays a remote link". The original
    // URL rides along in an attribute the webview never loads; the reader turns a missing image
    // into a link to it.
    #[test]
    fn images_keep_their_original_url_for_a_remote_link() {
        let out = clean(
            r#"<img src="/img/a.png" data-remote="https://evil.example/x"><img src="data:image/png;base64,AAAA">"#,
        );
        assert!(
            out.html
                .contains(r#"data-remote="https://blog.example/img/a.png""#),
            "{}",
            out.html
        );
        assert!(
            !out.html.contains("evil.example"),
            "feed can't set it: {}",
            out.html
        );
        assert_eq!(
            out.html.matches("data-remote").count(),
            1,
            "only http(s) URLs: {}",
            out.html
        );
    }

    #[test]
    fn sanitized_content_never_loads_anything_remote() {
        // Reading must not hit the network: every other way to load a resource is gone.
        let out = clean(
            r#"<picture><source srcset="https://t.example/a.webp 1x" type="image/webp">
               <img src="https://t.example/a.jpg" srcset="https://t.example/a2.jpg 2x"></picture>
               <img data-src="https://t.example/lazy.png" loading="lazy">
               <img srcset="https://t.example/only-srcset.png 2x">
               <svg><image href="https://t.example/s.png"/></svg>
               <p style="background:url(https://t.example/bg.png)">x</p>"#,
        );
        let html = &without_remote(&out.html);
        assert!(!html.contains("t.example"), "{html}");
        assert!(
            !html.contains("srcset") && !html.contains("<source"),
            "{html}"
        );
        for src in html.split("src=\"").skip(1) {
            assert!(src.starts_with(PREFIX), "non-local src in {html}");
        }
    }

    #[test]
    fn links_resolve_and_get_rel() {
        let out = clean(r#"<a href="../about">About</a>"#);
        assert!(
            out.html.contains(r#"href="https://blog.example/about""#),
            "{}",
            out.html
        );
        assert!(out.html.contains("noopener"));
    }

    #[test]
    fn style_attributes_are_dropped() {
        let out = clean(r#"<p style="position:fixed;inset:0">x</p>"#);
        assert!(!out.html.contains("style"));
    }

    // Issue #77 (miniflux/v2#4587): GFM code blocks carry the language in a class on pre/code.
    #[test]
    fn keeps_language_class_on_pre_and_code() {
        let out = clean(
            r#"<pre><code class="language-rust">fn main() { println!("hi"); }</code></pre><pre class="lang-python">print(1)</pre>"#,
        );
        assert!(
            out.html.contains(r#"<code class="language-rust">"#),
            "{}",
            out.html
        );
        assert!(
            out.html.contains(r#"<pre class="lang-python">"#),
            "{}",
            out.html
        );
        // The code text itself is untouched.
        assert!(
            out.html.contains(r#"fn main() { println!("hi"); }"#),
            "{}",
            out.html
        );
    }

    #[test]
    fn strips_every_other_class() {
        let out = clean(
            r#"<pre class="highlight"><code class="token keyword">x</code></pre><p class="language-rust">y</p><span class="language-js">z</span><code class="language-c++ bad!">w</code><code class="language-rust&lt;script&gt;">v</code>"#,
        );
        // Server-side highlighted markup (Prism/Pygments spans) loses its classes as today.
        assert!(!out.html.contains("highlight"), "{}", out.html);
        assert!(!out.html.contains("token"), "{}", out.html);
        // language-* is allowed on pre/code only, never on other tags.
        assert!(
            !out.html.contains(r#"<p class="language-rust">"#),
            "{}",
            out.html
        );
        assert!(
            !out.html.contains(r#"<span class="language-js">"#),
            "{}",
            out.html
        );
        // Mixed values keep only the language token; unsafe "languages" are dropped entirely.
        assert!(out.html.contains(r#"class="language-c++""#), "{}", out.html);
        assert!(!out.html.contains("language-rust&lt;"), "{}", out.html);
    }

    #[test]
    fn code_block_text_still_reaches_plain_text() {
        // FTS reads content_text = plain_text(sanitized html); the class allow-list must not
        // change it, so search still finds code content.
        let raw = r#"<p>Intro</p><pre><code class="language-rust">fn main() { println!("hi"); }</code></pre>"#;
        let out = clean(raw);
        assert_eq!(plain_text(raw), plain_text(&out.html));
        assert_eq!(
            plain_text(&out.html),
            "Introfn main() { println!(\"hi\"); }"
        );
    }

    #[test]
    fn decoding_survives_multibyte_text_after_ampersands() {
        assert_eq!(
            plain_text("Tom & Jérôme — 🍯🍯🍯🍯🍯 grüßt"),
            "Tom & Jérôme — 🍯🍯🍯🍯🍯 grüßt"
        );
        assert_eq!(plain_text("&äöü&#228;"), "&äöüä");
    }

    #[test]
    fn plain_text_and_excerpt() {
        assert_eq!(
            plain_text("<p>Fish &amp; chips&nbsp;&#8212; <b>good</b></p><script>x()</script>"),
            "Fish & chips — good"
        );
        assert_eq!(excerpt("one two three four", 9), "one two…");
        assert_eq!(excerpt("short", 9), "short");
    }

    // Issue #78 / ADR 0004: pre-baked MathML survives sanitizing. The MathML Core element set is
    // inert, and it is the format screen readers can navigate and voice (role "math", name from
    // alttext), so equations must not vanish from Articles.
    const MATHML_FIXTURE: &str = r#"<math alttext="E equals m c squared" display="block"><semantics><mrow><mi>E</mi><mo>=</mo><mi>m</mi><msup><mi>c</mi><mn>2</mn></msup></mrow><annotation>E = mc^2</annotation></semantics></math><p>Inline <math alttext="a squared plus b squared equals c squared"><mrow><msup><mi>a</mi><mn>2</mn></msup><mo>+</mo><msup><mi>b</mi><mn>2</mn></msup><mo>=</mo><msup><mi>c</mi><mn>2</mn></msup></mrow></math> in text.</p>"#;

    #[test]
    fn mathml_survives_sanitizing() {
        let out = clean(MATHML_FIXTURE);
        for expected in [
            "<math",
            r#"alttext="E equals m c squared""#,
            r#"display="block""#,
            "<semantics",
            "<mrow",
            "<mi>E</mi>",
            "<mo>=</mo>",
            "<msup",
            "<mn>2</mn>",
            "<annotation",
            "E = mc^2",
        ] {
            assert!(
                out.html.contains(expected),
                "missing {expected} in {}",
                out.html
            );
        }
    }

    #[test]
    fn mathml_introduces_no_script_handler_or_url_vector() {
        let out = clean(
            r#"<math onclick="alert(1)" onmouseover="alert(2)" style="position:fixed;inset:0" class="katex" href="javascript:alert(3)"><maction actiontype="statusline" href="javascript:alert(4)">x</maction><mtext><script>alert(5)</script>click</mtext></math><math><semantics><mrow><mi>y</mi></mrow><annotation-xml encoding="text/html"><img src="https://evil.example/x" onerror="alert(6)"></annotation-xml></semantics></math>"#,
        );
        let html = &out.html;
        for banned in [
            "onclick",
            "onmouseover",
            "onerror",
            "javascript:",
            "style=",
            "class=",
            "actiontype",
            "href=",
            "<script",
            "<annotation-xml",
            "evil.example",
        ] {
            assert!(!html.contains(banned), "found {banned} in {html}");
        }
        // The inert elements and their text survive.
        assert!(html.contains("<math"), "{html}");
        assert!(html.contains("<maction"), "{html}");
        assert!(html.contains("<mtext"), "{html}");
        assert!(
            html.contains("click") && html.contains('x') && html.contains('y'),
            "{html}"
        );
    }

    #[test]
    fn raw_tex_delimiters_stay_literal_selectable_text() {
        // ADR 0004: no bundled renderer in v1, no CDN per the spec network policy — $…$, $$…$$,
        // \(…\) and \[…\] stay plain text the user can select and copy.
        let out = clean(
            r#"<p>Inline $E = mc^2$ and \(a^2 + b^2 = c^2\), display $$x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$$ or \[ \int_0^1 x\,dx \].</p>"#,
        );
        for literal in [
            "$E = mc^2$",
            r"\(a^2 + b^2 = c^2\)",
            r"$$x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$$",
            r"\[ \int_0^1 x\,dx \]",
        ] {
            assert!(
                out.html.contains(literal),
                "missing {literal} in {}",
                out.html
            );
        }
        assert!(
            !out.html.contains("<math"),
            "delimiters are not turned into elements: {}",
            out.html
        );
    }
}
