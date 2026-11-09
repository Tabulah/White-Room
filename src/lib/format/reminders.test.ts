import { describe, expect, it } from 'vitest';
import { describeDelivery, formatTime, formatWeekdays, parseTime } from './reminders';

describe('formatTime', () => {
  it('pads to 24-hour HH:MM', () => {
    expect(formatTime(7, 5)).toBe('07:05');
    expect(formatTime(18, 0)).toBe('18:00');
    expect(formatTime(0, 0)).toBe('00:00');
  });
});

describe('parseTime', () => {
  it('reads time input values', () => {
    expect(parseTime('18:30')).toEqual({ hour: 18, minute: 30 });
    expect(parseTime('')).toBeNull();
    expect(parseTime('6pm')).toBeNull();
  });
});

describe('formatWeekdays', () => {
  it.each([
    [['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'], 'Every day'],
    [['mon', 'tue', 'wed', 'thu', 'fri'], 'Weekdays'],
    [['sun', 'sat'], 'Weekends'],
    [['mon', 'tue', 'wed', 'thu'], 'Mon–Thu'],
    [['mon', 'wed', 'fri'], 'Mon, Wed, Fri'],
    [['tue', 'wed'], 'Tue, Wed'],
    [['sat'], 'Sat'],
    [['fri', 'mon', 'mon'], 'Mon, Fri'],
  ] as const)('%j → %s', (days, text) => {
    expect(formatWeekdays(days)).toBe(text);
  });
});

describe('describeDelivery', () => {
  it('never claims reminders arrive unless macOS accepted them', () => {
    expect(describeDelivery({ status: 'scheduled' })).toMatch(/^On\. macOS delivers them/);
    for (const delivery of [
      { status: 'off' },
      { status: 'notAllowed' },
      { status: 'unavailable', reason: 'x' },
      { status: 'failed', reason: 'x' },
    ] as const) {
      expect(describeDelivery(delivery)).not.toMatch(/delivers them/);
    }
  });

  it('points to System Settings when notifications are not allowed', () => {
    expect(describeDelivery({ status: 'notAllowed' })).toContain('System Settings › Notifications');
  });

  it('passes reasons through', () => {
    expect(describeDelivery({ status: 'unavailable', reason: 'Only on macOS.' })).toBe(
      'Not available here: Only on macOS.',
    );
  });
});
