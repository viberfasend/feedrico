# Library on disk: one folder with an SQLite database, a media folder and a manifest

A Library is a folder (`Feedrico Library/`) containing `library.db` (SQLite) for all structured data, `media/` for cached images and favicons, and `library.json` holding the format version. It must contain no absolute paths and work on FAT32/exFAT, so the same folder opens on Linux, Windows and macOS. We rejected one file per Article (readable, but slow at tens of thousands of Articles and fragile when a stick is pulled mid-write) and a single file (it cannot reasonably hold the image cache).

## Consequences

The format version in `library.json` is a compatibility contract between app versions, because a Library will be opened by whatever app version the friend has installed.
