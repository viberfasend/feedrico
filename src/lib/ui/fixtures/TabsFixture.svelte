<script lang="ts">
  import Tabs from '../Tabs.svelte';

  let value = $state('one');
</script>

<Tabs bind:value tabs={[{ value: 'one', label: 'One' }, { value: 'two', label: 'Two' }]}>
  {#snippet children(tab)}
    <p>Panel {tab}</p>
  {/snippet}
</Tabs>
