<!--
  The fullscreen workspace: a placeholder dashboard and a status bar holding
  the exit control.

  The exit control sits bottom-right on purpose. In macOS fullscreen, moving
  the pointer to the top edge slides the menu bar and title bar down over the
  window, which would cover a control at the top; nothing covers the bottom
  corners. The status bar is outside the scrolling area and above any future
  overlay (z-50), so the control stays visible whatever the dashboard shows.
-->
<script lang="ts">
  import type { WorkspaceSnapshot } from '../api/workspace';
  import Clock from './Clock.svelte';
  import ExitControl from './ExitControl.svelte';

  interface Props {
    snapshot: WorkspaceSnapshot;
    /** Set when the last exit attempt failed. */
    exitError: string | null;
    onexit: () => void;
  }

  let { snapshot, exitError, onexit }: Props = $props();

  // Placeholders only: these areas are built in later roadmap phases.
  const areas = [
    { name: 'Session', purpose: 'Focus sessions' },
    { name: 'Curriculum', purpose: 'Courses and lessons' },
    { name: 'Progress', purpose: 'XP, levels, and mastery' },
  ];

  // Display names are not shown: macOS reports placeholders such as
  // "Monitor #41039" rather than real names. They are kept in the log.
  const displayLabel = $derived.by(() => {
    const display = snapshot.display;
    if (display === null) return 'Display unknown';
    return display.primary ? 'Primary display' : 'Current display';
  });
</script>

<div class="flex h-full flex-col bg-surface motion-safe:animate-fade-in">
  <main
    class="flex min-h-0 flex-1 flex-col items-center justify-center gap-16 overflow-auto p-gutter"
  >
    <Clock />

    <section class="w-full max-w-3xl" aria-labelledby="dashboard-heading">
      <h2 id="dashboard-heading" class="sr-only">Dashboard</h2>
      <ul class="grid grid-cols-3 gap-px border border-line bg-line">
        {#each areas as area (area.name)}
          <li class="flex flex-col gap-2 bg-surface-raised p-panel">
            <span class="text-micro tracking-label text-ink-faint uppercase">{area.name}</span>
            <span class="text-body text-ink-muted">{area.purpose}</span>
            <span class="text-caption text-ink-faint">Not yet available</span>
          </li>
        {/each}
      </ul>
    </section>
  </main>

  <footer
    class="relative z-50 flex h-bar shrink-0 items-center justify-between gap-6 border-t border-line bg-surface px-gutter"
  >
    <p class="truncate text-micro tracking-label text-ink-faint uppercase">
      White Room · {displayLabel}
      {#if !snapshot.emergencyExit.available}
        · Emergency shortcut unavailable
      {/if}
    </p>

    <div class="flex shrink-0 items-center gap-4">
      {#if exitError}
        <p class="max-w-md text-caption text-ink" role="alert">
          Could not leave the workspace: {exitError}. Use the system's fullscreen control or quit
          WHITE ROOM.
        </p>
      {/if}
      <ExitControl shortcut={snapshot.emergencyExit} {onexit} />
    </div>
  </footer>
</div>
