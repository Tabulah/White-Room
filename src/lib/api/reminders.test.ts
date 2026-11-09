import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import {
  addReminder,
  getReminders,
  removeReminder,
  setRemindersEnabled,
  type ReminderState,
} from './reminders';

const state: ReminderState = {
  enabled: true,
  reminders: [{ id: 1, hour: 18, minute: 0, weekdays: ['mon', 'fri'], label: null }],
  delivery: { status: 'scheduled' },
  defaultTitle: 'Time to study',
};

describe('reminder commands', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(state);
  });

  it('reads the state', async () => {
    await expect(getReminders()).resolves.toEqual(state);
    expect(invoke).toHaveBeenCalledWith('get_reminders');
  });

  it('switches reminders on and off', async () => {
    await setRemindersEnabled(true);
    expect(invoke).toHaveBeenCalledWith('set_reminders_enabled', { enabled: true });
  });

  it('sends a new reminder as one argument', async () => {
    const reminder = { hour: 7, minute: 30, weekdays: ['sat' as const], label: 'Chemistry' };
    await addReminder(reminder);
    expect(invoke).toHaveBeenCalledWith('add_reminder', { reminder });
  });

  it('removes by id', async () => {
    await removeReminder(1);
    expect(invoke).toHaveBeenCalledWith('remove_reminder', { id: 1 });
  });

  it('passes validation messages from Rust through unchanged', async () => {
    invoke.mockRejectedValue('Choose at least one day.');
    await expect(addReminder({ hour: 7, minute: 0, weekdays: [], label: null })).rejects.toBe(
      'Choose at least one day.',
    );
  });
});
