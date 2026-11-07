const time = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });
const dayMonth = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short' });
const full = new Intl.DateTimeFormat(undefined, { dateStyle: 'full', timeStyle: 'short' });
const withYear = new Intl.DateTimeFormat(undefined, {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
});

/** Compact Article Date for list rows: "14:05", "Yesterday", "7 Nov", "7 Nov 2024". */
export function shortDate(unixSeconds: number, now = new Date()): string {
  const date = new Date(unixSeconds * 1000);
  const startOfToday = new Date(now);
  startOfToday.setHours(0, 0, 0, 0);
  const dayMs = 86_400_000;
  if (date >= startOfToday) return time.format(date);
  if (date.getTime() >= startOfToday.getTime() - dayMs) return 'Yesterday';
  if (date.getFullYear() === now.getFullYear()) return dayMonth.format(date);
  return withYear.format(date);
}

export function longDate(unixSeconds: number): string {
  return full.format(new Date(unixSeconds * 1000));
}

export function hostOf(url: string | null | undefined): string {
  if (!url) return '';
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return '';
  }
}

export function fileSize(bytes: number | null): string {
  if (!bytes) return '';
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}
