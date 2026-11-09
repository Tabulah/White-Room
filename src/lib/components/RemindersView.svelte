<!--
  Study reminders: switch them on or off, see and remove them, add new ones.
  Presentation only; Reminders.svelte talks to Rust, which validates,
  stores, and schedules them.
-->
<script lang="ts">
  import { WEEKDAYS, type NewReminder, type ReminderState, type Weekday } from '../api/reminders';
  import {
    WEEKDAY_INITIALS,
    WEEKDAY_NAMES,
    describeDelivery,
    formatTime,
    formatWeekdays,
    parseTime,
  } from '../format/reminders';

  interface Props {
    /** `null` until the backend has answered. */
    reminders: ReminderState | null;
    busy: boolean;
    error: string | null;
    ontoggle: (enabled: boolean) => void;
    /** Resolves to whether the reminder was added. */
    onadd: (reminder: NewReminder) => Promise<boolean>;
    onremove: (id: number) => void;
    onback: () => void;
  }

  let { reminders, busy, error, ontoggle, onadd, onremove, onback }: Props = $props();

  // Form state for the next reminder.
  let time = $state('18:00');
  let days = $state<Weekday[]>(['mon', 'tue', 'wed', 'thu', 'fri']);
  let label = $state('');
  let formError = $state<string | null>(null);

  const enabled = $derived(reminders?.enabled ?? false);

  function toggleDay(day: Weekday) {
    days = days.includes(day) ? days.filter((d) => d !== day) : [...days, day];
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    formError = null;
    const parsed = parseTime(time);
    if (!parsed) {
      formError = 'Choose a time.';
      return;
    }
    const weekdays = WEEKDAYS.filter((day) => days.includes(day));
    if (await onadd({ ...parsed, weekdays, label: label || null })) label = '';
  }
</script>

<main class="flex h-full justify-center overflow-y-auto p-gutter motion-safe:animate-fade-in">
  <div class="flex w-full max-w-xl flex-col gap-10 py-12">
    <header class="flex items-baseline justify-between">
      <h1 class="text-micro tracking-label text-ink-faint uppercase">Study reminders</h1>
      <button
        type="button"
        class="text-caption text-ink-muted hover:text-ink"
        onclick={onback}
        data-back
      >
        Back
      </button>
    </header>

    <section class="flex flex-col gap-3">
      <button
        type="button"
        role="switch"
        aria-checked={enabled}
        class="group flex items-center gap-3 self-start text-body text-ink disabled:opacity-50"
        disabled={reminders === null || busy}
        onclick={() => ontoggle(!enabled)}
        data-toggle
      >
        <span
          class="flex h-5 w-9 items-center border border-ink px-0.5 transition-colors group-aria-checked:bg-ink"
        >
          <span
            class="h-3 w-3 bg-ink transition-transform motion-reduce:transition-none group-aria-checked:translate-x-4 group-aria-checked:bg-ink-inverse"
          ></span>
        </span>
        Remind me to study
      </button>
      <p class="text-caption text-ink-muted" data-delivery>
        {reminders ? describeDelivery(reminders.delivery) : 'Checking…'}
      </p>
    </section>

    <section class="flex flex-col gap-3" aria-labelledby="reminder-list">
      <h2 id="reminder-list" class="text-micro tracking-label text-ink-faint uppercase">
        Reminders
      </h2>
      {#if reminders && reminders.reminders.length > 0}
        <ul class="divide-y divide-line border-y border-line">
          {#each reminders.reminders as reminder (reminder.id)}
            {@const when = `${formatTime(reminder.hour, reminder.minute)}, ${formatWeekdays(reminder.weekdays)}`}
            <li class="flex items-baseline gap-6 py-3 text-caption" data-reminder>
              <span class="font-mono text-ink">{formatTime(reminder.hour, reminder.minute)}</span>
              <span class="w-28 text-ink-muted">{formatWeekdays(reminder.weekdays)}</span>
              <span class="min-w-0 flex-1 truncate text-ink">
                {reminder.label ?? reminders.defaultTitle}
              </span>
              <button
                type="button"
                class="text-ink-muted hover:text-ink disabled:opacity-50"
                aria-label={`Remove reminder ${when}`}
                disabled={busy}
                onclick={() => onremove(reminder.id)}
              >
                Remove
              </button>
            </li>
          {/each}
        </ul>
      {:else if reminders}
        <p class="text-caption text-ink-muted">No reminders yet.</p>
      {/if}
    </section>

    <form class="flex flex-col gap-5" aria-labelledby="add-reminder" onsubmit={submit}>
      <h2 id="add-reminder" class="text-micro tracking-label text-ink-faint uppercase">
        Add a reminder
      </h2>
      <div class="flex flex-wrap items-end gap-8">
        <label class="flex flex-col gap-2">
          <span class="text-micro tracking-label text-ink-faint uppercase">Time</span>
          <input
            type="time"
            class="h-9 border border-line-strong bg-surface-raised px-2 font-mono text-caption text-ink"
            bind:value={time}
            required
          />
        </label>
        <fieldset class="flex flex-col gap-2">
          <legend class="mb-2 text-micro tracking-label text-ink-faint uppercase">Days</legend>
          <div class="flex gap-1">
            {#each WEEKDAYS as day (day)}
              <button
                type="button"
                class="h-9 w-9 border border-line-strong text-caption text-ink-muted transition-colors aria-pressed:border-ink aria-pressed:bg-ink aria-pressed:text-ink-inverse"
                aria-pressed={days.includes(day)}
                aria-label={WEEKDAY_NAMES[day]}
                onclick={() => toggleDay(day)}
                data-day={day}
              >
                {WEEKDAY_INITIALS[day]}
              </button>
            {/each}
          </div>
        </fieldset>
      </div>
      <label class="flex flex-col gap-2">
        <span class="text-micro tracking-label text-ink-faint uppercase">Label (optional)</span>
        <input
          type="text"
          class="h-9 border border-line-strong bg-surface-raised px-2 text-caption text-ink placeholder:text-ink-faint"
          maxlength="60"
          placeholder={reminders?.defaultTitle ?? ''}
          bind:value={label}
        />
      </label>
      <div>
        <button
          type="submit"
          class="bg-ink px-6 py-2 text-micro font-medium tracking-label text-ink-inverse uppercase transition-colors hover:bg-grey-700 active:bg-grey-800 disabled:opacity-50"
          disabled={busy}
        >
          Add reminder
        </button>
      </div>
    </form>

    {#if formError ?? error}
      <p class="text-caption text-ink" role="alert">{formError ?? error}</p>
    {/if}

    <p class="text-caption text-ink-faint">
      Reminders say only that it is time to study. They never mention streaks or missed days, and
      you can also silence them in System Settings › Notifications.
    </p>
  </div>
</main>
