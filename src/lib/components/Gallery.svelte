<script lang="ts">
  // Dev-only component gallery: `npm run tauri dev`, then open #gallery. Not part of the
  // production bundle (imported dynamically behind import.meta.env.DEV in App.svelte).
  import { applyDensity, applyTheme, type Density, type Theme } from '../tokens';
  import Button from '../ui/Button.svelte';
  import ContextMenu, { type MenuItem } from '../ui/ContextMenu.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import Input from '../ui/Input.svelte';
  import Menu from '../ui/Menu.svelte';
  import Tabs from '../ui/Tabs.svelte';
  import Toast from '../ui/Toast.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Tooltip from '../ui/Tooltip.svelte';

  let theme = $state<Theme>('system');
  let density = $state<Density>('comfortable');
  $effect(() => applyTheme(theme));
  $effect(() => applyDensity(density));

  let input = $state('');
  let pressed = $state(false);
  let dialogOpen = $state(false);
  let tab = $state('one');
  const menuItems: MenuItem[] = [
    { label: 'Rename', onclick: () => {} },
    { label: 'Move to…', onclick: () => {} },
    { label: 'Unsubscribe', onclick: () => {}, danger: true, separatorBefore: true },
  ];
</script>

<main class="gallery">
  <header>
    <h1>Component gallery</h1>
    <div class="switchers">
      <label>Theme
        <select bind:value={theme}>
          <option value="system">system</option>
          <option value="light">light</option>
          <option value="dark">dark</option>
        </select>
      </label>
      <label>Density
        <select bind:value={density}>
          <option value="comfortable">comfortable</option>
          <option value="compact">compact</option>
        </select>
      </label>
    </div>
  </header>

  <section>
    <h2>Button</h2>
    <div class="row">
      <Button>Default</Button>
      <Button variant="primary">Primary</Button>
      <Button variant="danger">Danger</Button>
      <Button variant="ghost">Ghost</Button>
      <Button disabled>Disabled</Button>
    </div>
  </section>

  <section>
    <h2>IconButton</h2>
    <div class="row">
      <IconButton label="Add Feed" icon="plus" onclick={() => {}} />
      <IconButton label="Check for updates" icon="refresh" title="Check GitHub Releases for a newer version" onclick={() => {}} />
      <IconButton label="Starred" icon="star" onclick={() => {}} />
      <IconButton label="Disabled" icon="close" disabled onclick={() => {}} />
    </div>
  </section>

  <section>
    <h2>Input</h2>
    <div class="row">
      <Input bind:value={input} placeholder="Feed or website address" id="gallery-url" />
      <Input value="Filled" disabled id="gallery-disabled" />
    </div>
    <p class="hint">Value: {input || '—'}</p>
  </section>

  <section>
    <h2>Menu</h2>
    <div class="row">
      <Menu items={menuItems}>
        <span>Open menu</span>
      </Menu>
      <Menu items={menuItems.map((i) => ({ ...i, disabled: true }))}>
        <span>All disabled</span>
      </Menu>
    </div>
  </section>

  <section>
    <h2>ContextMenu</h2>
    <div class="row">
      <ContextMenu items={menuItems}>
        <button type="button" class="demo-target">Right-click or press the menu key</button>
      </ContextMenu>
    </div>
  </section>

  <section>
    <h2>Dialog</h2>
    <div class="row">
      <Button onclick={() => (dialogOpen = true)}>Open dialog</Button>
      <Dialog bind:open={dialogOpen} title="Example dialog">
        <p class="hint">Escape, scrim click and the button below all close it; focus returns to the trigger.</p>
        <div class="row">
          <Button variant="ghost" onclick={() => (dialogOpen = false)}>Cancel</Button>
          <Button variant="primary" onclick={() => (dialogOpen = false)}>Confirm</Button>
        </div>
      </Dialog>
    </div>
  </section>

  <section>
    <h2>Tooltip</h2>
    <div class="row">
      <Tooltip text="A tooltip on a button">
        <span>Hover me</span>
      </Tooltip>
    </div>
  </section>

  <section>
    <h2>Toggle</h2>
    <div class="row">
      <Toggle bind:pressed label="Compact density" />
      <Toggle pressed={true} label="Pressed (static)" disabled />
    </div>
    <p class="hint">aria-pressed: {pressed}</p>
  </section>

  <section>
    <h2>Tabs</h2>
    <Tabs bind:value={tab} tabs={[{ value: 'one', label: 'One' }, { value: 'two', label: 'Two' }, { value: 'three', label: 'Three' }]}>
      {#snippet children(value)}
        <p class="hint">Panel {value}</p>
      {/snippet}
    </Tabs>
  </section>

  <section>
    <h2>Toast</h2>
    <div class="row toast-demo">
      <Toast message="You're up to date (0.1.0)" />
      <Toast message="Feedrico 0.2.0 is available" action={{ label: 'Open', run: () => {} }} />
    </div>
  </section>
</main>

<style>
  .gallery {
    display: grid;
    gap: var(--space-stack-lg);
    max-width: 60rem;
    margin: 0 auto;
    padding: var(--space-inset-xl);
    overflow-y: auto;
    height: 100%;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h1 {
    font-family: var(--font-family-reading);
    font-size: var(--font-size-title-lg);
  }
  h2 {
    margin-block-end: var(--space-stack-sm);
    font-size: var(--font-size-title-sm);
    color: var(--color-text-muted);
  }
  .switchers {
    display: flex;
    gap: var(--space-inline-md);
  }
  .switchers label {
    display: grid;
    gap: var(--space-stack-2xs);
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  select {
    height: var(--control-height);
    padding-inline: var(--space-inset-sm);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-control);
    background: var(--color-surface-base);
    color: var(--color-text-default);
    font: inherit;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-inline-md);
    flex-wrap: wrap;
  }
  .hint {
    color: var(--color-text-muted);
    font-size: var(--font-size-caption);
  }
  .demo-target {
    height: var(--control-height);
    padding-inline: var(--space-inset-md);
    border: 1px dashed var(--color-border-default);
    border-radius: var(--radius-control);
    background: none;
    color: var(--color-text-default);
    cursor: context-menu;
  }
  .toast-demo {
    position: static;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-stack-sm);
  }
</style>
