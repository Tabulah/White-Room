import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, listen } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

import {
  WORKSPACE_CHANGED_EVENT,
  enterWorkspace,
  exitWorkspace,
  getWorkspaceState,
  newerSnapshot,
  onWorkspaceChanged,
  type WorkspaceSnapshot,
} from './workspace';

function snapshot(overrides: Partial<WorkspaceSnapshot> = {}): WorkspaceSnapshot {
  return {
    revision: 0,
    status: 'inactive',
    display: null,
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

describe('workspace commands', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it.each([
    ['get_workspace_state', getWorkspaceState],
    ['enter_workspace', enterWorkspace],
    ['exit_workspace', exitWorkspace],
  ])('invokes %s with no arguments and returns the snapshot', async (command, binding) => {
    const state = snapshot({ revision: 3 });
    invoke.mockResolvedValue(state);

    await expect(binding()).resolves.toEqual(state);
    expect(invoke).toHaveBeenCalledWith(command);
  });

  it('propagates backend errors', async () => {
    invoke.mockRejectedValue('workspace error: the main window is not available');

    await expect(exitWorkspace()).rejects.toBe('workspace error: the main window is not available');
  });
});

describe('onWorkspaceChanged', () => {
  beforeEach(() => {
    listen.mockReset();
  });

  it('listens to the change event and passes the payload through', async () => {
    const unlisten = vi.fn();
    listen.mockResolvedValue(unlisten);
    const handler = vi.fn();

    await expect(onWorkspaceChanged(handler)).resolves.toBe(unlisten);
    expect(listen).toHaveBeenCalledWith(WORKSPACE_CHANGED_EVENT, expect.any(Function));

    const payload = snapshot({ revision: 7, status: 'active' });
    const callback = listen.mock.calls[0][1] as (event: { payload: WorkspaceSnapshot }) => void;
    callback({ payload });
    expect(handler).toHaveBeenCalledWith(payload);
  });

  it('uses the event name emitted by the backend', () => {
    // Must match `workspace::CHANGED_EVENT` in Rust.
    expect(WORKSPACE_CHANGED_EVENT).toBe('workspace://changed');
  });
});

describe('newerSnapshot', () => {
  it('accepts the first snapshot', () => {
    const first = snapshot();
    expect(newerSnapshot(null, first)).toBe(first);
  });

  it('accepts a newer snapshot', () => {
    const current = snapshot({ revision: 1, status: 'active' });
    const incoming = snapshot({ revision: 2, lastExit: 'emergencyShortcut' });
    expect(newerSnapshot(current, incoming)).toBe(incoming);
  });

  it('ignores a stale snapshot that arrives late', () => {
    const current = snapshot({ revision: 2, lastExit: 'emergencyShortcut' });
    const stale = snapshot({ revision: 1, status: 'active' });
    expect(newerSnapshot(current, stale)).toBe(current);
  });

  it('accepts an equal revision (same state, e.g. a repeated request)', () => {
    const current = snapshot({ revision: 4 });
    const same = snapshot({ revision: 4 });
    expect(newerSnapshot(current, same)).toBe(same);
  });
});
