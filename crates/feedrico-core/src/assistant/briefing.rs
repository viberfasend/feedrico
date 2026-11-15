//! Briefing of a View (#120): the Assistant groups the View's newest Articles into topics, each
//! a sentence with the ids of the Articles it comes from.
//!
//! Input is title, Feed name and excerpt only (see [`super::view_input`]), capped and labelled.
//! Claude Code gets a `--json-schema`; Vibe is asked for the same fixed JSON shape and the answer
//! is validated in Rust. Ids that are not in the Library are dropped. The run is read-only: it
//! never marks anything Read.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::articles::ListQuery;
use crate::{Error, Library, Result};

use super::prompts::{SYSTEM_PREAMBLE, answer_json, cap};
use super::view_input::{self, ViewArticle};
use super::{AssistantError, Task};

/// At most this many Articles go into one briefing, labelled in the prompt.
pub const CAP: usize = 50;
/// How many topics the answer may group them into.
pub const MAX_TOPICS: usize = 8;
/// Per-run timeout: a briefing of 50 titles and excerpts is one shot.
const TIMEOUT: Duration = Duration::from_secs(180);
/// Caps on the answer's text, so a runaway answer stays readable.
const TOPIC_CAP: usize = 80;
const SENTENCE_CAP: usize = 400;

/// What one run is built from.
pub struct Plan {
    pub task: Task,
    pub articles: Vec<ViewArticle>,
    pub considered: usize,
    pub capped: bool,
}

/// The JSON shape both CLIs answer with.
pub fn json_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "topics": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "topic": { "type": "string" },
                        "sentence": { "type": "string" },
                        "articleIds": { "type": "array", "items": { "type": "integer" } }
                    },
                    "required": ["topic", "sentence", "articleIds"]
                }
            }
        },
        "required": ["topics"]
    })
}

/// Builds the briefing task for `query`: the View's Articles in its sort order, capped at
/// [`CAP`] and labelled. Fails when the View has no Articles at all.
pub fn plan(library: &Library, query: &ListQuery) -> Result<Plan> {
    let (articles, capped) = view_input::newest(library, query, false, CAP)?;
    if articles.is_empty() {
        return Err(Error::Invalid("no Articles in this View".into()));
    }
    let considered = articles.len();
    let label = view_input::input_label(considered, capped, query.sort);
    let system_prompt = format!(
        "{SYSTEM_PREAMBLE}\n\n\
         You write a short briefing of a reader's Articles. Group the Articles into at most \
         {MAX_TOPICS} topics. Each topic has a short topic label, one plain sentence that says \
         what is going on, and the ids of the Articles it comes from. Use only the ids given in \
         the data; never invent an id. Answer only with the required JSON."
    );
    let prompt = format!(
        "Write a briefing of this View, based {label}.\n\n\
         Answer only with the required JSON: \
         {{\"topics\": [{{\"topic\": \"…\", \"sentence\": \"…\", \"articleIds\": [1, 2]}}]}}.\n\n\
         Rules: at most {MAX_TOPICS} topics, one sentence per topic, every articleId from the \
         data below, nothing else in the answer.\n\n{}",
        view_input::render(articles.iter())
    );
    Ok(Plan {
        task: Task {
            system_prompt,
            prompt,
            json_schema: Some(json_schema()),
            model: None,
            timeout: TIMEOUT,
        },
        articles,
        considered,
        capped,
    })
}

/// One topic as the Assistant answered it, before validation.
#[derive(Debug, Clone, PartialEq)]
struct RawTopic {
    topic: String,
    sentence: String,
    article_ids: Vec<i64>,
}

/// An Article a result links to, enriched with its Library title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefingArticle {
    pub id: i64,
    pub title: String,
    pub feed_title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefingTopic {
    pub topic: String,
    pub sentence: String,
    pub article_ids: Vec<i64>,
}

/// The validated briefing that crosses to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Briefing {
    pub considered: usize,
    pub capped: bool,
    pub topics: Vec<BriefingTopic>,
    pub articles: Vec<BriefingArticle>,
}

/// Validates one run's answer against the Library: unknown ids are dropped, topics left without
/// any id are dropped, the rest is capped and enriched with titles.
pub fn validate(
    library: &Library,
    plan: &Plan,
    text: &str,
    structured: Option<&Value>,
) -> Result<Briefing, AssistantError> {
    let answer = answer_json(text, structured)?;
    let raw = parse_topics(&answer)?;
    let all_ids: Vec<i64> = raw
        .iter()
        .flat_map(|t| t.article_ids.iter().copied())
        .collect();
    let titles =
        view_input::article_titles(library, &all_ids).map_err(|e| AssistantError::Failed {
            message: e.to_string(),
        })?;
    let mut known: std::collections::HashMap<i64, (String, String)> = titles
        .into_iter()
        .map(|(id, title, feed_title)| (id, (title, feed_title)))
        .collect();

    let mut topics = Vec::new();
    let mut articles: Vec<BriefingArticle> = Vec::new();
    let mut seen_articles: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for raw_topic in raw {
        let mut ids = Vec::new();
        for id in raw_topic.article_ids {
            if known.contains_key(&id) && !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.is_empty() {
            continue; // Every id was unknown: the topic says nothing we can link to.
        }
        let (topic, _) = cap(&raw_topic.topic, TOPIC_CAP);
        let (sentence, _) = cap(&raw_topic.sentence, SENTENCE_CAP);
        for id in &ids {
            if seen_articles.insert(*id) {
                let (title, feed_title) = known.remove(id).expect("checked above");
                articles.push(BriefingArticle {
                    id: *id,
                    title,
                    feed_title,
                });
            }
        }
        topics.push(BriefingTopic {
            topic,
            sentence,
            article_ids: ids,
        });
    }
    Ok(Briefing {
        considered: plan.considered,
        capped: plan.capped,
        topics,
        articles,
    })
}

fn parse_topics(answer: &Value) -> Result<Vec<RawTopic>, AssistantError> {
    let Some(topics) = answer.get("topics").and_then(Value::as_array) else {
        return Err(AssistantError::Malformed {
            message: "the answer held no \"topics\" array".into(),
        });
    };
    let mut out = Vec::new();
    for t in topics {
        let ids = t
            .get("articleIds")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_i64).collect::<Vec<i64>>())
            .unwrap_or_default();
        out.push(RawTopic {
            topic: t
                .get("topic")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            sentence: t
                .get("sentence")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            article_ids: ids,
        });
    }
    Ok(out)
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

    fn feed_query(sort: Sort) -> ListQuery {
        ListQuery {
            view: View::Feed { id: 1 },
            sort,
            search: None,
            with_lead_image: false,
        }
    }

    #[test]
    fn plan_wraps_the_articles_and_labels_the_input() {
        let (_root, library) = library_with(3);
        let p = plan(&library, &feed_query(Sort::NewestFirst)).unwrap();
        assert_eq!(p.considered, 3);
        assert!(!p.capped);
        assert!(p.task.system_prompt.starts_with(SYSTEM_PREAMBLE));
        assert!(p.task.prompt.contains("based from all 3 Articles"));
        assert!(
            p.task
                .prompt
                .contains("<<<FEEDRICO_DATA Articles of the View>>>")
        );
        assert!(
            p.task
                .prompt
                .contains("[1] X — Article 0\nArticle 0 excerpt")
        );
        // Newest first: Article 2 comes first in the block.
        let first = p.task.prompt.find("Article 2").unwrap();
        let last = p.task.prompt.find("Article 0").unwrap();
        assert!(first < last);
        assert_eq!(p.task.json_schema, Some(json_schema()));
        assert!(
            p.task
                .json_schema
                .as_ref()
                .unwrap()
                .to_string()
                .contains("articleIds")
        );
        library.close().unwrap();
    }

    #[test]
    fn plan_caps_at_50_and_says_so() {
        let (_root, library) = library_with(60);
        let p = plan(&library, &feed_query(Sort::NewestFirst)).unwrap();
        assert_eq!(p.considered, 50);
        assert!(p.capped);
        assert!(p.task.prompt.contains("from the newest 50 Articles"));
        assert!(
            p.task.prompt.contains("— Article 10\n"),
            "the 50th newest is in"
        );
        assert!(
            !p.task.prompt.contains("— Article 9\n"),
            "only the newest 50 are in"
        );
        library.close().unwrap();
    }

    #[test]
    fn plan_respects_the_views_sort_and_fails_on_an_empty_view() {
        let (_root, library) = library_with(3);
        let p = plan(&library, &feed_query(Sort::OldestFirst)).unwrap();
        let first = p.task.prompt.find("Article 0").unwrap();
        let last = p.task.prompt.find("Article 2").unwrap();
        assert!(first < last, "oldest first");
        let empty = ListQuery {
            view: View::Feed { id: 99 },
            ..feed_query(Sort::NewestFirst)
        };
        assert!(plan(&library, &empty).is_err());
        library.close().unwrap();
    }

    #[test]
    fn validate_drops_unknown_ids_and_empty_topics_and_enriches_titles() {
        let (_root, library) = library_with(3);
        let p = plan(&library, &feed_query(Sort::NewestFirst)).unwrap();
        let ids: Vec<i64> = p.articles.iter().map(|a| a.id).collect();
        let structured = serde_json::json!({
            "topics": [
                { "topic": "Rust", "sentence": "Three posts about Rust.", "articleIds": [ids[0], 999_999, ids[1]] },
                { "topic": "Ghost", "sentence": "Only an unknown id.", "articleIds": [424_242] },
                { "topic": "Dup", "sentence": "Same id twice.", "articleIds": [ids[2], ids[2]] }
            ]
        });
        let briefing = validate(&library, &p, "", Some(&structured)).unwrap();
        assert_eq!(briefing.considered, 3);
        assert_eq!(briefing.topics.len(), 2, "the all-unknown topic is dropped");
        assert_eq!(briefing.topics[0].article_ids, vec![ids[0], ids[1]]);
        assert_eq!(
            briefing.topics[1].article_ids,
            vec![ids[2]],
            "duplicates collapse"
        );
        assert_eq!(briefing.articles.len(), 3);
        assert_eq!(briefing.articles[0].title, "Article 2");
        assert_eq!(briefing.articles[0].feed_title, "X");
        library.close().unwrap();
    }

    #[test]
    fn validate_reads_vibes_json_from_the_text_and_caps_long_answers() {
        let (_root, library) = library_with(2);
        let p = plan(&library, &feed_query(Sort::NewestFirst)).unwrap();
        let id = p.articles[0].id;
        let long_sentence = "s".repeat(SENTENCE_CAP + 100);
        let text = format!(
            "Sure!\n{{\"topics\": [{{\"topic\": \"T\", \"sentence\": \"{long_sentence}\", \"articleIds\": [{id}]}}]}}\nHope that helps."
        );
        let briefing = validate(&library, &p, &text, None).unwrap();
        assert_eq!(briefing.topics.len(), 1);
        assert_eq!(briefing.topics[0].sentence.chars().count(), SENTENCE_CAP);
        assert_eq!(briefing.topics[0].article_ids, vec![id]);
        // Garbage in the text is a malformed answer, not a crash.
        let err = validate(&library, &p, "no json here", None).unwrap_err();
        assert!(matches!(err, AssistantError::Malformed { .. }));
        library.close().unwrap();
    }
}
