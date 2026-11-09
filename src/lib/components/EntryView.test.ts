import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { AppInfo } from '../api/app';
import type { WorkspaceSnapshot } from '../api/workspace';
import EntryView from './EntryView.svelte';

const available: WorkspaceSnapshot = {
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
};

function html(snapshot: WorkspaceSnapshot | null, error: string | null = null) {
  // A pending promise: the version footer is not under test here.
  const appInfo = new Promise<AppInfo>(() => {});
  return render(EntryView, { props: { snapshot, appInfo, error, onenter: () => {} } }).body;
}

/** Text content of the emergency-exit description, whitespace-collapsed. */
function emergencyExitText(body: string): string {
  const dd = /<dd[^>]*data-emergency-exit[^>]*>([\s\S]*?)<\/dd>/.exec(body)?.[1] ?? '';
  return dd
    .replace(/<[^>]+>/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

describe('EntryView', () => {
  it('offers to enter the workspace', () => {
    expect(html(available)).toMatch(/<button[^>]*>\s*Enter workspace\s*<\/button>/);
  });

  it('explains the emergency shortcut before the user goes in', () => {
    expect(emergencyExitText(html(available))).toBe(
      '⇧⌘⎋ (Shift-Command-Escape) leaves the workspace from any app.',
    );
  });

  it('says so honestly when the shortcut is unavailable', () => {
    const unavailable: WorkspaceSnapshot = {
      ...available,
      emergencyExit: {
        ...available.emergencyExit,
        available: false,
        unavailableReason: 'The system did not accept Shift-Command-Escape: taken',
      },
    };
    const text = emergencyExitText(html(unavailable));
    expect(text).toMatch(/^Unavailable\. The system did not accept Shift-Command-Escape: taken/);
    expect(text).toContain('The Exit button and the system');
    expect(text).not.toContain('⇧⌘⎋');
  });

  it('does not claim a shortcut before the backend has answered', () => {
    expect(emergencyExitText(html(null))).toBe('Checking…');
  });

  it('shows errors as alerts', () => {
    expect(html(available, 'Could not enter the workspace: boom')).toMatch(
      /role="alert"[^>]*>Could not enter the workspace: boom</,
    );
  });
});
