import type { Entry, Snapshot } from './types';
export const statusLabels: Record<string, string> = {
  incomplete: '无法完成配置检查',
  mismatch: '配置不匹配',
  missing: '基础配置缺失',
  review: '覆盖待核实',
  unconfigured: '未设置规则',
  passed: '配置检查通过',
  loading: '读取中',
  unavailable: '尚未读取',
};
export const kindLabels: Record<string, string> = {
  normal: '普通仓库',
  worktree: '工作树',
  bare: '裸仓库',
  submodule: '子模块',
};
export const resultLabels: Record<string, string> = {
  applied: '保存结果已验证',
  overridden: '写入成功但仍被覆盖',
  partial: '部分写入',
  failed: '写入失败',
  unverified: '写入后无法验证',
};
export function entryName(entry: Entry): string {
  return (
    entry.alias || entry.path.replace(/\\/g, '/').split('/').filter(Boolean).at(-1) || entry.path
  );
}
export function identityState(snapshot?: Snapshot): string {
  if (!snapshot) return 'loading';
  if (
    snapshot.context.state === 'failed' ||
    snapshot.identity.state === 'failed' ||
    snapshot.identity.state === 'partial'
  )
    return 'incomplete';
  return snapshot.identity.data?.status || 'unavailable';
}
export function needsCheck(snapshot?: Snapshot): boolean {
  return ['incomplete', 'mismatch', 'missing', 'review'].includes(identityState(snapshot));
}
export function readErrors(snapshot?: Snapshot): boolean {
  return (
    !!snapshot &&
    [
      snapshot.context,
      snapshot.head,
      snapshot.work,
      snapshot.history,
      snapshot.upstream,
      snapshot.identity,
      snapshot.remotes,
    ].some((m) => m.state === 'failed' || m.state === 'partial')
  );
}
export function configValue(snapshot: Snapshot | undefined, key: string): string | null {
  return snapshot?.identity.data?.keys.find((k) => k.key === key)?.values.at(-1)?.value ?? null;
}
export function platforms(snapshot?: Snapshot): string {
  const values = [
    ...new Set(snapshot?.remotes.data?.map((r) => r.platform || r.host || r.protocol)),
  ];
  return values.length > 1
    ? '多个平台/主机'
    : values[0] || (snapshot?.remotes.state === 'success' ? '无远程' : '—');
}
export interface Filters {
  query: string;
  view: string;
  kind: string;
  identity: string;
  work: string;
}
export function filterEntries(
  entries: Entry[],
  snapshots: Record<string, Snapshot>,
  filters: Filters,
): Entry[] {
  const query = filters.query.toLocaleLowerCase();
  return entries.filter((e) => {
    const s = snapshots[e.id];
    if (filters.view === 'attention' && !needsCheck(s)) return false;
    if (filters.view === 'errors' && !readErrors(s)) return false;
    if (filters.view.startsWith('group:') && e.group !== filters.view.slice(6)) return false;
    if (filters.kind && (s?.context.data?.kind || e.kind) !== filters.kind) return false;
    if (filters.identity && identityState(s) !== filters.identity) return false;
    const w = s?.work.data;
    if (filters.work === 'changed' && (!w || w.staged + w.unstaged + w.untracked === 0))
      return false;
    if (filters.work === 'clean' && (!w || w.staged + w.unstaged + w.untracked !== 0)) return false;
    if (!query) return true;
    return [
      entryName(e),
      e.path,
      configValue(s, 'user.name'),
      configValue(s, 'user.email'),
      ...(s?.identity.data?.findings.map((f) => f.value) || []),
      ...(s?.remotes.data?.flatMap((r) => [r.host, r.platform, r.address]) || []),
    ]
      .filter(Boolean)
      .join(' ')
      .toLocaleLowerCase()
      .includes(query);
  });
}
