/**
 * How reminders are written on screen. Presentation only: what counts as a
 * valid reminder is decided in Rust.
 */
import { WEEKDAYS, type Delivery, type Weekday } from '../api/reminders';

export const WEEKDAY_NAMES: Record<Weekday, string> = {
  mon: 'Mon',
  tue: 'Tue',
  wed: 'Wed',
  thu: 'Thu',
  fri: 'Fri',
  sat: 'Sat',
  sun: 'Sun',
};

/** One letter per day, for compact toggles. */
export const WEEKDAY_INITIALS: Record<Weekday, string> = {
  mon: 'M',
  tue: 'T',
  wed: 'W',
  thu: 'T',
  fri: 'F',
  sat: 'S',
  sun: 'S',
};

/** `18:05`: 24-hour, as macOS schedules it. */
export function formatTime(hour: number, minute: number): string {
  return `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`;
}

/** Parses an `<input type="time">` value (`HH:MM`). */
export function parseTime(value: string): { hour: number; minute: number } | null {
  const match = /^(\d{2}):(\d{2})$/.exec(value);
  if (!match) return null;
  return { hour: Number(match[1]), minute: Number(match[2]) };
}

/**
 * `Every day`, `Weekdays`, `Weekends`, a range such as `Mon–Thu`, or a list
 * such as `Mon, Wed, Fri`. Monday is the first day.
 */
export function formatWeekdays(days: readonly Weekday[]): string {
  const indices = [...new Set(days.map((day) => WEEKDAYS.indexOf(day)))]
    .filter((index) => index >= 0)
    .sort((a, b) => a - b);
  const key = indices.join('');
  if (key === '0123456') return 'Every day';
  if (key === '01234') return 'Weekdays';
  if (key === '56') return 'Weekends';

  const names = indices.map((index) => WEEKDAY_NAMES[WEEKDAYS[index]]);
  const consecutive = indices.every((index, i) => i === 0 || index === indices[i - 1] + 1);
  if (consecutive && indices.length >= 3) return `${names[0]}–${names[names.length - 1]}`;
  return names.join(', ');
}

/** One sentence on whether reminders will actually arrive. */
export function describeDelivery(delivery: Delivery): string {
  switch (delivery.status) {
    case 'off':
      return 'Off. Nothing is scheduled.';
    case 'scheduled':
      return 'On. macOS delivers them even when WHITE ROOM is closed, and keeps them silent while WHITE ROOM is the app in front.';
    case 'notAllowed':
      return 'On, but macOS is not allowing WHITE ROOM to show notifications. Allow them in System Settings › Notifications › WHITE ROOM.';
    case 'unavailable':
      return `Not available here: ${delivery.reason}`;
    case 'failed':
      return `macOS did not accept the reminders: ${delivery.reason}`;
  }
}
