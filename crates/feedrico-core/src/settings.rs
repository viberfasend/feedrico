//! Library settings: they travel with the Library (spec: "What lives in the Library").

use std::collections::HashMap;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{Error, Library, Result};

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

const PRUNE_READ_DAYS_KEY: &str = "pruning.readDays";
const PRUNE_UNREAD_DAYS_KEY: &str = "pruning.unreadDays";
/// Spec: "Read, unstarred Articles: removed when older than 90 days (configurable)."
pub const DEFAULT_PRUNE_READ_DAYS: u32 = 90;
/// Spec: "Unread Articles: removed when older than 1 year (configurable)."
pub const DEFAULT_PRUNE_UNREAD_DAYS: u32 = 365;

const THEME_KEY: &str = "appearance.theme";
const DENSITY_KEY: &str = "appearance.density";
const UNREAD_COUNTS_KEY: &str = "appearance.unreadCounts";
const READER_FONT_FAMILY_KEY: &str = "reader.fontFamily";
const READER_FONT_SIZE_KEY: &str = "reader.fontSize";
const READER_MEASURE_KEY: &str = "reader.measure";
const READER_LINE_HEIGHT_KEY: &str = "reader.lineHeight";
const REFRESH_INTERVAL_KEY: &str = "refresh.intervalMinutes";

/// The refresh intervals the UI offers (spec: "on an interval while the app is open").
pub const REFRESH_INTERVALS: [u32; 5] = [15, 30, 60, 120, 240];
pub const DEFAULT_REFRESH_INTERVAL_MINUTES: u32 = 30;

/// Reader typography defaults come from the current tokens: `--font-size-reading` is 1.125rem
/// (18 px), `--reader-measure` is 66ch, `--font-line-height-reading` is 1.65.
pub const DEFAULT_READER_FONT_SIZE: u32 = 18;
pub const DEFAULT_READER_MEASURE: u32 = 66;
pub const DEFAULT_READER_LINE_HEIGHT: f64 = 1.65;

pub const MIN_READER_FONT_SIZE: u32 = 14;
pub const MAX_READER_FONT_SIZE: u32 = 24;
pub const MIN_READER_MEASURE: u32 = 50;
pub const MAX_READER_MEASURE: u32 = 90;
pub const MIN_READER_LINE_HEIGHT: f64 = 1.3;
pub const MAX_READER_LINE_HEIGHT: f64 = 2.0;
/// Retention in days stays within ten years, so a Library can't prune everything by accident.
pub const MAX_PRUNE_DAYS: u32 = 3650;

/// Theme: `system` follows the OS (spec: themes "light, dark, follow OS").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

/// Unread numbers in the sidebar and list header. `Hide` shows a dot instead, for readers who
/// find counters nagging; Read state itself is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnreadCounts {
    #[default]
    Show,
    Hide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReaderFontFamily {
    #[default]
    Serif,
    Sans,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub theme: Theme,
    pub density: Density,
    #[serde(default)]
    pub unread_counts: UnreadCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderSettings {
    pub font_family: ReaderFontFamily,
    /// Font size in px (14–24).
    pub font_size: u32,
    /// Line width in ch (50–90).
    pub measure: u32,
    /// Line height (1.3–2.0).
    pub line_height: f64,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        Self {
            font_family: ReaderFontFamily::default(),
            font_size: DEFAULT_READER_FONT_SIZE,
            measure: DEFAULT_READER_MEASURE,
            line_height: DEFAULT_READER_LINE_HEIGHT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSettings {
    pub interval_minutes: u32,
}

impl Default for RefreshSettings {
    fn default() -> Self {
        Self {
            interval_minutes: DEFAULT_REFRESH_INTERVAL_MINUTES,
        }
    }
}

/// Every setting that travels with the Library (spec: "What lives in the Library"). Stored as
/// individual JSON values in the `setting` table under stable flat keys; a missing or garbage
/// value reads back as the default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub appearance: AppearanceSettings,
    pub reader: ReaderSettings,
    pub refresh: RefreshSettings,
    pub image_cache_cap_bytes: u64,
    pub prune_read_days: u32,
    pub prune_unread_days: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance: AppearanceSettings::default(),
            reader: ReaderSettings::default(),
            refresh: RefreshSettings::default(),
            image_cache_cap_bytes: DEFAULT_IMAGE_CACHE_CAP,
            prune_read_days: DEFAULT_PRUNE_READ_DAYS,
            prune_unread_days: DEFAULT_PRUNE_UNREAD_DAYS,
        }
    }
}

impl Settings {
    /// Rejects out-of-range values before anything is written.
    pub fn validate(&self) -> Result<()> {
        let invalid = |key: &str, message: String| Error::InvalidSetting {
            key: key.to_owned(),
            message,
        };
        if !(MIN_READER_FONT_SIZE..=MAX_READER_FONT_SIZE).contains(&self.reader.font_size) {
            return Err(invalid(
                READER_FONT_SIZE_KEY,
                format!(
                    "fontSize must be {MIN_READER_FONT_SIZE}–{MAX_READER_FONT_SIZE} px, got {}",
                    self.reader.font_size
                ),
            ));
        }
        if !(MIN_READER_MEASURE..=MAX_READER_MEASURE).contains(&self.reader.measure) {
            return Err(invalid(
                READER_MEASURE_KEY,
                format!(
                    "measure must be {MIN_READER_MEASURE}–{MAX_READER_MEASURE} ch, got {}",
                    self.reader.measure
                ),
            ));
        }
        if !(MIN_READER_LINE_HEIGHT..=MAX_READER_LINE_HEIGHT).contains(&self.reader.line_height) {
            return Err(invalid(
                READER_LINE_HEIGHT_KEY,
                format!(
                    "lineHeight must be {MIN_READER_LINE_HEIGHT}–{MAX_READER_LINE_HEIGHT}, got {}",
                    self.reader.line_height
                ),
            ));
        }
        if !REFRESH_INTERVALS.contains(&self.refresh.interval_minutes) {
            return Err(invalid(
                REFRESH_INTERVAL_KEY,
                format!(
                    "intervalMinutes must be one of {}, got {}",
                    REFRESH_INTERVALS
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    self.refresh.interval_minutes
                ),
            ));
        }
        if self.image_cache_cap_bytes == 0 {
            return Err(invalid(
                IMAGE_CACHE_CAP_KEY,
                "capBytes must be greater than 0".to_owned(),
            ));
        }
        for (key, days) in [
            (PRUNE_READ_DAYS_KEY, self.prune_read_days),
            (PRUNE_UNREAD_DAYS_KEY, self.prune_unread_days),
        ] {
            if days == 0 || days > MAX_PRUNE_DAYS {
                return Err(invalid(
                    key,
                    format!("days must be 1–{MAX_PRUNE_DAYS}, got {days}"),
                ));
            }
        }
        Ok(())
    }
}

const LAST_VIEW_KEY: &str = "view.last";
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

    /// Age in days (by Article Date) past which Read, unstarred Articles are pruned.
    pub fn prune_read_days(&self) -> Result<u32> {
        Ok(self
            .setting(PRUNE_READ_DAYS_KEY)?
            .unwrap_or(DEFAULT_PRUNE_READ_DAYS))
    }

    pub fn set_prune_read_days(&self, days: u32) -> Result<()> {
        self.set_setting(PRUNE_READ_DAYS_KEY, &days)
    }

    /// Age in days (by Article Date) past which unread, unstarred Articles are pruned.
    pub fn prune_unread_days(&self) -> Result<u32> {
        Ok(self
            .setting(PRUNE_UNREAD_DAYS_KEY)?
            .unwrap_or(DEFAULT_PRUNE_UNREAD_DAYS))
    }

    pub fn set_prune_unread_days(&self, days: u32) -> Result<()> {
        self.set_setting(PRUNE_UNREAD_DAYS_KEY, &days)
    }

    /// The View that was selected last, as a View key (`allUnread`, `feed:3`, …). Travels with
    /// the Library, so reading continues where it stopped, on any machine.
    pub fn last_view(&self) -> Result<Option<String>> {
        self.setting(LAST_VIEW_KEY)
    }

    pub fn set_last_view(&self, view: &str) -> Result<()> {
        self.set_setting(LAST_VIEW_KEY, &view)
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

    /// Whether `key` is present in the `setting` table, regardless of its value.
    pub fn has_setting(&self, key: &str) -> Result<bool> {
        let found: Option<i64> = self
            .conn()
            .query_row("SELECT 1 FROM setting WHERE key = ?1", [key], |r| r.get(0))
            .optional()?;
        Ok(found.is_some())
    }

    /// Every Library setting. A missing key, an unparseable value or an out-of-range value
    /// reads back as that field's default.
    pub fn settings(&self) -> Result<Settings> {
        let mut s = Settings::default();
        if let Some(v) = self.setting(THEME_KEY)? {
            s.appearance.theme = v;
        }
        if let Some(v) = self.setting(DENSITY_KEY)? {
            s.appearance.density = v;
        }
        if let Some(v) = self.setting(UNREAD_COUNTS_KEY)? {
            s.appearance.unread_counts = v;
        }
        if let Some(v) = self.setting(READER_FONT_FAMILY_KEY)? {
            s.reader.font_family = v;
        }
        if let Some(v) = self.setting::<u32>(READER_FONT_SIZE_KEY)?
            && (MIN_READER_FONT_SIZE..=MAX_READER_FONT_SIZE).contains(&v)
        {
            s.reader.font_size = v;
        }
        if let Some(v) = self.setting::<u32>(READER_MEASURE_KEY)?
            && (MIN_READER_MEASURE..=MAX_READER_MEASURE).contains(&v)
        {
            s.reader.measure = v;
        }
        if let Some(v) = self.setting::<f64>(READER_LINE_HEIGHT_KEY)?
            && (MIN_READER_LINE_HEIGHT..=MAX_READER_LINE_HEIGHT).contains(&v)
        {
            s.reader.line_height = v;
        }
        if let Some(v) = self.setting::<u32>(REFRESH_INTERVAL_KEY)?
            && REFRESH_INTERVALS.contains(&v)
        {
            s.refresh.interval_minutes = v;
        }
        if let Some(v) = self.setting(IMAGE_CACHE_CAP_KEY)?
            && v > 0
        {
            s.image_cache_cap_bytes = v;
        }
        if let Some(v) = self.setting::<u32>(PRUNE_READ_DAYS_KEY)?
            && (1..=MAX_PRUNE_DAYS).contains(&v)
        {
            s.prune_read_days = v;
        }
        if let Some(v) = self.setting::<u32>(PRUNE_UNREAD_DAYS_KEY)?
            && (1..=MAX_PRUNE_DAYS).contains(&v)
        {
            s.prune_unread_days = v;
        }
        Ok(s)
    }

    /// Validates and stores every Library setting in one transaction, so a rejected value never
    /// leaves the Library half-updated.
    pub fn set_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        let tx = self.conn().unchecked_transaction()?;
        fn write(
            tx: &rusqlite::Transaction,
            key: &str,
            value: impl serde::Serialize,
        ) -> Result<()> {
            tx.execute(
                "INSERT INTO setting (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, serde_json::to_string(&value)?],
            )?;
            Ok(())
        }
        write(&tx, THEME_KEY, settings.appearance.theme)?;
        write(&tx, DENSITY_KEY, settings.appearance.density)?;
        write(&tx, UNREAD_COUNTS_KEY, settings.appearance.unread_counts)?;
        write(&tx, READER_FONT_FAMILY_KEY, settings.reader.font_family)?;
        write(&tx, READER_FONT_SIZE_KEY, settings.reader.font_size)?;
        write(&tx, READER_MEASURE_KEY, settings.reader.measure)?;
        write(&tx, READER_LINE_HEIGHT_KEY, settings.reader.line_height)?;
        write(&tx, REFRESH_INTERVAL_KEY, settings.refresh.interval_minutes)?;
        write(&tx, IMAGE_CACHE_CAP_KEY, settings.image_cache_cap_bytes)?;
        write(&tx, PRUNE_READ_DAYS_KEY, settings.prune_read_days)?;
        write(&tx, PRUNE_UNREAD_DAYS_KEY, settings.prune_unread_days)?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::DIR_NAME;

    /// Keeps the tempdir alive for as long as the Library is open.
    fn fresh() -> (tempfile::TempDir, Library) {
        let root = tempfile::tempdir().unwrap();
        let library = Library::create(&root.path().join(DIR_NAME)).unwrap();
        (root, library)
    }

    #[test]
    fn settings_default_on_a_fresh_library() {
        let (_root, library) = fresh();
        let settings = library.settings().unwrap();
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.appearance.theme, Theme::System);
        assert_eq!(settings.appearance.density, Density::Comfortable);
        assert_eq!(settings.appearance.unread_counts, UnreadCounts::Show);
        assert_eq!(settings.reader.font_family, ReaderFontFamily::Serif);
        assert_eq!(settings.reader.font_size, 18);
        assert_eq!(settings.reader.measure, 66);
        assert_eq!(settings.reader.line_height, 1.65);
        assert_eq!(settings.refresh.interval_minutes, 30);
        assert_eq!(settings.image_cache_cap_bytes, DEFAULT_IMAGE_CACHE_CAP);
        assert_eq!(settings.prune_read_days, DEFAULT_PRUNE_READ_DAYS);
        assert_eq!(settings.prune_unread_days, DEFAULT_PRUNE_UNREAD_DAYS);
        library.close().unwrap();
    }

    #[test]
    fn last_view_is_remembered() {
        let (_root, library) = fresh();
        assert_eq!(library.last_view().unwrap(), None);
        library.set_last_view("folder:3").unwrap();
        assert_eq!(library.last_view().unwrap().as_deref(), Some("folder:3"));
    }

    #[test]
    fn settings_round_trip_every_field() {
        let (_root, library) = fresh();
        let settings = Settings {
            appearance: AppearanceSettings {
                theme: Theme::Dark,
                density: Density::Compact,
                unread_counts: UnreadCounts::Hide,
            },
            reader: ReaderSettings {
                font_family: ReaderFontFamily::Sans,
                font_size: 20,
                measure: 72,
                line_height: 1.8,
            },
            refresh: RefreshSettings {
                interval_minutes: 60,
            },
            image_cache_cap_bytes: 500 * 1024 * 1024,
            prune_read_days: 180,
            prune_unread_days: 730,
        };
        library.set_settings(&settings).unwrap();
        assert_eq!(library.settings().unwrap(), settings);
        // The pre-existing single-key accessors read the same keys.
        assert_eq!(library.image_cache_cap().unwrap(), 500 * 1024 * 1024);
        assert_eq!(library.prune_read_days().unwrap(), 180);
        assert_eq!(library.prune_unread_days().unwrap(), 730);
        library.close().unwrap();
    }

    #[test]
    fn out_of_range_values_are_rejected_with_a_typed_error() {
        let (_root, library) = fresh();
        let cases: Vec<(Settings, &str)> = vec![
            (
                Settings {
                    reader: ReaderSettings {
                        font_size: 13,
                        ..ReaderSettings::default()
                    },
                    ..Settings::default()
                },
                "reader.fontSize",
            ),
            (
                Settings {
                    reader: ReaderSettings {
                        font_size: 25,
                        ..ReaderSettings::default()
                    },
                    ..Settings::default()
                },
                "reader.fontSize",
            ),
            (
                Settings {
                    reader: ReaderSettings {
                        measure: 49,
                        ..ReaderSettings::default()
                    },
                    ..Settings::default()
                },
                "reader.measure",
            ),
            (
                Settings {
                    reader: ReaderSettings {
                        measure: 91,
                        ..ReaderSettings::default()
                    },
                    ..Settings::default()
                },
                "reader.measure",
            ),
            (
                Settings {
                    reader: ReaderSettings {
                        line_height: 1.29,
                        ..ReaderSettings::default()
                    },
                    ..Settings::default()
                },
                "reader.lineHeight",
            ),
            (
                Settings {
                    reader: ReaderSettings {
                        line_height: 2.01,
                        ..ReaderSettings::default()
                    },
                    ..Settings::default()
                },
                "reader.lineHeight",
            ),
            (
                Settings {
                    refresh: RefreshSettings {
                        interval_minutes: 45,
                    },
                    ..Settings::default()
                },
                "refresh.intervalMinutes",
            ),
            (
                Settings {
                    image_cache_cap_bytes: 0,
                    ..Settings::default()
                },
                "imageCache.capBytes",
            ),
            (
                Settings {
                    prune_read_days: 0,
                    ..Settings::default()
                },
                "pruning.readDays",
            ),
            (
                Settings {
                    prune_unread_days: 4000,
                    ..Settings::default()
                },
                "pruning.unreadDays",
            ),
        ];
        for (settings, key) in cases {
            match library.set_settings(&settings) {
                Err(crate::Error::InvalidSetting { key: k, .. }) => assert_eq!(k, key),
                other => panic!("expected InvalidSetting for {key}, got {other:?}"),
            }
            // A rejected write leaves the stored value untouched.
            assert_eq!(library.settings().unwrap(), Settings::default());
        }
        library.close().unwrap();
    }

    #[test]
    fn garbage_stored_values_read_as_the_default() {
        let (_root, library) = fresh();
        for (key, value) in [
            ("appearance.theme", "\"blue\""),
            ("appearance.density", "\"cozy\""),
            ("reader.fontFamily", "\"comic\""),
            ("reader.fontSize", "\"eighteen\""),
            ("reader.lineHeight", "\"loose\""),
            ("refresh.intervalMinutes", "45"),
            ("imageCache.capBytes", "\"a gig\""),
            ("pruning.readDays", "-3"),
        ] {
            library.conn().execute(
                "INSERT INTO setting (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            ).unwrap();
        }
        assert_eq!(library.settings().unwrap(), Settings::default());
        library.close().unwrap();
    }

    #[test]
    fn has_setting_reports_whether_a_key_is_stored() {
        let (_root, library) = fresh();
        assert!(!library.has_setting("appearance.theme").unwrap());
        let settings = Settings {
            appearance: AppearanceSettings {
                theme: Theme::Light,
                ..AppearanceSettings::default()
            },
            ..Settings::default()
        };
        library.set_settings(&settings).unwrap();
        assert!(library.has_setting("appearance.theme").unwrap());
        library.close().unwrap();
    }

    #[test]
    fn settings_survive_export_library() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(DIR_NAME);
        let library = Library::create(&dir).unwrap();
        let settings = Settings {
            appearance: AppearanceSettings {
                theme: Theme::Dark,
                density: Density::Compact,
                unread_counts: UnreadCounts::Hide,
            },
            reader: ReaderSettings {
                font_size: 22,
                ..ReaderSettings::default()
            },
            refresh: RefreshSettings {
                interval_minutes: 240,
            },
            prune_unread_days: 730,
            ..Settings::default()
        };
        library.set_settings(&settings).unwrap();
        let target = root.path().join("backup");
        library.export_to(&target, |_| {}).unwrap();
        library.close().unwrap();

        let copy = Library::open(&target, crate::OpenOptions::default()).unwrap();
        assert_eq!(copy.settings().unwrap(), settings);
        copy.close().unwrap();
    }

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
