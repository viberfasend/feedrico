<script lang="ts">
  import Menu, { type MenuItem } from '../Menu.svelte';

  let open = $state(false);
  const items: MenuItem[] = [
    { label: 'First', onclick: () => {} },
    { label: 'Second', onclick: () => {} },
  ];
</script>

<Menu {items} label="More actions" bind:open>
  <span>Open menu</span>
</Menu>
