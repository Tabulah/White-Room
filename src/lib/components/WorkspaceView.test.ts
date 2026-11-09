import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { WorkspaceSnapshot } from '../api/workspace';
import WorkspaceView from './WorkspaceView.svelte';

function snapshot(overrides: Partial<WorkspaceSnapshot> = {}): WorkspaceSnapshot {
  return {
    revision: 1,
    status: 'active',
    display: { name: 'Built-in Display', primary: true },
    lastExit: null,
    emergencyExit: {
      label: '⇧⌘⎋',
      description: 'Shift-Command-Escape',
      available: true,
      unavailableReason: null,
    },
    ...overrides,
  };
}

function html(props: { snapshot: WorkspaceSnapshot; exitError?: string | null }) {
  return render(WorkspaceView, {
    props: { exitError: null, onexit: () => {}, ...props },
  }).body;
}

/** The exit control's opening tag. */
function exitControl(body: string): string | undefined {
  return /<button[^>]*data-exit-control[^>]*>/.exec(body)?.[0];
}

describe('WorkspaceView', () => {
  it.each([
    ['shortcut available', snapshot()],
    [
      'shortcut unavailable',
      snapshot({
        emergencyExit: { ...snapshot().emergencyExit, available: false, unavailableReason: 'x' },
      }),
    ],
    ['display unknown', snapshot({ display: null })],
  ])('always renders an enabled exit control (%s)', (_, state) => {
    const control = exitControl(html({ snapshot: state }));
    expect(control).toBeDefined();
    expect(control).not.toMatch(/\bdisabled\b/);
    expect(control).not.toMatch(/\bhidden\b/);
  });

  it('keeps the exit control when an exit attempt failed', () => {
    const body = html({ snapshot: snapshot(), exitError: 'window refused' });
    expect(exitControl(body)).toBeDefined();
    expect(body).toMatch(/role="alert"[^>]*>[^<]*window refused/);
  });

  it('names the shortcut on the exit control when it is available', () => {
    const body = html({ snapshot: snapshot() });
    expect(exitControl(body)).toMatch(/aria-label="Exit workspace \(Shift-Command-Escape\)"/);
    expect(body).toContain('⇧⌘⎋');
  });

  it('does not advertise a shortcut that is not registered', () => {
    const state = snapshot({
      emergencyExit: { ...snapshot().emergencyExit, available: false, unavailableReason: 'x' },
    });
    const body = html({ snapshot: state });
    expect(exitControl(body)).toMatch(/aria-label="Exit workspace"/);
    expect(body).not.toContain('⇧⌘⎋');
    expect(body).toContain('Emergency shortcut unavailable');
  });

  it('shows which display the workspace is on', () => {
    expect(html({ snapshot: snapshot() })).toContain('Primary display');
    expect(html({ snapshot: snapshot({ display: { name: null, primary: false } }) })).toContain(
      'Current display',
    );
    expect(html({ snapshot: snapshot({ display: null }) })).toContain('Display unknown');
  });

  it('does not show OS display names, which are placeholders on macOS', () => {
    const state = snapshot({ display: { name: 'Monitor #41039', primary: true } });
    expect(html({ snapshot: state })).not.toContain('Monitor #41039');
  });

  it('marks dashboard areas as not yet available', () => {
    const body = html({ snapshot: snapshot() });
    expect(body.match(/Not yet available/g)).toHaveLength(3);
  });
});
