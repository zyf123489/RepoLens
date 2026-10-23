import { describe, expect, it } from 'vitest';
import {
  configValue,
  entryName,
  filterEntries,
  identityState,
  needsCheck,
  platforms,
  readErrors,
} from './view';
import type { Endpoint, Entry, Snapshot } from './types';
const entry: Entry = {
  id: 'one',
  path: '/work/repo',
  alias: '',
  group: 'Work',
  kind: 'normal',
  policy: { name: null, email: null, domains: [] },
};
function snapshot(status: string): Snapshot {
  const m = <T>(data: T) => ({ state: 'success', data, error: null, readAt: 1 });
  return {
    entryId: 'one',
    readAt: 1,
    git: 'git',
    context: m({
      path: entry.path,
      gitDir: '/git',
      commonDir: '/git',
      kind: 'normal',
      parent: null,
    }),
    identity: m({ status, keys: [], findings: [], checked: '' }),
    work: m({ staged: 0, unstaged: 0, untracked: 0, conflicts: 0 }),
    head: m({ branch: 'main', oid: null, detached: false, unborn: true }),
    history: { ...m(null), state: 'empty' },
    remotes: m([]),
    upstream: m({ state: 'unconfigured', name: null, ahead: null, behind: null }),
  };
}
describe('workspace business filters', () => {
  it('does not classify absent rules as attention', () =>
    expect(needsCheck(snapshot('unconfigured'))).toBe(false));
  it('does classify missing values as attention', () =>
    expect(needsCheck(snapshot('missing'))).toBe(true));
  it('preserves identity success when work status fails', () => {
    const s = snapshot('passed');
    s.work = { state: 'failed', data: null, error: { code: 'work', message: 'failed' }, readAt: 1 };
    expect(identityState(s)).toBe('passed');
    expect(readErrors(s)).toBe(true);
    expect(needsCheck(s)).toBe(false);
  });
  it('never treats failed work status as clean', () => {
    const s = snapshot('passed');
    s.work = { state: 'failed', data: null, error: { code: 'work', message: 'failed' }, readAt: 1 };
    expect(
      filterEntries(
        [entry],
        { one: s },
        { query: '', view: 'all', kind: '', identity: '', work: 'clean' },
      ),
    ).toHaveLength(0);
  });
  it('combines group and search filters', () => {
    expect(
      filterEntries(
        [entry],
        {},
        { query: 'repo', view: 'group:Work', kind: 'normal', identity: '', work: '' },
      ),
    ).toHaveLength(1);
    expect(
      filterEntries(
        [entry],
        {},
        { query: 'repo', view: 'group:Personal', kind: '', identity: '', work: '' },
      ),
    ).toHaveLength(0);
  });
  it('does not report unread entries as passed', () =>
    expect(identityState(undefined)).toBe('loading'));
});

describe('repository display boundaries', () => {
  it('uses aliases and handles Windows, trailing separators, and root paths', () => {
    expect(entryName({ ...entry, alias: '工作项目' })).toBe('工作项目');
    expect(entryName({ ...entry, path: 'C:\\work\\中文 repo\\' })).toBe('中文 repo');
    expect(entryName({ ...entry, path: '/' })).toBe('/');
    expect(entryName({ ...entry, path: 'C:\\' })).toBe('C:');
  });

  it('distinguishes unknown configuration, absent values, and explicit empty overrides', () => {
    const s = snapshot('passed');
    expect(configValue(undefined, 'user.email')).toBeNull();
    expect(configValue(s, 'user.email')).toBeNull();
    s.identity.data!.keys = [{ key: 'user.email', values: [] }];
    expect(configValue(s, 'user.email')).toBeNull();
    s.identity.data!.keys[0].values = [
      { value: 'before@example.com', scope: 'global', origin: 'file:global' },
      { value: '', scope: 'local', origin: 'file:local' },
    ];
    expect(configValue(s, 'user.email')).toBe('');
  });

  it('deduplicates remote platforms and keeps failed reads separate from no remotes', () => {
    const s = snapshot('passed');
    const endpoint = (platform: string | null, host: string, protocol = 'https'): Endpoint => ({
      remote: 'origin',
      usage: 'fetch',
      address: '',
      configuredAddress: '',
      protocol,
      host,
      platform,
      username: null,
      metadata: [],
      limitations: [],
      configState: 'read',
      verification: 'untested',
    });
    expect(platforms(undefined)).toBe('—');
    expect(platforms(s)).toBe('无远程');
    s.remotes.state = 'failed';
    s.remotes.data = null;
    expect(platforms(s)).toBe('—');
    s.remotes.state = 'success';
    s.remotes.data = [endpoint('GitHub', 'github.com'), endpoint('GitHub', 'github.com')];
    expect(platforms(s)).toBe('GitHub');
    s.remotes.data.push(endpoint(null, 'company.example'));
    expect(platforms(s)).toBe('多个平台/主机');
    s.remotes.data = [endpoint(null, '', 'local')];
    expect(platforms(s)).toBe('local');
  });

  it('keeps partial identity reads in attention while preserving known fields', () => {
    const s = snapshot('passed');
    s.identity.state = 'partial';
    s.identity.data!.keys = [
      { key: 'user.name', values: [{ value: 'Known User', scope: 'local', origin: 'file:local' }] },
    ];
    expect(identityState(s)).toBe('incomplete');
    expect(needsCheck(s)).toBe(true);
    expect(configValue(s, 'user.name')).toBe('Known User');
  });
});
