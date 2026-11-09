<!--
  The windowed entry screen shown outside the workspace. It says plainly how
  to get back out before the user goes in.
-->
<script lang="ts">
  import type { AppInfo } from '../api/app';
  import type { WorkspaceSnapshot } from '../api/workspace';

  interface Props {
    /** `null` until the backend has answered. */
    snapshot: WorkspaceSnapshot | null;
    appInfo: Promise<AppInfo>;
    /** Set when loading the workspace state or entering it failed. */
    error: string | null;
    onenter: () => void;
  }

  let { snapshot, appInfo, error, onenter }: Props = $props();
</script>

<main
  class="flex h-full flex-col items-center justify-center gap-12 p-gutter motion-safe:animate-fade-in"
>
  <header class="flex flex-col items-center gap-3 text-center">
    <!-- Left padding balances the trailing letter-spacing so the wordmark is centred. -->
    <h1 class="pl-[0.4em] text-lead font-medium tracking-wordmark text-ink">WHITE ROOM</h1>
    <p class="text-caption text-ink-muted">A local-first learning and focus environment</p>
  </header>

  <button
    type="button"
    class="bg-ink px-8 py-3 text-micro font-medium tracking-label text-ink-inverse uppercase transition-colors hover:bg-grey-700 active:bg-grey-800"
    onclick={onenter}
  >
    Enter workspace
  </button>

  <dl class="grid max-w-md grid-cols-[auto_1fr] items-baseline gap-x-6 gap-y-3 text-caption">
    <dt class="text-micro tracking-label text-ink-faint uppercase">Emergency exit</dt>
    <dd data-emergency-exit>
      {#if snapshot === null}
        <span class="text-ink-faint">Checking…</span>
      {:else if snapshot.emergencyExit.available}
        <kbd class="font-mono text-ink">{snapshot.emergencyExit.label}</kbd>
        <span class="text-ink-muted">
          ({snapshot.emergencyExit.description}) leaves the workspace from any app.
        </span>
      {:else}
        <span class="text-ink">Unavailable.</span>
        <span class="text-ink-muted">
          {snapshot.emergencyExit.unavailableReason ?? 'The shortcut could not be registered.'}
          The Exit button and the system's fullscreen control still work.
        </span>
      {/if}
    </dd>

    <dt class="text-micro tracking-label text-ink-faint uppercase">Display</dt>
    <dd class="text-ink-muted">Opens fullscreen on the primary display.</dd>
  </dl>

  {#if error}
    <p class="max-w-md text-center text-caption text-ink" role="alert">{error}</p>
  {/if}

  <footer class="text-micro tracking-label text-ink-faint uppercase">
    {#await appInfo}
      Initializing
    {:then info}
      Version <span class="font-mono">{info.version}</span> · Schema
      <span class="font-mono">{info.schemaVersion}</span>
    {:catch err}
      <span role="alert">Backend unavailable: {String(err)}</span>
    {/await}
  </footer>
</main>
