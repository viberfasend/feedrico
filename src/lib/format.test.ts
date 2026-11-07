import { describe, expect, it } from 'vitest';
import { fileSize, hostOf, shortDate } from './format';

describe('shortDate', () => {
  const now = new Date(2026, 10, 7, 15, 0, 0);
  const at = (d: Date) => Math.floor(d.getTime() / 1000);

  it('shows the time for today', () => {
    expect(shortDate(at(new Date(2026, 10, 7, 9, 5)), now)).toMatch(/09.05|9.05/);
  });
  it('says Yesterday', () => {
    expect(shortDate(at(new Date(2026, 10, 6, 23, 0)), now)).toBe('Yesterday');
  });
  it('adds the year for older dates', () => {
    expect(shortDate(at(new Date(2024, 0, 2)), now)).toMatch(/2024/);
  });
});

describe('helpers', () => {
  it('extracts hosts', () => {
    expect(hostOf('https://www.example.org/a')).toBe('example.org');
    expect(hostOf('nope')).toBe('');
  });
  it('formats sizes', () => {
    expect(fileSize(1536)).toBe('1.5 KB');
    expect(fileSize(null)).toBe('');
  });
});
