<script lang="ts">
  // Settings, laid out like a desktop preferences window: sections on the left, one pane on the
  // right. Every control writes through at once (no Save button); the core validates.
  import { Tabs } from 'bits-ui';
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
  import IconButton from '../ui/IconButton.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Select from '../ui/Select.svelte';
  import Slider from '../ui/Slider.svelte';
  import Toggle from '../ui/Toggle.svelte';

  const SECTIONS = [
    { value: 'appearance', label: 'Appearance' },
    { value: 'reading', label: 'Reading' },
    { value: 'storage', label: 'Refresh and storage' },
    { value: 'library', label: 'About this Library' },
    { value: 'machine', label: 'This computer' },
  ] as const;
  type Section = (typeof SECTIONS)[number]['value'];

  let section = $state<Section>('appearance');
  const s = $derived(app.settings);

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

  /** The ranges the core accepts (settings.rs); anything outside is pulled back in. */
  const LIMITS = { fontSize: [14, 24], measure: [50, 90], lineHeight: [1.3, 2] } as const;
  type ReaderNumber = keyof typeof LIMITS;

  /** Slider values while dragging: the preview follows at once, the Library on release. */
  let draft = $state<Partial<Record<ReaderNumber, number>>>({});
  const reader = $derived(s ? { ...s.reader, ...draft } : null);

  function commitNumber(field: ReaderNumber, event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    delete draft[field];
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
  const daysLabel = (days: number) => (days % 365 === 0 ? (days === 365 ? '1 year' : `${days / 365} years`) : `${days} days`);

  const THEMES: { value: Theme; label: string }[] = [
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
    { value: 'system', label: 'Follow OS' },
  ];
  const themeName = $props.id();
</script>

{#snippet miniWindow(theme: 'light' | 'dark')}
  <span class="mini" data-theme={theme} aria-hidden="true">
    <span class="mini-sidebar">
      <span class="mini-line short"></span>
      <span class="mini-line"></span>
      <span class="mini-line"></span>
    </span>
    <span class="mini-list">
      <span class="mini-row"><span class="mini-dot"></span><span class="mini-title"></span></span>
      <span class="mini-row"><span class="mini-dot"></span><span class="mini-title short"></span></span>
      <span class="mini-row read"><span class="mini-dot"></span><span class="mini-title"></span></span>
    </span>
  </span>
{/snippet}

<Dialog bind:open={app.settingsOpen} title="Settings" class="settings-dialog">
  {#if s && reader}
    <Tabs.Root bind:value={section} orientation="vertical">
      {#snippet child({ props })}
        <div {...props} class="settings">
          <Tabs.List>
            {#snippet child({ props: listProps })}
              <div {...listProps} class="nav" aria-label="Settings sections">
                {#each SECTIONS as item (item.value)}
                  <Tabs.Trigger value={item.value}>
                    {#snippet child({ props: triggerProps })}
                      <button {...triggerProps} type="button" class="nav-item" class:apart={item.value === 'machine'}>
                        {item.label}
                      </button>
                    {/snippet}
                  </Tabs.Trigger>
                {/each}
              </div>
            {/snippet}
          </Tabs.List>

          {#each SECTIONS as item (item.value)}
            <Tabs.Content value={item.value}>
              {#snippet child({ props: contentProps })}
                <div {...contentProps} class="pane">
                  <header class="pane-head">
                    <h3>{item.label}</h3>
                    <p class="where">
                      {item.value === 'machine'
                        ? 'Kept on this computer only. Another computer opening this Library has its own.'
                        : 'Saved in this Library, so it comes along to every computer that opens it.'}
                    </p>
                  </header>

                  {#if item.value === 'appearance'}
                    <div class="block">
                      <div class="block-head">
                        <span class="label">Theme</span>
                      </div>
                      <div class="themes" role="radiogroup" aria-label="Theme">
                        {#each THEMES as theme (theme.value)}
                          <label class="theme" class:active={s.appearance.theme === theme.value}>
                            <input
                              type="radio"
                              name={themeName}
                              value={theme.value}
                              checked={s.appearance.theme === theme.value}
                              onchange={() => setAppearance({ theme: theme.value })}
                            />
                            <span class="theme-preview">
                              {#if theme.value === 'system'}
                                <span class="split">{@render miniWindow('light')}</span>
                                <span class="split dark">{@render miniWindow('dark')}</span>
                              {:else}
                                {@render miniWindow(theme.value)}
                              {/if}
                            </span>
                            <span class="theme-name">{theme.label}</span>
                          </label>
                        {/each}
                      </div>
                    </div>
                    <div class="group">
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Density</span>
                          <span class="desc">Compact fits more Articles and Feeds on screen.</span>
                        </div>
                        <Segmented
                          label="Density"
                          value={s.appearance.density}
                          options={[
                            { value: 'comfortable', label: 'Comfortable' },
                            { value: 'compact', label: 'Compact' },
                          ]}
                          onchange={(v: Density) => setAppearance({ density: v })}
                        />
                      </div>
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Unread counts</span>
                          <span class="desc">A dot says “something new” without a number to chase.</span>
                        </div>
                        <Segmented
                          label="Unread counts"
                          value={s.appearance.unreadCounts ?? 'show'}
                          options={[
                            { value: 'show', label: 'Numbers' },
                            { value: 'hide', label: 'Dot' },
                          ]}
                          onchange={(v: 'show' | 'hide') => setAppearance({ unreadCounts: v })}
                        />
                      </div>
                    </div>
                  {:else if item.value === 'reading'}
                    <div class="group">
                      <div class="row">
                        <div class="row-text"><span class="label">Typeface</span></div>
                        <Segmented
                          label="Typeface"
                          value={s.reader.fontFamily}
                          options={[
                            { value: 'serif', label: 'Literata', font: 'var(--font-family-reading)' },
                            { value: 'sans', label: 'Inter', font: 'var(--font-family-ui)' },
                          ]}
                          onchange={(v: Settings['reader']['fontFamily']) => setReader({ fontFamily: v })}
                        />
                      </div>
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Text size</span>
                          <span class="desc value">{reader.fontSize} px</span>
                        </div>
                        <Slider
                          label="Reader font size in pixels"
                          min={14}
                          max={24}
                          value={reader.fontSize}
                          oninput={(v) => (draft.fontSize = v)}
                          onchange={(e) => commitNumber('fontSize', e)}
                        />
                      </div>
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Line width</span>
                          <span class="desc value">{reader.measure} characters</span>
                        </div>
                        <Slider
                          label="Reader line width in characters"
                          min={50}
                          max={90}
                          value={reader.measure}
                          oninput={(v) => (draft.measure = v)}
                          onchange={(e) => commitNumber('measure', e)}
                        />
                      </div>
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Line spacing</span>
                          <span class="desc value">{reader.lineHeight.toFixed(2)}</span>
                        </div>
                        <Slider
                          label="Reader line height"
                          min={1.3}
                          max={2}
                          step={0.05}
                          value={reader.lineHeight}
                          oninput={(v) => (draft.lineHeight = v)}
                          onchange={(e) => commitNumber('lineHeight', e)}
                        />
                      </div>
                    </div>
                    <figure class="page" aria-label="Preview">
                      <div
                        class="page-text"
                        style:font-family={reader.fontFamily === 'serif' ? 'var(--font-family-reading)' : 'var(--font-family-ui)'}
                        style:font-size="{reader.fontSize}px"
                        style:line-height={reader.lineHeight}
                        style:max-width="{reader.measure}ch"
                      >
                        <p class="page-source">Gardening Notes</p>
                        <p class="page-title">Altweibersommer</p>
                        <p>
                          Warm light on the old garden wall, the last figs going soft on the branch,
                          and the spiders’ threads drifting across the path at dusk. Some
                          afternoons ask for nothing more than a chair in the sun and a long letter
                          from a friend.
                        </p>
                      </div>
                    </figure>
                  {:else if item.value === 'storage'}
                    <div class="group">
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Check for new Articles</span>
                          <span class="desc">Refresh always works on demand, too (r).</span>
                        </div>
                        <div class="control">
                          <Select
                            label="Refresh Feeds"
                            value={String(s.refresh.intervalMinutes)}
                            options={REFRESH_INTERVALS.map((m) => ({ value: String(m), label: intervalLabel(m) }))}
                            onchange={(v) => setRefresh({ intervalMinutes: Number(v) })}
                          />
                        </div>
                      </div>
                    </div>
                    <div class="group">
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Keep read Articles</span>
                          <span class="desc">Starred Articles are always kept.</span>
                        </div>
                        <div class="control">
                          <Select
                            label="Keep read Articles for"
                            value={String(s.pruneReadDays)}
                            options={PRUNE_READ_OPTIONS.map((d) => ({ value: String(d), label: daysLabel(d) }))}
                            onchange={(v) => setFlat({ pruneReadDays: Number(v) })}
                          />
                        </div>
                      </div>
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Keep unread Articles</span>
                          <span class="desc">Older ones you never opened are removed.</span>
                        </div>
                        <div class="control">
                          <Select
                            label="Keep unread Articles for"
                            value={String(s.pruneUnreadDays)}
                            options={PRUNE_UNREAD_OPTIONS.map((d) => ({ value: String(d), label: daysLabel(d) }))}
                            onchange={(v) => setFlat({ pruneUnreadDays: Number(v) })}
                          />
                        </div>
                      </div>
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Image cache limit</span>
                          <span class="desc">
                            Using {fileSize(app.mediaSize)} now. Favicons and images of unread or starred
                            Articles are never removed.
                          </span>
                        </div>
                        <div class="control">
                          <Select
                            label="Image cache limit"
                            value={String(s.imageCacheCapBytes)}
                            options={IMAGE_CACHE_CAPS.map((c) => ({ value: String(c), label: fileSize(c) }))}
                            onchange={(v) => setFlat({ imageCacheCapBytes: Number(v) })}
                          />
                        </div>
                      </div>
                    </div>
                  {:else if item.value === 'library'}
                    <dl class="group facts">
                      <div class="row">
                        <dt class="label">Location</dt>
                        <dd class="path" title={app.library?.path}>{app.library?.path}</dd>
                      </div>
                      <div class="row">
                        <dt class="label">Size</dt>
                        <dd>{fileSize(app.librarySize)}</dd>
                      </div>
                      <div class="row">
                        <dt class="label">Format version</dt>
                        <dd>{app.library?.formatVersion}</dd>
                      </div>
                    </dl>
                  {:else}
                    <div class="group">
                      <div class="row">
                        <div class="row-text">
                          <span class="label">Start with OS</span>
                          <span class="desc">
                            {app.library?.portable
                              ? 'Not available in Portable Mode'
                              : 'Opens Feedrico in the background when you log in.'}
                          </span>
                        </div>
                        <Toggle
                          variant="switch"
                          pressed={app.autostart ?? false}
                          disabled={app.library?.portable ?? false}
                          onchange={(pressed) => void app.setAutostart(pressed)}
                          label="Start Feedrico with the OS"
                        >
                          Start with OS
                        </Toggle>
                      </div>
                    </div>
                  {/if}
                </div>
              {/snippet}
            </Tabs.Content>
          {/each}

          <div class="close">
            <IconButton icon="close" label="Close settings" title="Close (Esc)" onclick={() => (app.settingsOpen = false)} />
          </div>
        </div>
      {/snippet}
    </Tabs.Root>
  {/if}
</Dialog>

<style>
  /* The window: title and section list on a sunken rail, the pane on paper. */
  :global(div.dialog.settings-dialog) {
    display: grid;
    grid-template-columns: var(--settings-nav-width) minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    width: min(var(--settings-width), calc(100vw - 2rem));
    height: min(var(--settings-height), calc(100vh - 2rem));
    padding: 0;
    overflow: hidden;
    border-radius: var(--radius-overlay);
    background: linear-gradient(
      to right,
      var(--color-surface-sunken) var(--settings-nav-width),
      var(--color-surface-raised) var(--settings-nav-width)
    );
  }
  :global(div.settings-dialog h2.dialog-title) {
    grid-column: 1;
    grid-row: 1;
    margin: 0;
    padding: var(--space-inset-xl) var(--space-inset-lg) var(--space-stack-md);
  }
  .settings {
    display: contents;
  }
  .nav {
    grid-column: 1;
    grid-row: 2;
    display: flex;
    flex-direction: column;
    gap: var(--space-stack-2xs);
    padding: 0 var(--space-inset-sm) var(--space-inset-lg);
    border-inline-end: 1px solid var(--color-border-subtle);
  }
  .nav-item {
    height: var(--sidebar-row-height);
    padding-inline: var(--space-inset-sm);
    border: 0;
    border-radius: var(--radius-control);
    background: none;
    color: var(--color-text-muted);
    text-align: start;
    cursor: pointer;
    transition: background-color var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .nav-item:hover {
    background: var(--color-surface-hover);
    color: var(--color-text-default);
  }
  .nav-item[data-state='active'] {
    background: var(--color-surface-selected);
    color: var(--color-text-default);
    font-weight: var(--font-weight-medium);
    box-shadow: inset 3px 0 0 var(--color-border-focus);
  }
  /* Machine-local settings stand apart from the ones that travel with the Library. */
  .nav-item.apart {
    margin-block-start: auto;
  }
  .close {
    grid-column: 2;
    grid-row: 1;
    justify-self: end;
    padding: var(--space-inset-md) var(--space-inset-md) 0;
  }
  .pane {
    grid-column: 2;
    grid-row: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: var(--space-stack-lg);
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior-y: none;
    padding: var(--space-inset-xl) var(--space-inset-xl) var(--space-inset-2xl);
    outline: none;
  }
  .pane[hidden] {
    display: none;
  }
  .pane-head {
    display: grid;
    gap: var(--space-stack-xs);
    padding-inline-end: var(--space-inset-2xl);
  }
  h3 {
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-lg);
    font-weight: var(--font-weight-heading);
    line-height: var(--font-line-height-heading);
  }
  .where {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }

  /* Rows of settings in one rounded group, divided by hairlines. */
  .group {
    margin: 0;
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-card);
    background: var(--color-surface-base);
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--settings-control-width);
    align-items: center;
    gap: var(--space-inline-lg);
    min-height: 56px;
    padding: var(--space-inset-md) var(--space-inset-lg);
  }
  .row + .row {
    border-block-start: 1px solid var(--color-border-subtle);
  }
  .row-text {
    display: grid;
    gap: var(--space-stack-2xs);
    min-width: 0;
  }
  .label {
    font-weight: var(--font-weight-medium);
    color: var(--color-text-default);
  }
  .desc {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .desc.value {
    font-variant-numeric: tabular-nums;
  }
  .control {
    min-width: 0;
  }
  .row > :global(.segmented) {
    justify-self: stretch;
  }
  .row > :global(.switch) {
    justify-self: end;
  }

  /* Theme: three small windows, the choice you can see. */
  .block {
    display: grid;
    gap: var(--space-stack-sm);
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-inline-md);
  }
  .theme {
    position: relative;
    display: grid;
    gap: var(--space-stack-sm);
    justify-items: center;
    cursor: pointer;
  }
  .theme input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .theme-preview {
    display: flex;
    width: 100%;
    aspect-ratio: 16 / 10;
    overflow: hidden;
    border-radius: var(--radius-card);
    box-shadow:
      0 0 0 1px var(--color-border-subtle),
      var(--shadow-raised);
    transition: box-shadow var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .theme:hover .theme-preview {
    box-shadow:
      0 0 0 1px var(--color-border-default),
      var(--shadow-raised);
  }
  .theme.active .theme-preview {
    box-shadow:
      0 0 0 2px var(--color-border-focus),
      var(--shadow-raised);
  }
  .theme:has(input:focus-visible) .theme-preview {
    outline: var(--focus-ring-width) solid var(--color-border-focus);
    outline-offset: calc(var(--focus-ring-offset) + 2px);
  }
  .theme-name {
    color: var(--color-text-muted);
    font-size: var(--font-size-small);
  }
  .theme.active .theme-name {
    color: var(--color-text-default);
    font-weight: var(--font-weight-medium);
  }
  .split {
    flex: 1;
    display: flex;
    overflow: hidden;
  }
  .split :global(.mini) {
    min-width: 200%;
  }
  .split.dark {
    justify-content: flex-end;
  }
  .mini {
    flex: 1;
    display: flex;
    background: var(--color-surface-base);
  }
  .mini-sidebar {
    display: flex;
    flex-direction: column;
    gap: 6%;
    width: 30%;
    padding: 9% 6%;
    background: var(--color-surface-sunken);
  }
  .mini-line,
  .mini-title {
    display: block;
    height: 4px;
    border-radius: var(--radius-pill);
    background: var(--color-text-disabled);
  }
  .mini-line.short {
    width: 60%;
    background: var(--color-text-accent);
  }
  .mini-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 14%;
    padding: 12% 8%;
  }
  .mini-row {
    display: flex;
    align-items: center;
    gap: 6%;
  }
  .mini-dot {
    flex: none;
    width: 5px;
    height: 5px;
    border-radius: var(--radius-pill);
    background: var(--color-unread-indicator);
  }
  .mini-title {
    flex: 1;
    background: var(--color-text-default);
  }
  .mini-title.short {
    flex: 0.6;
  }
  .mini-row.read .mini-dot {
    background: transparent;
  }
  .mini-row.read .mini-title {
    background: var(--color-text-disabled);
  }

  /* Reading: a page that reflows as you drag. */
  .page {
    margin: 0;
    padding: var(--space-inset-xl) var(--space-inset-xl) var(--space-inset-2xl);
    overflow: hidden;
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-card);
    background: var(--color-surface-base);
    box-shadow: var(--shadow-raised);
  }
  .page-text {
    color: var(--color-text-default);
    transition: max-width var(--motion-duration-fast) var(--motion-easing-standard);
  }
  .page-text p + p {
    margin-block-start: 0.6em;
  }
  .page-source {
    color: var(--color-text-accent);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-small);
    font-weight: var(--font-weight-medium);
  }
  .page-title {
    font-size: 1.45em;
    font-weight: var(--font-weight-heading);
    line-height: var(--font-line-height-heading);
  }

  /* About this Library. */
  .facts dd {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-default);
    text-align: end;
    user-select: text;
  }
  .facts .row {
    grid-template-columns: auto minmax(0, 1fr);
  }

  @media (prefers-reduced-motion: reduce) {
    .page-text {
      transition: none;
    }
  }
</style>
