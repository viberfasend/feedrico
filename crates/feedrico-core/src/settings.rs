//! Library settings: they travel with the Library (spec: "What lives in the Library").

use std::collections::HashMap;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{Library, Result};

/// See "Layout" in GLOSSARY.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    #[default]
    List,
    Cards,
    Magazine,
    Gallery,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layouts {
    /// For Views without their own choice, and for search results.
    pub default: Layout,
    /// Keyed by View: `allUnread`, `today`, `starred`, `feed:<id>`, `folder:<id>`.
    pub views: HashMap<String, Layout>,
}

const IMAGE_CACHE_CAP_KEY: &str = "imageCache.capBytes";
/// Spec: "A total cap is configurable (default 1 GB)."
pub const DEFAULT_IMAGE_CACHE_CAP: u64 = 1024 * 1024 * 1024;

const DEFAULT_KEY: &str = "layout.default";
const VIEW_PREFIX: &str = "layout.view.";

impl Library {
    fn setting<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = self
            .conn()
            .query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(raw.and_then(|v| serde_json::from_str(&v).ok()))
    }

    fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.conn().execute(
            "INSERT INTO setting (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    /// Total size cap of cached images, in bytes. See `evict_media`.
    pub fn image_cache_cap(&self) -> Result<u64> {
        Ok(self
            .setting(IMAGE_CACHE_CAP_KEY)?
            .unwrap_or(DEFAULT_IMAGE_CACHE_CAP))
    }

    pub fn set_image_cache_cap(&self, bytes: u64) -> Result<()> {
        self.set_setting(IMAGE_CACHE_CAP_KEY, &bytes)
    }

    pub fn layouts(&self) -> Result<Layouts> {
        let default = self.setting(DEFAULT_KEY)?.unwrap_or_default();
        let mut stmt = self
            .conn()
            .prepare("SELECT key, value FROM setting WHERE key LIKE 'layout.view.%'")?;
        let views = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|row| {
                let (key, value) = row.ok()?;
                let layout = serde_json::from_str(&value).ok()?;
                Some((key.strip_prefix(VIEW_PREFIX)?.to_owned(), layout))
            })
            .collect();
        Ok(Layouts { default, views })
    }

    /// `view` is a View key, or `None` for the global default.
    pub fn set_layout(&self, view: Option<&str>, layout: Layout) -> Result<()> {
        match view {
            None => self.set_setting(DEFAULT_KEY, &layout),
            Some(view) => self.set_setting(&format!("{VIEW_PREFIX}{view}"), &layout),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::DIR_NAME;

    #[test]
    fn image_cache_cap_defaults_to_1_gb_and_is_configurable() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        assert_eq!(library.image_cache_cap().unwrap(), 1024 * 1024 * 1024);
        library.set_image_cache_cap(5_000).unwrap();
        assert_eq!(library.image_cache_cap().unwrap(), 5_000);
        library.close().unwrap();
    }

    #[test]
    fn layouts_per_view_with_default() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        assert_eq!(library.layouts().unwrap(), Layouts::default());
        library.set_layout(None, Layout::Cards).unwrap();
        library.set_layout(Some("feed:3"), Layout::Gallery).unwrap();
        library
            .set_layout(Some("feed:3"), Layout::Magazine)
            .unwrap();
        let layouts = library.layouts().unwrap();
        assert_eq!(layouts.default, Layout::Cards);
        assert_eq!(layouts.views.get("feed:3"), Some(&Layout::Magazine));
        assert_eq!(layouts.views.len(), 1);
        library.close().unwrap();
    }
}
