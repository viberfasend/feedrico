<script lang="ts">
  import ContextMenu, { type MenuItem } from '../ContextMenu.svelte';

  const items: MenuItem[] = [
    { label: 'Rename', onclick: () => {} },
    { label: 'Delete', onclick: () => {}, danger: true, separatorBefore: true },
  ];
</script>

<ContextMenu {items}>
  <button id="ctx-target" type="button">Right-click or press the menu key</button>
</ContextMenu>
