/**
 * Typed bindings for the backend's application-level Tauri commands.
 *
 * The frontend never computes authoritative state itself; it asks Rust.
 * Each binding mirrors one `#[tauri::command]` in `src-tauri/src/commands.rs`.
 */
import { invoke } from '@tauri-apps/api/core';

/** Mirrors `commands::AppInfo` in Rust (serialized as camelCase). */
export interface AppInfo {
  name: string;
  version: string;
  schemaVersion: number;
}

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>('get_app_info');
}
