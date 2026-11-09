/**
 * Typed bindings for the fullscreen workspace.
 *
 * Rust owns the workspace state. These bindings mirror the commands in
 * `src-tauri/src/commands.rs` and the `workspace://changed` event emitted by
 * `src-tauri/src/workspace.rs`.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type WorkspaceStatus = 'inactive' | 'active';

/** How the workspace was last left. */
export type ExitReason = 'exitControl' | 'emergencyShortcut' | 'system';

/** Mirrors `workspace::EmergencyExitInfo`. */
export interface EmergencyExit {
  /** Compact label, e.g. `⇧⌘⎋`. */
  label: string;
  /** Spelled out, e.g. `Shift-Command-Escape`. */
  description: string;
  /** Only `true` if the operating system accepted the shortcut. */
  available: boolean;
  unavailableReason: string | null;
}

/** Mirrors `workspace::WorkspaceDisplay`. */
export interface WorkspaceDisplay {
  name: string | null;
  /** `false` if the primary display could not be determined. */
  primary: boolean;
}

/** Mirrors `workspace::WorkspaceSnapshot`. */
export interface WorkspaceSnapshot {
  /** Increases with every change. */
  revision: number;
  status: WorkspaceStatus;
  display: WorkspaceDisplay | null;
  lastExit: ExitReason | null;
  emergencyExit: EmergencyExit;
}

export const WORKSPACE_CHANGED_EVENT = 'workspace://changed';

export function getWorkspaceState(): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>('get_workspace_state');
}

export function enterWorkspace(): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>('enter_workspace');
}

export function exitWorkspace(): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>('exit_workspace');
}

/**
 * Calls `handler` whenever the workspace changes, including changes the
 * frontend did not cause (the emergency shortcut, the system fullscreen
 * control). Resolves to a function that stops listening.
 */
export function onWorkspaceChanged(
  handler: (snapshot: WorkspaceSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<WorkspaceSnapshot>(WORKSPACE_CHANGED_EVENT, (event) => handler(event.payload));
}

/**
 * Picks the more recent of two snapshots. Command results and change events
 * can arrive in either order, so the frontend keeps whichever has the higher
 * revision instead of whichever arrived last.
 */
export function newerSnapshot(
  current: WorkspaceSnapshot | null,
  incoming: WorkspaceSnapshot,
): WorkspaceSnapshot {
  return current !== null && current.revision > incoming.revision ? current : incoming;
}
