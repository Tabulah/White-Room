<!--
  The study reminders screen: loads the state from Rust and sends changes
  back. Every command returns the full state, which replaces the local copy.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    addReminder,
    getReminders,
    removeReminder,
    setRemindersEnabled,
    type ReminderState,
  } from '../api/reminders';
  import RemindersView from './RemindersView.svelte';

  interface Props {
    onback: () => void;
  }

  let { onback }: Props = $props();

  let reminders = $state<ReminderState | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  /** Runs one command; resolves to whether it succeeded. */
  async function run(command: () => Promise<ReminderState>): Promise<boolean> {
    busy = true;
    error = null;
    try {
      reminders = await command();
      return true;
    } catch (err) {
      error = String(err);
      return false;
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void run(getReminders);
  });
</script>

<RemindersView
  {reminders}
  {busy}
  {error}
  ontoggle={(enabled) => void run(() => setRemindersEnabled(enabled))}
  onadd={(reminder) => run(() => addReminder(reminder))}
  onremove={(id) => void run(() => removeReminder(id))}
  {onback}
/>
