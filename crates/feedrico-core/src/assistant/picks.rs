//! Assistant picks (#121): rank the unread Articles of a View against the user's interests.
//!
//! The interests are free text in the machine-local Assistant setup, never in the Library. Input
//! is title, Feed name and excerpt per Article (see [`super::view_input`]), in batches with a
//! token (character) cap — at the Archivist's scale only the newest [`CAP`] per run are ranked,
//! and the result says how many were considered. Each pick gets a score and a one-line reason.
//! The ranking is ephemeral: it never changes Read or Starred and never reorders anything on its
//! own; the chronological list stays the default.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::articles::ListQuery;
use crate::{Error, Library, Result};

use super::prompts::{SYSTEM_PREAMBLE, answer_json, cap};
use super::view_input::{self, ViewArticle};
use super::{AssistantError, Task};

/// At most this many unread Articles are ranked per run.
pub const CAP: usize = 100;
/// Batch limits: at most this many Articles, and about this many characters (~4 chars per token),
/// per Assistant call.
pub const BATCH_SIZE: usize = 25;
pub const BATCH_CHAR_CAP: usize = 16_000;
/// Caps on the answer's text.
const REASON_CAP: usize = 200;
const INTERESTS_CAP: usize = 1_000;
/// Per-batch timeout.
const TIMEOUT: Duration = Duration::from_secs(120);

/// What one picks run is built from: one task per batch.
pub struct Plan {
    pub tasks: Vec<Task>,
    pub articles: Vec<ViewArticle>,
    pub considered: usize,
    pub capped: bool,
}

/// The JSON shape both CLIs answer with.
pub fn json_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "picks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "score": { "type": "number", "minimum": 0, "maximum": 1 },
                        "reason": { "type": "string" }
                    },
                    "required": ["id", "score", "reason"]
                }
            }
        },
        "required": ["picks"]
    })
}

/// Splits the Articles into batches: at most [`BATCH_SIZE`] Articles and [`BATCH_CHAR_CAP`]
/// characters each. A single Article bigger than the cap still gets its own batch (its excerpt
/// is already capped), so nothing is silently dropped.
pub fn batches(articles: &[ViewArticle]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut chars = 0usize;
    for (i, a) in articles.iter().enumerate() {
        let len = article_chars(a);
        if !current.is_empty() && (current.len() >= BATCH_SIZE || chars + len > BATCH_CHAR_CAP) {
            out.push(std::mem::take(&mut current));
            chars = 0;
        }
        current.push(i);
        chars += len;
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn article_chars(a: &ViewArticle) -> usize {
    a.title.chars().count() + a.excerpt.chars().count() + a.feed_title.chars().count() + 16
}

/// Builds the picks tasks for `query` (unread Articles only) against `interests`. Fails when
/// there is nothing unread to rank.
pub fn plan(library: &Library, query: &ListQuery, interests: &str) -> Result<Plan> {
    let (articles, capped) = view_input::newest(library, query, true, CAP)?;
    if articles.is_empty() {
        return Err(Error::Invalid("no unread Articles in this View".into()));
    }
    let considered = articles.len();
    let label = view_input::input_label(considered, capped, query.sort);
    let (interests, _) = cap(interests.trim(), INTERESTS_CAP);
    let batches = batches(&articles);
    let system_prompt = format!(
        "{SYSTEM_PREAMBLE}\n\n\
         You rank a reader's unread Articles against their interests. For every Article in the \
         data, give a score from 0 to 1 (how well it matches the interests and is worth reading) \
         and a one-line reason. Use only the ids given in the data; never invent an id. Answer \
         only with the required JSON."
    );
    let tasks = batches
        .iter()
        .map(|batch| Task {
            system_prompt: system_prompt.clone(),
            prompt: format!(
                "The user's interests:\n{interests}\n\n\
                 Rank the Articles in this batch of the View (based {label}).\n\n\
                 Answer only with the required JSON: \
                 {{\"picks\": [{{\"id\": 1, \"score\": 0.9, \"reason\": \"…\"}}]}}.\n\n\
                 Rules: every id from the data below, a score from 0 to 1, a one-line reason, \
                 nothing else in the answer.\n\n{}",
                view_input::render(batch.iter().map(|&i| &articles[i]))
            ),
            json_schema: Some(json_schema()),
            model: None,
            timeout: TIMEOUT,
        })
        .collect();
    Ok(Plan {
        tasks,
        articles,
        considered,
        capped,
    })
}

/// One pick as the Assistant answered it, before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct RawPick {
    pub id: i64,
    pub score: f64,
    pub reason: String,
}

/// A validated pick that crosses to the UI, enriched with the Library title.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pick {
    pub id: i64,
    pub title: String,
    pub feed_title: String,
    pub score: f64,
    pub reason: String,
}

/// The validated picks run: how many Articles were considered, and the picks sorted best first.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Picks {
    pub considered: usize,
    pub capped: bool,
    pub picks: Vec<Pick>,
}

/// Validates one batch's answer: ids must exist in the Library, scores are clamped to 0..1.
pub fn validate_batch(
    library: &Library,
    text: &str,
    structured: Option<&Value>,
) -> Result<Vec<RawPick>, AssistantError> {
    let answer = answer_json(text, structured)?;
    let Some(picks) = answer.get("picks").and_then(Value::as_array) else {
        return Err(AssistantError::Malformed {
            message: "the answer held no \"picks\" array".into(),
        });
    };
    let mut raw = Vec::new();
    for p in picks {
        let Some(id) = p.get("id").and_then(Value::as_i64) else {
            continue;
        };
        let score = p.get("score").and_then(Value::as_f64).unwrap_or(0.0);
        raw.push(RawPick {
            id,
            score: score.clamp(0.0, 1.0),
            reason: p
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    // Only ids that exist in the Library survive; the rest are dropped.
    let ids: Vec<i64> = raw.iter().map(|p| p.id).collect();
    let known: std::collections::HashSet<i64> = view_input::article_titles(library, &ids)
        .map_err(|e| AssistantError::Failed {
            message: e.to_string(),
        })?
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    Ok(raw.into_iter().filter(|p| known.contains(&p.id)).collect())
}

/// Merges the batches' validated picks: one pick per Article (the first batch that named it
/// wins, batches run newest first), sorted by score, enriched with titles.
pub fn merge(
    library: &Library,
    plan: &Plan,
    batches: Vec<Vec<RawPick>>,
) -> Result<Picks, AssistantError> {
    let all_ids: Vec<i64> = batches
        .iter()
        .flat_map(|b| b.iter().map(|p| p.id))
        .collect();
    let titles: std::collections::HashMap<i64, (String, String)> =
        view_input::article_titles(library, &all_ids)
            .map_err(|e| AssistantError::Failed {
                message: e.to_string(),
            })?
            .into_iter()
            .map(|(id, title, feed_title)| (id, (title, feed_title)))
            .collect();

    let mut picks: Vec<Pick> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for batch in batches {
        for raw in batch {
            if !seen.insert(raw.id) {
                continue;
            }
            let Some((title, feed_title)) = titles.get(&raw.id) else {
                continue;
            };
            let (reason, _) = cap(&raw.reason, REASON_CAP);
            picks.push(Pick {
                id: raw.id,
                title: title.clone(),
                feed_title: feed_title.clone(),
                score: raw.score,
                reason,
            });
        }
    }
    picks.sort_by(|a, b| b.score.total_cmp(&a.score).then(b.id.cmp(&a.id)));
    Ok(Picks {
        considered: plan.considered,
        capped: plan.capped,
        picks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::articles::{Sort, View};
    use crate::feeds::FetchResult;
    use crate::library::DIR_NAME;
    use crate::opml::OpmlFeed;
    use crate::parse::{ParsedEntry, ParsedFeed};

    fn entry(identity: &str, title: &str, published_at: i64) -> ParsedEntry {
        ParsedEntry {
            identity: identity.into(),
            url: Some(format!("https://x.example/{identity}")),
            title: title.into(),
            author: None,
            content_html: format!("<p>{title} body</p>"),
            content_text: format!("{title} body"),
            excerpt: format!("{title} excerpt"),
            published_at: Some(published_at),
            images: vec![],
            lead_image: None,
            enclosures: vec![],
            video: None,
        }
    }

    fn library_with(n: usize) -> (tempfile::TempDir, Library) {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        library
            .import_opml(&[OpmlFeed {
                url: "https://x.example/feed".into(),
                title: "X".into(),
                site_url: None,
                folder: Some("F".into()),
            }])
            .unwrap();
        let entries = (0..n)
            .map(|i| {
                entry(
                    &format!("e{i}"),
                    &format!("Article {i}"),
                    1_700_000_000 + i as i64,
                )
            })
            .collect();
        library
            .apply_fetch(
                1,
                FetchResult::Fetched {
                    feed: ParsedFeed {
                        title: None,
                        site_url: None,
                        entries,
                    },
                    etag: None,
                    last_modified: None,
                },
            )
            .unwrap();
        (root, library)
    }

    fn query(view: View, sort: Sort) -> ListQuery {
        ListQuery {
            view,
            sort,
            search: None,
            with_lead_image: false,
        }
    }

    fn article(id: i64, title: &str, excerpt_len: usize) -> ViewArticle {
        ViewArticle {
            id,
            feed_title: "X".into(),
            title: title.into(),
            excerpt: "e".repeat(excerpt_len),
        }
    }

    #[test]
    fn batches_respect_size_and_char_caps() {
        let articles: Vec<ViewArticle> = (0..60).map(|i| article(i, "t", 100)).collect();
        let bs = batches(&articles);
        assert_eq!(bs.len(), 3, "60 Articles at 25 per batch");
        assert!(bs.iter().all(|b| b.len() <= BATCH_SIZE));
        // A char cap forces smaller batches.
        let big: Vec<ViewArticle> = (0..10).map(|i| article(i, "t", 5_000)).collect();
        let bs = batches(&big);
        assert!(bs.len() > 1);
        assert!(bs.iter().all(|b| {
            b.iter().map(|&i| article_chars(&big[i])).sum::<usize>()
                <= BATCH_CHAR_CAP + article_chars(&big[*b.last().unwrap()])
        }));
        // One Article bigger than the whole cap still gets its own batch, never dropped.
        let huge = vec![article(0, "t", BATCH_CHAR_CAP + 500), article(1, "t", 10)];
        let bs = batches(&huge);
        assert_eq!(bs.len(), 2);
        assert_eq!(bs[0], vec![0]);
        assert_eq!(bs[1], vec![1]);
        // Empty input: no batches.
        assert!(batches(&[]).is_empty());
    }

    #[test]
    fn plan_builds_one_task_per_batch_over_unread_articles_only() {
        let (_root, library) = library_with(60);
        let all_unread = query(View::AllUnread, Sort::NewestFirst);
        library.set_read(1, true).unwrap(); // article id 1 is read
        let p = plan(
            &library,
            &all_unread,
            "Rust, local-first software; not crypto",
        )
        .unwrap();
        assert_eq!(p.considered, 59);
        assert!(!p.capped);
        assert_eq!(p.tasks.len(), 3, "59 unread at 25 per batch");
        for task in &p.tasks {
            assert!(task.system_prompt.starts_with(SYSTEM_PREAMBLE));
            assert!(
                task.prompt
                    .contains("The user's interests:\nRust, local-first software; not crypto")
            );
            assert!(
                task.prompt
                    .contains("<<<FEEDRICO_DATA Articles of the View>>>")
            );
            assert!(task.prompt.contains("based from all 59 Articles"));
            assert!(task.json_schema.is_some());
            // The read Article is not in any batch.
            assert!(!task.prompt.contains("[1] "));
        }
        // The first batch holds the newest Articles.
        assert!(p.tasks[0].prompt.contains("— Article 59\n"));
        library.close().unwrap();
    }

    #[test]
    fn plan_caps_at_100_and_says_how_many_were_considered() {
        let (_root, library) = library_with(120);
        let p = plan(&library, &query(View::AllUnread, Sort::NewestFirst), "Rust").unwrap();
        assert_eq!(p.considered, 100);
        assert!(p.capped);
        assert!(p.tasks[0].prompt.contains("from the newest 100 Articles"));
        assert_eq!(p.tasks.len(), 4);
        // Nothing unread: a plain error, no run.
        library
            .mark_all_read(&query(View::AllUnread, Sort::NewestFirst))
            .unwrap();
        assert!(plan(&library, &query(View::AllUnread, Sort::NewestFirst), "Rust").is_err());
        library.close().unwrap();
    }

    #[test]
    fn validate_batch_drops_unknown_ids_and_clamps_scores() {
        let (_root, library) = library_with(3);
        let structured = serde_json::json!({
            "picks": [
                { "id": 1, "score": 0.9, "reason": "right on topic" },
                { "id": 999_999, "score": 1.0, "reason": "unknown id" },
                { "id": 2, "score": 7.5, "reason": "too high" },
                { "id": 3, "reason": "no score" }
            ]
        });
        let raw = validate_batch(&library, "", Some(&structured)).unwrap();
        assert_eq!(raw.len(), 3);
        assert_eq!(
            raw[0],
            RawPick {
                id: 1,
                score: 0.9,
                reason: "right on topic".into()
            }
        );
        assert_eq!(raw[1].id, 2);
        assert_eq!(raw[1].score, 1.0, "clamped to 1");
        assert_eq!(raw[2].id, 3);
        assert_eq!(raw[2].score, 0.0, "missing score reads as 0");
        // Vibe-style: the JSON is in the text.
        let text = "Here: {\"picks\": [{\"id\": 1, \"score\": 0.5, \"reason\": \"ok\"}]}";
        let raw = validate_batch(&library, text, None).unwrap();
        assert_eq!(raw.len(), 1);
        let err = validate_batch(&library, "nope", None).unwrap_err();
        assert!(matches!(err, AssistantError::Malformed { .. }));
        library.close().unwrap();
    }

    #[test]
    fn merge_dedupes_sorts_and_enriches() {
        let (_root, library) = library_with(3);
        let p = plan(&library, &query(View::AllUnread, Sort::NewestFirst), "Rust").unwrap();
        let batches = vec![
            vec![
                RawPick {
                    id: 3,
                    score: 0.4,
                    reason: "ok".into(),
                },
                RawPick {
                    id: 1,
                    score: 0.9,
                    reason: "great".into(),
                },
            ],
            vec![
                RawPick {
                    id: 1,
                    score: 0.1,
                    reason: "again".into(),
                }, // duplicate: first wins
                RawPick {
                    id: 2,
                    score: 0.7,
                    reason: "long ".repeat(100),
                },
            ],
        ];
        let picks = merge(&library, &p, batches).unwrap();
        assert_eq!(picks.considered, 3);
        assert_eq!(picks.picks.len(), 3);
        assert_eq!(picks.picks[0].id, 1);
        assert_eq!(picks.picks[0].score, 0.9);
        assert_eq!(picks.picks[0].title, "Article 0");
        assert_eq!(picks.picks[0].feed_title, "X");
        assert_eq!(picks.picks[1].id, 2);
        assert_eq!(picks.picks[2].id, 3);
        assert_eq!(
            picks.picks[1].reason.chars().count(),
            REASON_CAP,
            "reason is capped"
        );
        library.close().unwrap();
    }
}
