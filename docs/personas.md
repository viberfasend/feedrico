# Personas

Who we design and test Feedrico for. Use them in every design session, ADR, issue and release
test: name the personas a decision helps or hurts, and walk their test scenarios.

> **Status: proto-personas.** These are explicit assumptions, not research results. They are
> grounded in practitioner writing about feed readers and in one real Library (the Archivist's
> numbers). Revise them as soon as real users tell us otherwise.

Each persona has a **role name** (canonical: use it in issues, tests and decisions) and a first
name for empathy. Vocabulary follows [`GLOSSARY.md`](../GLOSSARY.md); personas themselves are
not domain terms.

## Research basis

- Two reader philosophies: *Inbox Zero* (prune Feeds until the unread count is manageable) and
  *"RSS is not email"* (hundreds unread is fine; read Today or a river, let things expire).
- Long-time users from the Google Reader era often follow 100+ Feeds and check several times a
  day; a younger group returns to RSS out of fatigue with algorithmic feeds.
- Heavy users read backlogs in batches and rely on search and expiry rather than reading
  everything.
- Carried data breaks at version, path and file-system boundaries between machines.
- Method: proto-personas with Jobs-to-be-Done, plus an inclusive-design persona spectrum for
  permanent, temporary and situational limits.

Sources: [Digg Google Reader survey](https://martech.org/digg-does-the-google-reader-survey-that-google-shouldve-done),
[Feed reader usage 2026](https://dev.to/stanlymt/feed-reader-usage-in-2026-a-data-snapshot-c1),
[Marco Arment: The power of the RSS reader](https://Marco.org/2013/03/26/power-of-rss),
[Dan Q: RSS Zero isn't the path to RSS Joy](https://bcdn.danq.me/2023/07/29/rss-zero/),
[Sebastian De Deyne on RSS readers](https://sebastiandedeyne.com/current-rss-reader/),
[NewsBlur forum on batch reading](https://forum.newsblur.com/t/feed-cut-off-after-100-entries/1554?page=3),
[JTBD and personas](https://uxplanet.org/can-jobs-to-be-done-research-and-personas-co-exist-should-they-25e632b48665),
[Persona spectrums](https://careerfoundry.com/en/blog/ux-design/persona-spectrums/),
[PortableApps forum on version mismatches](https://portableapps.com/node/10915).

## Overview

| Role | Name | In one line | Reference Library |
|---|---|---|---|
| [The Newcomer](#the-newcomer) | Mira | First feed reader ever, wants to start in two minutes | 0 → 20 Feeds |
| [The Archivist](#the-archivist) | Henrik | 1,000+ Feeds, 10k+ unread, keyboard only | 1,035 Feeds · 10 Folders · 10,630 Articles · 357 broken Feeds · 119 MB |
| [The Skimmer](#the-skimmer) | Clara | Fifteen minutes with coffee, never at zero | 40 Feeds · 3 Folders · ~600 Articles, image-heavy |
| [The Carrier](#the-carrier) | Jonas | Library on a stick, between home, office and friends | 150 Feeds · 8 Folders · ~4,000 Articles · 500 MB stick, exFAT |
| [The Host](#the-host) | Ute | The friend whose machine the stick is plugged into | Own Library: 30 Feeds, older Feedrico |
| [The Offline Reader](#the-offline-reader) | Paul | Reads on the train, checks that nothing phones home | 80 Feeds · ~2,000 Articles |

Plus the [accessibility persona spectrum](#accessibility-persona-spectrum) and the
[anti-persona](#anti-persona-the-synced-commuter).

---

## The Newcomer

**Mira** · *"I just want the blogs I like, without an algorithm deciding for me."*

- **Context:** has never used a feed reader. Maybe has an OPML export from Feedly, a podcast app
  or a friend; maybe only a handful of website addresses. Doesn't know what RSS, OPML or a
  Library is, and shouldn't need to.
- **Job-to-be-Done:** when I'm tired of algorithmic feeds, I want to get my first twenty Feeds
  going in two minutes, so I can start reading today.
- **Needs:** one obvious first step, sensible defaults (Library location), plain words, empty
  states that say what to do next, adding a Feed from a normal website address.
- **Frustrations:** jargon, choices before value, an empty screen with no hint, errors that blame
  the user.
- **Test scenarios:**
  1. Fresh install → welcome screen → *Create new Library* → import an OPML file → first Articles
     appear without pressing anything else.
  2. No OPML: add three Feeds by pasting website addresses (not feed URLs).
  3. Import a broken or non-OPML file: the message says what went wrong and what to do.
  4. Every empty state (no Feeds, all caught up, no search results) offers a next step.
  5. Read every word on screen once: no unexplained jargon.

## The Archivist

**Henrik** · *"I've followed some of these blogs for fifteen years. Don't lose a single post, and
don't make me touch the mouse."*

- **Context:** Google Reader veteran. 1,035 Feeds, hundreds of them dead or flaky, 10k+ unread at
  any time. Reads in batches, searches the archive for things read years ago, organises with
  Folders. *Numbers taken from a real Library.*
- **Job-to-be-Done:** when a batch of reading time opens up, I want to move through thousands of
  Articles at full speed, so I can keep up with everything I care about without losing
  anything.
- **Needs:** the performance budgets at this scale, complete keyboard control, reliable search,
  *mark all as read* with undo, Starred kept forever, clear but quiet handling of broken Feeds,
  predictable Pruning.
- **Frustrations:** jank, spinners, anything that needs the mouse, silent data loss, a noisy UI
  for 357 broken Feeds.
- **Test scenarios:**
  1. Open a 1,000-Feed / 10k-Article Library: within the cold-start budget, list scrolls at 60 fps
     in every Layout.
  2. Read 50 Articles with `j`/`k`/`s`/`m` only; switch Views and Layouts by keyboard.
  3. Search for a phrase from an Article months old; scope it to a Folder.
  4. *Mark all as read* on a Folder, then undo.
  5. Refresh with hundreds of broken Feeds: no UI jank, badges are quiet, backoff respected.
  6. Pruning never removes a Starred Article.

## The Skimmer

**Clara** · *"I'll never be at zero, and that's fine. Show me what's good today."*

- **Context:** about 40 Feeds: a few news sites, design blogs, photo blogs, two webcomics. Fifteen
  minutes each morning. Lives in *Today*, Magazine and Cards; uses Gallery for the photo Feeds.
  Never looks at unread counts.
- **Job-to-be-Done:** when I have a coffee's worth of time, I want a beautiful overview of what's
  new, so I can enjoy a few good reads without feeling behind.
- **Needs:** good-looking Layouts, Lead Images that actually show, a calm Today View, each View
  remembering its Layout, nothing nagging about unread counts.
- **Frustrations:** walls of text, broken or missing images, guilt-inducing counters.
- **Test scenarios:**
  1. Morning routine: open the app → *Today* in Magazine → read three Articles → close.
  2. Photo Feed in Gallery, news Folder in List, Today in Magazine; all remembered after restart.
  3. A View with few or no images in Cards and Gallery looks deliberate, not broken.
  4. Both themes and both densities look polished in every Layout.

## The Carrier

**Jonas** · *"My reading goes where I go: home, office, my friend's place."*

- **Context:** keeps the Library on a USB stick (exFAT, sometimes FAT32). Uses the installed app at
  home, sometimes Portable Mode from the stick, sometimes a friend's installed Feedrico. Versions
  differ between these machines. Sometimes pulls the stick without closing the app.
- **Job-to-be-Done:** when I'm at another machine, I want my exact Library with the same Read and
  Starred state, so I can continue where I left off.
- **Needs:** the Library always self-contained and safe to copy, clear upgrade decisions with a
  backup, locks that protect without nagging, Portable Mode that just works.
- **Frustrations:** a Library that won't open, an upgrade that strands him at home, duplicate or
  lost state, absolute paths, drive-letter changes.
- **Test scenarios:** walk the [version scenario matrix](#version-scenario-matrix) A–E, plus:
  1. Copy the closed Library folder to a stick with a file manager, open it on another machine:
     identical Feeds, Articles, Read/Starred, Layouts, images.
  2. Different mount points or drive letters on each machine.
  3. Stick with little free space: refresh and image caching fail gracefully.

## The Host

**Ute** · *"Sure, plug it in. Just don't mess up my stuff."*

- **Context:** Jonas's friend. Different OS (often Windows), possibly no admin rights, possibly no
  internet at the moment. Has an older Feedrico installed and never updates it, because there's no
  auto-update. Has her own Library.
- **Job-to-be-Done:** when a friend brings their Library, I want them to read it on my machine
  without affecting my own setup, so I can help without risk.
- **Needs:** her own Library and recent list untouched (or easily restored), a clear message when
  her app is too old, nothing installed or changed behind her back.
- **Frustrations:** her Library replaced, confusing errors, being asked to update, leftovers on
  her machine.
- **Test scenarios:**
  1. Open Jonas's Library from the stick, then switch back to her own via *Recent Libraries*: her
     Library is unchanged.
  2. Her app is older than Jonas's Library (scenario B): clear message, nothing written to the
     stick.
  3. Run Jonas's Portable Mode build on her machine without installing anything.
  4. No internet: the Library still opens and reads from the cache.

## The Offline Reader

**Paul** · *"If it makes a request I didn't ask for, I'll notice."*

- **Context:** reads on the train with a flaky or no connection. Privacy-minded and technical;
  watches network traffic now and then. About 80 Feeds.
- **Job-to-be-Done:** when I'm offline, I want everything I fetched earlier, images included, to
  be readable, and I want reading never to make network requests, so I'm not tracked.
- **Needs:** the image cache filled at fetch time, no requests when opening Articles, links that
  only open when clicked, no telemetry, honest status when fetching fails.
- **Frustrations:** broken images offline, tracking pixels, surprise requests, vague network
  errors.
- **Test scenarios:**
  1. Refresh online, go offline, read 20 Articles: images present, no request attempts.
  2. Watch traffic while reading: requests go only to Feed and website hosts during refresh.
  3. Refresh while offline: Feeds fail calmly, with clear status and no error flood.

---

## Version scenario matrix

Every change to the Library format, locking or startup must walk all five scenarios with the
[Carrier](#the-carrier) and the [Host](#the-host).

| | Situation | Expected |
|---|---|---|
| **A** | Carrier's Library is **older** than the Host's app | The Host's app asks *"Upgrade this Library? Older versions won't open it afterwards."*, backs up the database, upgrades. Back home, the Carrier's older app refuses with *"needs Feedrico X"* and changes nothing. |
| **B** | Carrier's Library is **newer** than the Host's app | *"This Library needs a newer Feedrico."* Nothing is written to the stick. |
| **C** | Same version, **different OS** (Linux Library on a Windows Host) | Opens identically; no paths, permissions or filenames break. |
| **D** | Stick **pulled mid-session** at the Host | At most the last action is lost; back home the lock is shown as stale with machine and time, and *Open anyway* works. |
| **E** | Carrier runs **Portable Mode** from the stick | App and Library travel together; no version mismatch, the Host's own setup is untouched. |

## Accessibility persona spectrum

Check every UI change against each spectrum, not just one point on it.

| Need | Permanent | Temporary | Situational | What it demands |
|---|---|---|---|---|
| Can't use a mouse | Motor impairment | Broken arm | Laptop on the train, no mouse | Everything works by keyboard; visible focus; no hover-only actions |
| Can't see the screen well | Blind, uses a screen reader | Migraine, eye strain | Bright sunlight on the screen | Labels and roles for every control; AA contrast in both themes; text scales |
| Limited attention | ADHD | Exhausted | Reading between meetings | Calm UI, no surprise motion, state survives interruptions, clear next step |

## Anti-persona: the Synced Commuter

**Who Feedrico is deliberately not for (v1):** someone who wants Read state live-synced between
phone and laptop, push notifications, and an account with a cloud backend.

Why it matters: these wishes conflict with the core decisions (a local-only, single-source-of-truth
Library: [ADR 0001](adr/0001-library-is-single-source-of-truth.md)). When a request comes from
this direction, the answer is "not in v1", and this section explains why.

## How to use the personas

- **Design sessions and ADRs:** name the personas a decision helps and hurts. A decision that
  hurts the Archivist's speed or the Carrier's data safety needs a strong reason.
- **Issues:** fill in the *Personas* line of the issue template.
- **Tests:** the scenarios above are the manual release checklist; reference Library numbers feed
  fixtures and benchmarks (the Archivist's numbers drive the performance harness).
- **Revise:** when a real user contradicts a persona, update the persona, not the user.
