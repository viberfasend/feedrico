<script lang="ts">
  import Dialog from '../Dialog.svelte';

  let open = $state(false);
</script>

<button id="open-dialog" onclick={() => (open = true)}>Open</button>
<Dialog bind:open title="Confirm">
  <p>Dialog body</p>
</Dialog>
