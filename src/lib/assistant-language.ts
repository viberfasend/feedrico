// The Article's language, used only to decide whether Translate is offered (#117).
//
// The Library stores no Feed language (this wave makes no Library format change), so the
// Article's language is a cheap guess from its own text: the writing system first, then a stopword
// vote for Latin-script languages. A wrong guess only means Translate is offered (or not) wrongly,
// so cheap beats exact here. The target language from the machine-local setup is compared by its
// primary subtag, so "de" and "de-DE" match.

/** The primary subtag of a BCP 47 tag, lowercased: "de-DE" -> "de". Null when empty. */
export function primarySubtag(language: string): string | null {
  const tag = language.trim().split('-')[0]?.trim().toLowerCase() ?? '';
  return tag || null;
}

// Scripts that map to one language code. Han is shared between Chinese and Japanese, so the
// unambiguous kana (Japanese) wins when present; Cyrillic covers ru/uk/bg and guesses "ru".
function scriptOf(ch: string): string | null {
  const cp = ch.codePointAt(0)!;
  if (cp >= 0x0400 && cp <= 0x04ff) return 'ru'; // Cyrillic
  if (cp >= 0x0600 && cp <= 0x06ff) return 'ar'; // Arabic
  if (cp >= 0x0590 && cp <= 0x05ff) return 'he'; // Hebrew
  if (cp >= 0x0370 && cp <= 0x03ff) return 'el'; // Greek
  if (cp >= 0x0e00 && cp <= 0x0e7f) return 'th'; // Thai
  if (cp >= 0x0900 && cp <= 0x097f) return 'hi'; // Devanagari
  if (cp >= 0xac00 && cp <= 0xd7af) return 'ko'; // Hangul
  if (cp >= 0x3040 && cp <= 0x30ff) return 'ja'; // Hiragana / Katakana
  if (cp >= 0x4e00 && cp <= 0x9fff) return 'zh'; // Han
  if (
    (cp >= 0x41 && cp <= 0x5a) ||
    (cp >= 0x61 && cp <= 0x7a) ||
    (cp >= 0xc0 && cp <= 0xff && cp !== 0xd7 && cp !== 0xf7)
  ) {
    return 'latin';
  }
  return null;
}

// Stopword votes for Latin-script languages. Distinctive words only; ties and no matches fall
// back to English, which is also the guess for any Latin text without known stopwords.
const STOPWORDS: [string, string[]][] = [
  ['de', ['der', 'die', 'das', 'und', 'ist', 'nicht', 'mit', 'für', 'auf', 'den', 'dem', 'des']],
  ['fr', ['le', 'la', 'les', 'des', 'une', 'est', 'dans', 'pour', 'pas', 'que', 'qui', 'sur']],
  ['es', ['el', 'la', 'los', 'las', 'que', 'para', 'con', 'una', 'por', 'del', 'les', 'está']],
  ['pt', ['que', 'para', 'com', 'uma', 'não', 'mais', 'como', 'por', 'dos', 'das', 'está']],
  ['it', ['il', 'lo', 'la', 'di', 'che', 'per', 'con', 'una', 'non', 'sono', 'gli', 'nel']],
  ['nl', ['de', 'het', 'een', 'van', 'niet', 'met', 'voor', 'zijn', 'op', 'aan', 'dat', 'die']],
  ['en', ['the', 'and', 'of', 'to', 'in', 'is', 'that', 'for', 'with', 'are', 'was', 'this']],
];

/** At least this many Latin letters before a stopword vote is trusted. */
const MIN_LATIN = 20;

/** A cheap guess of the text's language as a primary subtag, or null when there is too little text. */
export function guessLanguage(raw: string): string | null {
  const text = raw.replace(/<[^>]*>/g, ' ').replace(/&[a-zA-Z#0-9]+;/g, ' ');
  const counts = new Map<string, number>();
  let latin = 0;
  for (const ch of text) {
    const script = scriptOf(ch);
    if (!script) continue;
    if (script === 'latin') {
      latin++;
    } else {
      counts.set(script, (counts.get(script) ?? 0) + 1);
    }
  }
  const nonLatin = [...counts.entries()].filter(([s]) => s !== 'latin');
  if (nonLatin.length > 0) {
    // Kana means Japanese even when Han characters dominate.
    if ((counts.get('ja') ?? 0) > 0) return 'ja';
    return nonLatin.sort((a, b) => b[1] - a[1])[0]![0];
  }
  if (latin < MIN_LATIN) return null;
  const words = text.toLowerCase().match(/[a-zà-ÿ]+/g) ?? [];
  const votes = new Map<string, number>();
  for (const word of words) {
    for (const [lang, list] of STOPWORDS) {
      if (list.includes(word)) votes.set(lang, (votes.get(lang) ?? 0) + 1);
    }
  }
  let best: string | null = null;
  let bestCount = 0;
  for (const [lang, count] of votes) {
    if (count > bestCount) {
      best = lang;
      bestCount = count;
    }
  }
  return best ?? 'en';
}
