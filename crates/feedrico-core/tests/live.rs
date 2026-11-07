//! Live network check, not part of the normal test run: `cargo test -p feedrico-core -- --ignored`.

use feedrico_core::library::DIR_NAME;
use feedrico_core::opml::OpmlFeed;
use feedrico_core::{Fetcher, Library, ListQuery, Sort, View};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network"]
async fn fetches_real_feeds_end_to_end() {
    let root = tempfile::tempdir().unwrap();
    let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
    library
        .import_opml(&[
            OpmlFeed {
                url: "https://blog.rust-lang.org/feed.xml".into(),
                title: "Rust Blog".into(),
                site_url: None,
                folder: Some("Tech".into()),
            },
            OpmlFeed {
                url: "https://example.invalid/feed".into(),
                title: "Broken".into(),
                site_url: None,
                folder: None,
            },
        ])
        .unwrap();
    let fetcher = Fetcher::new("media://localhost/").unwrap();
    let mut images = Vec::new();
    for feed in library.feeds_to_fetch(false).unwrap() {
        let result = fetcher.fetch(&feed).await;
        let outcome = library.apply_fetch(feed.id, result).unwrap();
        images.extend(outcome.images);
    }
    let query = ListQuery {
        view: View::AllUnread,
        sort: Sort::NewestFirst,
        search: None,
        with_lead_image: false,
    };
    let total = library.count_articles(&query).unwrap();
    let rows = library.list_articles(&query, 0, 3).unwrap();
    println!(
        "{total} Articles; newest: {:#?}",
        rows.iter().map(|r| &r.title).collect::<Vec<_>>()
    );
    assert!(total > 0);
    let detail = library.article(rows[0].id).unwrap();
    assert!(!detail.content_html.contains("<script"));

    let sidebar = library.sidebar(0).unwrap();
    assert!(
        sidebar.feeds[0].last_error.is_some(),
        "broken Feed is marked"
    );
    println!("broken: {:?}", sidebar.feeds[0].last_error);

    // Second fetch should be conditional (304) or at least add nothing.
    let feed = library.feeds_to_fetch(true).unwrap().remove(0);
    let again = library
        .apply_fetch(feed.id, fetcher.fetch(&feed).await)
        .unwrap();
    assert_eq!(again.new_articles, 0);
    library.close().unwrap();
}
