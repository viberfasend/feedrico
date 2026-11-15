// @vitest-environment happy-dom
// The cheap language guess behind offering Translate (#117): script first, stopwords for Latin.
import { describe, expect, it } from 'vitest';
import { guessLanguage, primarySubtag } from './assistant-language';

describe('primarySubtag', () => {
  it('reduces tags to their primary subtag', () => {
    expect(primarySubtag('de-DE')).toBe('de');
    expect(primarySubtag(' EN ')).toBe('en');
    expect(primarySubtag('pt-BR')).toBe('pt');
    expect(primarySubtag('')).toBeNull();
  });
});

describe('guessLanguage', () => {
  it('guesses by script for non-Latin text', () => {
    expect(guessLanguage('Москва — столица России, и это большой город на реке Москве.')).toBe('ru');
    expect(guessLanguage('안녕하세요, 저는 東京에서 온 사람입니다.')).toBe('ko');
    expect(guessLanguage('مرحبا بالعالم هذا نص عربي طويل بما فيه الكفاية')).toBe('ar');
    expect(guessLanguage('שלום עולם, זה טקסט בעברית')).toBe('he');
    expect(guessLanguage('Αθήνα είναι η πρωτεύουσα της Ελλάδας και μια όμορφη πόλη')).toBe('el');
  });

  it('tells Japanese from Chinese by the kana', () => {
    expect(guessLanguage('これはペンです。私は毎日学校に行きます。楽しいです。')).toBe('ja');
    expect(guessLanguage('今天天气很好，我们去公园散步吧，那里的风景非常美丽。')).toBe('zh');
  });

  it('votes stopwords for Latin-script text', () => {
    expect(
      guessLanguage(
        'Der Hund ist nicht im Haus und das ist gut so, denn der Mann ist mit dem Auto auf der Straße.',
      ),
    ).toBe('de');
    expect(
      guessLanguage(
        'Le chat est dans la maison pour une grande fête avec les amis que je ne vois pas souvent.',
      ),
    ).toBe('fr');
    expect(
      guessLanguage(
        'The quick brown fox jumps over the lazy dog and then the dog is happy with that and this.',
      ),
    ).toBe('en');
  });

  it('strips HTML and entities before guessing', () => {
    expect(guessLanguage('<p>Der &amp; die &lt;b&gt;das&lt;/b&gt; sind Artikel im Deutschen Haus.</p>')).toBe('de');
  });

  it('returns null for too little text', () => {
    expect(guessLanguage('')).toBeNull();
    expect(guessLanguage('<p>Hi</p>')).toBeNull();
    expect(guessLanguage('123 !!! …')).toBeNull();
  });
});
