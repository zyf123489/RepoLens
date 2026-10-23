import { describe, expect, it } from 'vitest';
import { filterEntries, identityState, needsCheck, readErrors } from './view';
import type { Entry, Snapshot } from './types';
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
