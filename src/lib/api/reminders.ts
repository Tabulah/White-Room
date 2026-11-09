/**
 * Typed bindings for study reminders.
 *
 * Rust validates, stores, and schedules reminders with macOS (see
 * `src-tauri/src/sessions/reminders.rs`). Every command returns the full,
 * current state, including whether macOS will actually deliver them.
 */
import { invoke } from '@tauri-apps/api/core';

export type Weekday = 'mon' | 'tue' | 'wed' | 'thu' | 'fri' | 'sat' | 'sun';

export const WEEKDAYS: readonly Weekday[] = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'];

/** Mirrors `sessions::reminders::Reminder`. Times are local. */
export interface Reminder {
  id: number;
  hour: number;
  minute: number;
  /** Monday first. */
  weekdays: Weekday[];
  /** Shown instead of the default title (`ReminderState.defaultTitle`). */
  label: string | null;
}

/** Mirrors `sessions::reminders::NewReminder`. Validated in Rust. */
export interface NewReminder {
  hour: number;
  minute: number;
  weekdays: Weekday[];
  label: string | null;
}

/** Mirrors `sessions::reminders::Delivery`: what macOS has confirmed. */
export type Delivery =
  | { status: 'off' }
  | { status: 'scheduled' }
  | { status: 'notAllowed' }
  | { status: 'unavailable'; reason: string }
  | { status: 'failed'; reason: string };

/** Mirrors `sessions::reminders::ReminderState`. */
export interface ReminderState {
  /** The user's choice, kept even when delivery is not possible. */
  enabled: boolean;
  /** Earliest time of day first. */
  reminders: Reminder[];
  delivery: Delivery;
  /** Title of reminders without a label. */
  defaultTitle: string;
}

export function getReminders(): Promise<ReminderState> {
  return invoke<ReminderState>('get_reminders');
}

/** Turning reminders on may show macOS's notification permission prompt. */
export function setRemindersEnabled(enabled: boolean): Promise<ReminderState> {
  return invoke<ReminderState>('set_reminders_enabled', { enabled });
}

export function addReminder(reminder: NewReminder): Promise<ReminderState> {
  return invoke<ReminderState>('add_reminder', { reminder });
}

export function removeReminder(id: number): Promise<ReminderState> {
  return invoke<ReminderState>('remove_reminder', { id });
}
