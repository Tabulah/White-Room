<script lang="ts">
  import { onMount } from 'svelte';
  import { getAppInfo } from './lib/api/app';
  import {
    enterWorkspace,
    exitWorkspace,
    getWorkspaceState,
    newerSnapshot,
    onWorkspaceChanged,
    type WorkspaceSnapshot,
  } from './lib/api/workspace';
  import EntryView from './lib/components/EntryView.svelte';
  import Reminders from './lib/components/Reminders.svelte';
  import WorkspaceView from './lib/components/WorkspaceView.svelte';

  const appInfo = getAppInfo();

  // Rust owns the workspace state; this is only the latest copy of it.
  let snapshot = $state<WorkspaceSnapshot | null>(null);
  let entryError = $state<string | null>(null);
  let exitError = $state<string | null>(null);
  /** Which screen to show outside the workspace (client-side UI state). */
  let screen = $state<'entry' | 'reminders'>('entry');

  function apply(next: WorkspaceSnapshot) {
    snapshot = newerSnapshot(snapshot, next);
    if (next.status === 'active') entryError = null;
    if (next.status === 'inactive') exitError = null;
  }

  onMount(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    // Subscribe before reading so that no change can fall between the two;
    // revisions sort out the order in which they arrive.
    onWorkspaceChanged(apply)
      .then((stop) => (disposed ? stop() : (unlisten = stop)))
      .catch((err) => console.error('workspace events unavailable', err));
    getWorkspaceState()
      .then(apply)
      .catch((err) => (entryError = `Workspace unavailable: ${String(err)}`));

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  async function enter() {
    entryError = null;
    try {
      apply(await enterWorkspace());
    } catch (err) {
      entryError = `Could not enter the workspace: ${String(err)}`;
    }
  }

  async function exit() {
    exitError = null;
    try {
      apply(await exitWorkspace());
    } catch (err) {
      exitError = String(err);
    }
  }
</script>

{#if snapshot?.status === 'active'}
  <WorkspaceView {snapshot} {exitError} onexit={exit} />
{:else if screen === 'reminders'}
  <Reminders onback={() => (screen = 'entry')} />
{:else}
  <EntryView
    {snapshot}
    {appInfo}
    error={entryError}
    onenter={enter}
    onreminders={() => (screen = 'reminders')}
  />
{/if}
