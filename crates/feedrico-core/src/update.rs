//! Manual update checks against GitHub Releases. Spec "Network policy": the only non-feed
//! request the app makes, and only when the user clicks "Check for updates".

use reqwest::StatusCode;

use crate::{Error, Result};

/// The endpoint for the newest GitHub Release. Only ever requested on a user click.
pub(crate) const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/viberfasend/feedrico/releases/latest";
/// Where a release link goes when the API's `html_url` is missing or points elsewhere.
pub(crate) const RELEASES_PAGE: &str = "https://github.com/viberfasend/feedrico/releases";
/// The only prefix a release response may send the user to; anything else falls back to
/// `RELEASES_PAGE`, so a hostile response can't redirect the browser elsewhere.
const RELEASE_URL_PREFIX: &str = "https://github.com/viberfasend/feedrico/";
/// Release JSON is a few kilobytes; anything bigger is not worth reading.
pub(crate) const MAX_UPDATE_BYTES: usize = 256 * 1024;
/// Update checks must feel instant and never hang on a slow network.
pub(crate) const UPDATE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// What one manual check against GitHub Releases found.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateStatus {
    UpToDate {
        current: String,
    },
    Available {
        current: String,
        latest: String,
        url: String,
    },
    /// The repo has no Releases yet; a 404 from the API.
    NoReleases {
        current: String,
    },
}

/// Compares the running version with a release tag. Pure, so it is testable without network.
pub fn compare(current: &str, latest_tag: &str) -> UpdateStatus {
    decide(current, latest_tag, RELEASES_PAGE)
}

fn decide(current: &str, latest_tag: &str, url: &str) -> UpdateStatus {
    let current = current.trim().trim_start_matches('v');
    let tag = latest_tag.trim().trim_start_matches('v');
    let (Some(cur), Some(latest)) = (Version::parse(current), Version::parse(tag)) else {
        // Anything unparseable reads as "up to date": never offer an update we can't reason
        // about, and never crash on a weird tag.
        return UpdateStatus::UpToDate {
            current: current.into(),
        };
    };
    if latest > cur {
        UpdateStatus::Available {
            current: current.into(),
            latest: tag.into(),
            url: url.into(),
        }
    } else {
        UpdateStatus::UpToDate {
            current: current.into(),
        }
    }
}

/// Maps one `releases/latest` response to an `UpdateStatus`. Pure, so it is testable without
/// network. 404 means no Release has been published yet, not an error.
pub(crate) fn map_response(current: &str, status: u16, body: &[u8]) -> Result<UpdateStatus> {
    if status == StatusCode::NOT_FOUND.as_u16() {
        return Ok(UpdateStatus::NoReleases {
            current: current.into(),
        });
    }
    if !(200..300).contains(&status) {
        return Err(Error::Network(format!("HTTP {status}")));
    }
    let json: serde_json::Value = serde_json::from_slice(body)
        .map_err(|e| Error::Network(format!("unreadable response: {e}")))?;
    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Network("response has no tag_name".into()))?;
    let url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .filter(|u| u.starts_with(RELEASE_URL_PREFIX))
        .unwrap_or(RELEASES_PAGE);
    Ok(decide(current, tag, url))
}

/// A parsed `major.minor.patch` with an optional `-pre` suffix. A stable release (`pre: None`)
/// sorts after any pre-release of the same numbers, so a `-beta` tag is never offered over a
/// stable equal. (Derived `Option` ordering would say the opposite, hence the manual `Ord`.)
#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Option<String>,
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

impl Version {
    fn parse(s: &str) -> Option<Self> {
        let (core, pre) = match s.split_once('-') {
            Some((core, pre)) => (core, Some(pre.to_owned())),
            None => (s, None),
        };
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next().unwrap_or("0").parse().ok()?;
        let patch = parts.next().unwrap_or("0").parse().ok()?;
        if parts.next().is_some() {
            return None; // "1.2.3.4" is not a version we understand
        }
        Some(Version {
            major,
            minor,
            patch,
            pre,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_versions_are_up_to_date() {
        assert_eq!(
            compare("0.1.0", "0.1.0"),
            UpdateStatus::UpToDate {
                current: "0.1.0".into()
            }
        );
    }

    #[test]
    fn a_newer_latest_release_is_available() {
        assert_eq!(
            compare("0.1.0", "0.2.0"),
            UpdateStatus::Available {
                current: "0.1.0".into(),
                latest: "0.2.0".into(),
                url: RELEASES_PAGE.into(),
            }
        );
    }

    #[test]
    fn an_older_latest_release_is_up_to_date() {
        assert!(matches!(
            compare("0.2.0", "0.1.0"),
            UpdateStatus::UpToDate { .. }
        ));
    }

    #[test]
    fn a_leading_v_is_ignored() {
        assert!(matches!(
            compare("0.1.0", "v0.2.0"),
            UpdateStatus::Available { ref latest, .. } if latest == "0.2.0"
        ));
        assert!(matches!(
            compare("v0.1.0", "v0.1.0"),
            UpdateStatus::UpToDate { .. }
        ));
    }

    #[test]
    fn versions_compare_numerically_not_lexicographically() {
        // "0.10.0" > "0.9.0"; a string compare would say the opposite.
        assert!(matches!(
            compare("0.9.0", "0.10.0"),
            UpdateStatus::Available { .. }
        ));
        assert!(matches!(
            compare("0.10.0", "0.9.0"),
            UpdateStatus::UpToDate { .. }
        ));
    }

    #[test]
    fn a_pre_release_is_never_newer_than_the_stable_equal_version() {
        assert!(matches!(
            compare("0.1.0", "0.1.0-beta"),
            UpdateStatus::UpToDate { .. }
        ));
        assert!(matches!(
            compare("0.1.0", "v0.1.0-rc.1"),
            UpdateStatus::UpToDate { .. }
        ));
        // ...but a stable release upgrades a pre-release of the same version.
        assert!(matches!(
            compare("0.1.0-beta", "0.1.0"),
            UpdateStatus::Available { .. }
        ));
        // A newer pre-release still counts as newer.
        assert!(matches!(
            compare("0.1.0", "0.2.0-beta"),
            UpdateStatus::Available { .. }
        ));
    }

    #[test]
    fn short_versions_and_pre_release_names_compare_sensibly() {
        // Missing components read as zero.
        assert!(matches!(
            compare("0.1", "0.1.0"),
            UpdateStatus::UpToDate { .. }
        ));
        assert!(matches!(
            compare("1", "1.0.1"),
            UpdateStatus::Available { .. }
        ));
        // Pre-release names order alphabetically within the same numbers.
        assert!(matches!(
            compare("0.1.0-alpha", "0.1.0-beta"),
            UpdateStatus::Available { .. }
        ));
        assert!(matches!(
            compare("0.1.0-beta", "0.1.0-alpha"),
            UpdateStatus::UpToDate { .. }
        ));
    }

    #[test]
    fn a_garbage_tag_is_up_to_date_not_a_crash() {
        for tag in ["", "latest", "v", "0.x.0", "1.2.3.4", "0..1"] {
            assert!(
                matches!(compare("0.1.0", tag), UpdateStatus::UpToDate { .. }),
                "tag {tag:?} must read as up to date"
            );
        }
    }

    #[test]
    fn a_404_means_no_release_published_yet() {
        assert_eq!(
            map_response("0.1.0", 404, b"").unwrap(),
            UpdateStatus::NoReleases {
                current: "0.1.0".into()
            }
        );
    }

    #[test]
    fn a_200_maps_tag_and_url_into_the_status() {
        let body = br#"{"tag_name": "v0.2.0", "html_url": "https://github.com/viberfasend/feedrico/releases/tag/v0.2.0"}"#;
        assert_eq!(
            map_response("0.1.0", 200, body).unwrap(),
            UpdateStatus::Available {
                current: "0.1.0".into(),
                latest: "0.2.0".into(),
                url: "https://github.com/viberfasend/feedrico/releases/tag/v0.2.0".into(),
            }
        );
        let body = br#"{"tag_name": "v0.1.0", "html_url": "https://github.com/viberfasend/feedrico/releases/tag/v0.1.0"}"#;
        assert_eq!(
            map_response("0.1.0", 200, body).unwrap(),
            UpdateStatus::UpToDate {
                current: "0.1.0".into()
            }
        );
    }

    #[test]
    fn only_github_release_urls_pass_through() {
        let evil = br#"{"tag_name": "v9.9.9", "html_url": "https://evil.example/feedrico"}"#;
        assert_eq!(
            map_response("0.1.0", 200, evil).unwrap(),
            UpdateStatus::Available {
                current: "0.1.0".into(),
                latest: "9.9.9".into(),
                url: RELEASES_PAGE.into(),
            }
        );
        let missing = br#"{"tag_name": "v9.9.9"}"#;
        assert_eq!(
            map_response("0.1.0", 200, missing).unwrap(),
            UpdateStatus::Available {
                current: "0.1.0".into(),
                latest: "9.9.9".into(),
                url: RELEASES_PAGE.into(),
            }
        );
    }

    #[test]
    fn an_error_status_is_a_network_error() {
        assert!(map_response("0.1.0", 500, b"").is_err());
        assert!(map_response("0.1.0", 403, b"{}").is_err());
    }

    #[test]
    fn a_body_without_a_tag_is_an_error_not_a_crash() {
        assert!(map_response("0.1.0", 200, b"{}").is_err());
        assert!(map_response("0.1.0", 200, b"not json").is_err());
    }
}
