<!--
  The workspace exit control.

  Never disabled and never hidden: it is rendered unconditionally by
  WorkspaceView, outside any loading or error state. Clicking it asks Rust to
  leave the workspace; Rust takes the window out of fullscreen even if its own
  state disagrees (see `Workspace::exit`).
-->
<script lang="ts">
  import type { EmergencyExit } from '../api/workspace';

  interface Props {
    shortcut: EmergencyExit;
    onexit: () => void;
  }

  let { shortcut, onexit }: Props = $props();

  const accessibleName = $derived(
    shortcut.available ? `Exit workspace (${shortcut.description})` : 'Exit workspace',
  );
</script>

<button
  type="button"
  class="flex h-9 items-center gap-3 border border-ink bg-surface-raised px-4 text-ink transition-colors hover:bg-ink hover:text-ink-inverse active:bg-grey-800"
  aria-label={accessibleName}
  title={accessibleName}
  data-exit-control
  onclick={onexit}
>
  <span class="text-micro font-medium tracking-label uppercase">Exit</span>
  {#if shortcut.available}
    <kbd class="font-mono text-caption" aria-hidden="true">{shortcut.label}</kbd>
  {/if}
</button>
