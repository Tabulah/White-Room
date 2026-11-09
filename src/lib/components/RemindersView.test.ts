import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { ReminderState } from '../api/reminders';
import RemindersView from './RemindersView.svelte';

function state(overrides: Partial<ReminderState> = {}): ReminderState {
  return {
    enabled: true,
    reminders: [
      { id: 1, hour: 9, minute: 30, weekdays: ['sat', 'sun'], label: 'Chemistry' },
      { id: 2, hour: 18, minute: 0, weekdays: ['mon', 'tue', 'wed', 'thu', 'fri'], label: null },
    ],
    delivery: { status: 'scheduled' },
    defaultTitle: 'Time to study',
    ...overrides,
  };
}

function html(reminders: ReminderState | null, extra: { busy?: boolean; error?: string } = {}) {
  return render(RemindersView, {
    props: {
      reminders,
      busy: extra.busy ?? false,
      error: extra.error ?? null,
      ontoggle: () => {},
      onadd: () => Promise.resolve(true),
      onremove: () => {},
      onback: () => {},
    },
  }).body;
}

function text(fragment: string): string {
  return fragment
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function rows(body: string): string[] {
  return [...body.matchAll(/<li[^>]*data-reminder[^>]*>([\s\S]*?)<\/li>/g)].map((m) => text(m[1]));
}

describe('RemindersView', () => {
  it('lists reminders with their time, days, and title', () => {
    expect(rows(html(state()))).toEqual([
      '09:30 Weekends Chemistry Remove',
      '18:00 Weekdays Time to study Remove',
    ]);
  });

  it('names each remove button after its reminder', () => {
    expect(html(state())).toContain('aria-label="Remove reminder 18:00, Weekdays"');
  });

  it('shows the switch state from Rust', () => {
    expect(html(state())).toMatch(/role="switch"[^>]*aria-checked="true"/);
    expect(html(state({ enabled: false, delivery: { status: 'off' } }))).toMatch(
      /role="switch"[^>]*aria-checked="false"/,
    );
  });

  it('says whether reminders will actually arrive', () => {
    const delivery = (body: string) =>
      text(/<p[^>]*data-delivery[^>]*>([\s\S]*?)<\/p>/.exec(body)?.[1] ?? '');
    expect(delivery(html(state()))).toMatch(/^On\. macOS delivers them/);
    expect(delivery(html(state({ delivery: { status: 'notAllowed' } })))).toContain(
      'System Settings › Notifications',
    );
    expect(delivery(html(null))).toBe('Checking…');
  });

  it('disables the switch until the state is known', () => {
    expect(html(null)).toMatch(/<button[^>]*role="switch"[^>]*disabled/);
  });

  it('offers an empty list honestly', () => {
    expect(html(state({ reminders: [] }))).toContain('No reminders yet.');
  });

  it('starts the form on weekday evenings', () => {
    const body = html(state());
    const pressed = [...body.matchAll(/data-day="(\w+)"/g)].map((m) => m[1]);
    expect(pressed).toEqual(['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun']);
    expect(body.match(/aria-pressed="true"/g)).toHaveLength(5);
    expect(body).toMatch(/type="time"[^>]*value="18:00"/);
  });

  it('shows errors from Rust as alerts', () => {
    expect(html(state(), { error: 'Choose at least one day.' })).toMatch(
      /role="alert"[^>]*>Choose at least one day\./,
    );
  });
});
