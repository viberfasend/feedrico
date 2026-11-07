# Feedrico

A desktop RSS reader (the name plays on "feed" and the Italian name Federico) that keeps everything on the user's machine. All user state lives in one portable Library that can be carried to another machine and opened there with identical results.

## Language

**Library**:
The self-contained unit holding all of a user's state: Feeds, Folders, Articles, read and starred state, cached content and settings. It is the single source of truth; copies are never merged.
_Avoid_: Profile, database, account, workspace

**Feed**:
A source the user follows (RSS, Atom or JSON Feed), identified by its feed URL. Sits in at most one Folder, or at the top level.
_Avoid_: Subscription, channel, source

**Folder**:
A named group of Feeds, one level deep.
_Avoid_: Category, tag, group

**Article**:
A single entry published by a Feed. Belongs to exactly one Feed; the same post in two Feeds is two Articles.
_Avoid_: Item, entry, post, story

**Article Date**:
The publish date an Article claims if present and plausible, otherwise the moment the app first saw it. Never in the future.
_Avoid_: Timestamp, pubDate, created

**Starred**:
An Article the user marked to keep. Starred Articles are never pruned.
_Avoid_: Saved, favourite, bookmarked

**Read**:
The state of an Article the user has opened in the reader or explicitly marked as read. Tracked per Article.
_Avoid_: Seen, viewed, opened

**Smart View**:
A built-in list of Articles drawn from all Feeds by a fixed rule: All Unread, Starred, Today (Article Date since local midnight).
_Avoid_: Filter, saved search, virtual folder

**Portable Mode**:
Running the app from the same place as its Library, such as a USB stick, so nothing needs to be installed on the host machine.
_Avoid_: Stick mode, USB mode

**Pruning**:
The automatic removal of old Articles from the Library according to retention rules.
_Avoid_: Cleanup, expiry, garbage collection
