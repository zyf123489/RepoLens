import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  Change,
  Context,
  Diagnostics,
  Entry,
  Policy,
  Preview,
  ScanOptions,
  ScanState,
  Snapshot,
  WriteResult,
} from './types';

export const desktop = isTauri();
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop)
    throw {
      code: 'desktop',
      message: '请在 RepoLens 桌面应用中使用本地仓库功能。网页预览不访问本机仓库。',
    };
  return invoke<T>(command, args);
}
export const api = {
  diagnostics: () => call<Diagnostics>('diagnostics'),
  entries: () => call<Entry[]>('list_entries'),
  groups: () => call<string[]>('list_groups'),
  inspect: (path: string) => call<Context>('inspect_path', { path }),
  add: (paths: string[]) => call<Entry[]>('add_entries', { paths }),
  snapshot: (id: string) => call<Snapshot>('read_snapshot', { id }),
  update: (id: string, alias: string, group: string | null, policy: Policy) =>
    call<Entry>('update_entry', { id, alias, group, policy }),
  remove: (id: string) => call<void>('remove_entry', { id }),
  saveGroup: (name: string) => call<void>('save_group', { name }),
  deleteGroup: (name: string) => call<void>('delete_group', { name }),
  setGit: (path: string) => call<Diagnostics>('set_git', { path }),
  preview: (id: string, changes: Change[]) => call<Preview>('create_preview', { id, changes }),
  apply: (token: string) => call<WriteResult>('apply_preview', { token }),
  scan: (options: ScanOptions) => call<string>('start_scan', { options }),
  pollScan: (id: string) => call<ScanState>('poll_scan', { id }),
  stopScan: (id: string) => call<void>('cancel_scan', { id }),
  chooseDirectory: async (): Promise<string | null> => {
    if (!desktop) throw { message: '目录选择仅在桌面应用中可用。' };
    const path = await open({ directory: true, multiple: false, title: '选择仓库或扫描目录' });
    return typeof path === 'string' ? path : null;
  },
};
export function errorText(error: unknown): string {
  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string'
  )
    return error.message;
  return typeof error === 'string' ? error : '操作未完成，请重试。';
}
