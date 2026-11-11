<script lang="ts">
  import {
    IMAGE_CACHE_CAPS,
    PRUNE_READ_OPTIONS,
    PRUNE_UNREAD_OPTIONS,
    REFRESH_INTERVALS,
    type Settings,
  } from '../api';
  import { fileSize } from '../format';
  import { app } from '../state.svelte';
  import type { Density, Theme } from '../tokens';
  import Dialog from '../ui/Dialog.svelte';
  import Input from '../ui/Input.svelte';
  import Tabs from '../ui/Tabs.svelte';
  import Toggle from '../ui/Toggle.svelte';

  let tab = $state('library');

  const s = $derived(app.settings);

  // Changes apply immediately (no Save button): each control writes the whole struct to the
  // Library; the core validates and rejects out-of-range values.
  function update(next: Settings) {
    void app.updateSettings(next);
  }
  const setAppearance = (patch: Partial<Settings['appearance']>) =>
    s && update({ ...s, appearance: { ...s.appearance, ...patch } });
  const setReader = (patch: Partial<Settings['reader']>) =>
    s && update({ ...s, reader: { ...s.reader, ...patch } });
  const setRefresh = (patch: Partial<Settings['refresh']>) =>
    s && update({ ...s, refresh: { ...s.refresh, ...patch } });
  const setFlat = (
    patch: Partial<Pick<Settings, 'imageCacheCapBytes' | 'pruneReadDays' | 'pruneUnreadDays'>>,
  ) => s && update({ ...s, ...patch });

  function onSelect(event: Event, apply: (value: string) => void) {
    apply((event.currentTarget as HTMLSelectElement).value);
  }

  /** The ranges the core accepts (settings.rs); anything outside is pulled back in. */
  const LIMITS = { fontSize: [14, 24], measure: [50, 90], lineHeight: [1.3, 2] } as const;

  function onNumber(field: keyof typeof LIMITS, event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const raw = Number(input.value);
    if (!input.value.trim() || !Number.isFinite(raw) || !s) {
      input.value = String(s?.reader[field] ?? '');
      return;
    }
    const [min, max] = LIMITS[field];
    const rounded = field === 'lineHeight' ? Math.round(raw * 100) / 100 : Math.round(raw);
    const value = Math.min(max, Math.max(min, rounded));
    input.value = String(value);
    setReader({ [field]: value } as Partial<Settings['reader']>);
  }

  const intervalLabel = (minutes: number) =>
    minutes < 60 ? `Every ${minutes} minutes` : minutes === 60 ? 'Every hour' : `Every ${minutes / 60} hours`;
</script>

<Dialog bind:open={app.settingsOpen} title="Settings" class="settings-dialog">
  {#if s}
    <Tabs
      bind:value={tab}
      tabs={[
        { value: 'library', label: 'Library' },
        { value: 'machine', label: 'This computer' },
      ]}
    >
      {#snippet children(value)}
        {#if value === 'library'}
          <div class="pane">
            <section aria-label="Appearance">
              <h3>Appearance</h3>
              <label class="field">
                <span>Theme</span>
                <select
                  class="select"
                  value={s.appearance.theme}
                  onchange={(e) => onSelect(e, (v) => setAppearance({ theme: v as Theme }))}
                >
                  <option value="system">Follow OS</option>
                  <option value="light">Light</option>
                  <option value="dark">Dark</option>
                </select>
              </label>
              <label class="field">
                <span>Density</span>
                <select
                  class="select"
                  value={s.appearance.density}
                  onchange={(e) => onSelect(e, (v) => setAppearance({ density: v as Density }))}
                >
                  <option value="comfortable">Comfortable</option>
                  <option value="compact">Compact</option>
                </select>
              </label>
            </section>

            <section aria-label="Reader typography">
              <h3>Reader typography</h3>
              <div class="field-grid">
                <label class="field">
                  <span>Font</span>
                  <select
                    class="select"
                    value={s.reader.fontFamily}
                    onchange={(e) => onSelect(e, (v) => setReader({ fontFamily: v as Settings['reader']['fontFamily'] }))}
                  >
                    <option value="serif">Serif</option>
                    <option value="sans">Sans</option>
                  </select>
                </label>
                <label class="field">
                  <span>Size (px)</span>
                  <Input
                    type="number"
                    min={14}
                    max={24}
                    value={String(s.reader.fontSize)}
                    aria-label="Reader font size in pixels"
                    onchange={(e: Event) => onNumber('fontSize', e)}
                  />
                </label>
                <label class="field">
                  <span>Line width (ch)</span>
                  <Input
                    type="number"
                    min={50}
                    max={90}
                    value={String(s.reader.measure)}
                    aria-label="Reader line width in characters"
                    onchange={(e: Event) => onNumber('measure', e)}
                  />
                </label>
                <label class="field">
                  <span>Line height</span>
                  <Input
                    type="number"
                    min={1.3}
                    max={2}
                    step={0.05}
                    value={String(s.reader.lineHeight)}
                    aria-label="Reader line height"
                    onchange={(e: Event) => onNumber('lineHeight', e)}
                  />
                </label>
              </div>
              <p class="preview">
                Altweibersommer: warm light on the old garden wall, and the Articles read like
                letters from a friend.
              </p>
            </section>

            <section aria-label="Refresh">
              <h3>Refresh</h3>
              <label class="field">
                <span>Refresh Feeds</span>
                <select
                  class="select"
                  value={String(s.refresh.intervalMinutes)}
                  onchange={(e) =>
                    onSelect(e, (v) => setRefresh({ intervalMinutes: Number(v) }))}
                >
                  {#each REFRESH_INTERVALS as minutes (minutes)}
                    <option value={String(minutes)}>{intervalLabel(minutes)}</option>
                  {/each}
                </select>
              </label>
            </section>

            <section aria-label="Retention">
              <h3>Retention</h3>
              <label class="field">
                <span>Keep Read Articles for</span>
                <select
                  class="select"
                  value={String(s.pruneReadDays)}
                  onchange={(e) => onSelect(e, (v) => setFlat({ pruneReadDays: Number(v) }))}
                >
                  {#each PRUNE_READ_OPTIONS as days (days)}
                    <option value={String(days)}>{days} days</option>
                  {/each}
                </select>
              </label>
              <label class="field">
                <span>Keep Unread Articles for</span>
                <select
                  class="select"
                  value={String(s.pruneUnreadDays)}
                  onchange={(e) => onSelect(e, (v) => setFlat({ pruneUnreadDays: Number(v) }))}
                >
                  {#each PRUNE_UNREAD_OPTIONS as days (days)}
                    <option value={String(days)}>{days} days</option>
                  {/each}
                </select>
              </label>
            </section>

            <section aria-label="Image cache">
              <h3>Image cache</h3>
              <label class="field">
                <span>Cap</span>
                <select
                  class="select"
                  value={String(s.imageCacheCapBytes)}
                  onchange={(e) =>
                    onSelect(e, (v) => setFlat({ imageCacheCapBytes: Number(v) }))}
                >
                  {#each IMAGE_CACHE_CAPS as cap (cap)}
                    <option value={String(cap)}>{fileSize(cap)}</option>
                  {/each}
                </select>
              </label>
              <p class="hint">Currently using {fileSize(app.mediaSize)} of cached images.</p>
            </section>

            <section aria-label="About this Library">
              <h3>Library</h3>
              <dl class="facts">
                <div><dt>Location</dt><dd class="path" title={app.library?.path}>{app.library?.path}</dd></div>
                <div><dt>Size</dt><dd>{fileSize(app.librarySize)}</dd></div>
                <div><dt>Format version</dt><dd>{app.library?.formatVersion}</dd></div>
              </dl>
            </section>
          </div>
        {:else}
          <div class="pane">
            <section aria-label="This computer">
              <h3>This computer</h3>
              <div class="field">
                <Toggle
                  pressed={app.autostart ?? false}
                  disabled={app.library?.portable ?? false}
                  onchange={(pressed) => void app.setAutostart(pressed)}
                  label="Start Feedrico with the OS"
                >
                  Start with OS
                </Toggle>
                {#if app.library?.portable}
                  <p class="hint">Not available in Portable Mode</p>
                {/if}
              </div>
            </section>
          </div>
        {/if}
      {/snippet}
    </Tabs>
  {/if}
</Dialog>

<style>
  .pane {
    display: grid;
    gap: var(--space-stack-lg);
  }
  section {
    display: grid;
    gap: var(--space-stack-sm);
  }
  h3 {
    font-size: var(--font-size-small);
    font-weight: var(--font-weight-strong);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .field {
    display: flex;
    align-items: center;
    gap: var(--space-inline-md);
    justify-content: space-between;
  }
  .field > span {
    color: var(--color-text-default);
  }
  .field-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: var(--space-stack-sm);
  }
  .field-grid .field {
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-stack-2xs);
    justify-content: start;
  }
  .select {
    height: var(--input-height);
    padding-inline: var(--input-padding-inline);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-base);
    color: var(--color-text-default);
    font: inherit;
  }
  .preview {
    max-width: var(--reader-measure);
    font-family: var(--reader-font-family);
    font-size: var(--reader-font-size);
    line-height: var(--reader-line-height);
    color: var(--color-text-default);
  }
  .hint {
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .facts {
    display: grid;
    gap: var(--space-stack-2xs);
    margin: 0;
  }
  .facts > div {
    display: flex;
    gap: var(--space-inline-md);
  }
  .facts dt {
    flex: none;
    width: 9rem;
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .facts dd {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
