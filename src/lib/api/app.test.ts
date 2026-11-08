import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import { getAppInfo } from './app';

describe('getAppInfo', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('invokes the get_app_info command with no arguments', async () => {
    const info = { name: 'WHITE ROOM', version: '0.1.0', schemaVersion: 1 };
    invoke.mockResolvedValue(info);

    await expect(getAppInfo()).resolves.toEqual(info);
    expect(invoke).toHaveBeenCalledWith('get_app_info');
  });

  it('propagates backend errors', async () => {
    invoke.mockRejectedValue('database unavailable');

    await expect(getAppInfo()).rejects.toBe('database unavailable');
  });
});
