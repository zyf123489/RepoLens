import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import {
  Activity,
  ArrowLeft,
  ArrowUpRight,
  Check,
  CheckCircle2,
  ChevronRight,
  Circle,
  CircleAlert,
  Database,
  Folder,
  FolderGit2,
  FolderPlus,
  GitBranch,
  Layers,
  LoaderCircle,
  Plus,
  RefreshCw,
  ScanSearch,
  Search,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Trash2,
  X,
} from 'lucide-react';
import { api, desktop, errorText } from './api';
import type {
  Change,
  ConfigKey,
  Context,
  Diagnostics,
  Entry,
  Module as DataModule,
  Policy,
  Preview,
  ScanState,
  Snapshot,
  WriteResult,
} from './types';
import {
  configValue,
  entryName,
  filterEntries,
  identityState,
  kindLabels,
  needsCheck,
  platforms,
  readErrors,
  resultLabels,
  statusLabels,
} from './view';

function Badge({ state, children }: { state: string; children?: ReactNode }) {
  const good = state === 'passed' || state === 'match' || state === 'applied';
  const warning = [
    'mismatch',
    'missing',
    'review',
    'incomplete',
    'failed',
    'overridden',
    'unverified',
    'partial',
  ].includes(state);
  return (
    <span className={`badge ${good ? 'good' : warning ? 'warning' : 'neutral'}`}>
      {good ? (
        <CheckCircle2 size={12} />
      ) : warning ? (
        <CircleAlert size={12} />
      ) : (
        <Circle size={10} />
      )}{' '}
      {children || statusLabels[state] || state}
    </span>
  );
}
function Modal({
  title,
  children,
  onClose,
  busy = false,
  wide = false,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  busy?: boolean;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
  }, []);
  return (
    <dialog
      ref={ref}
      className={wide ? 'modal wide' : 'modal'}
      onCancel={(e) => {
        e.preventDefault();
        if (!busy) onClose();
      }}
      aria-label={title}
    >
      <header>
        <h2>{title}</h2>
        <button className="icon-button" onClick={onClose} disabled={busy} aria-label="关闭">
          <X size={18} />
        </button>
      </header>
      {children}
    </dialog>
  );
}
function ModuleState({ module, children }: { module?: DataModule<unknown>; children: ReactNode }) {
  if (!module)
    return (
      <p className="muted">
        <LoaderCircle className="spin" size={14} /> 读取中…
      </p>
    );
  if (module.error && !module.data)
    return (
      <p className="inline-error">
        <CircleAlert size={14} />
        {module.error.message}
      </p>
    );
  if (module.state === 'empty') return <p className="muted">暂无数据</p>;
  if (module.state === 'notApplicable') return <p className="muted">不适用</p>;
  if (!module.data) return <p className="muted">尚无法读取</p>;
  return (
    <>
      {module.error && <p className="inline-error">{module.error.message}</p>}
      {children}
    </>
  );
}
function ConfigSource({ config }: { config: ConfigKey }) {
  return (
    <div className="source">
      <div className="source-title">
        <code>{config.key}</code>
        <strong>{config.values.at(-1)?.value ?? '未配置'}</strong>
      </div>
      {config.values.map((v, i) => (
        <div
          className={`source-row ${i === config.values.length - 1 ? 'effective' : ''}`}
          key={`${i}-${v.origin}`}
        >
          <span>
            {i === config.values.length - 1 ? <Check size={13} /> : <ChevronRight size={13} />}
          </span>
          <div>
            <span className="scope">{v.scope}</span>
            <span>{v.value || '显式空值'}</span>
            <small>{v.origin}</small>
          </div>
        </div>
      ))}
    </div>
  );
}

export default function App() {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [groups, setGroups] = useState<string[]>([]);
  const [snapshots, setSnapshots] = useState<Record<string, Snapshot>>({});
  const [diagnostics, setDiagnostics] = useState<Diagnostics>();
  const [selected, setSelected] = useState<string>();
  const [view, setView] = useState('all');
  const [query, setQuery] = useState('');
  const [kind, setKind] = useState('');
  const [identityFilter, setIdentityFilter] = useState('');
  const [workFilter, setWorkFilter] = useState('');
  const [showFilters, setShowFilters] = useState(false);
  const [tab, setTab] = useState('overview');
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [refreshing, setRefreshing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [dialog, setDialog] = useState<
    'add' | 'scan' | 'edit' | 'policy' | 'settings' | 'group' | 'remove' | null
  >(null);
  const [candidate, setCandidate] = useState<Context>();
  const [pathInput, setPathInput] = useState('');
  const [groupInput, setGroupInput] = useState('');
  const [scanRoots, setScanRoots] = useState<string[]>([]);
  const [depth, setDepth] = useState(3);
  const [excludeNames, setExcludeNames] = useState(
    'node_modules, target, dist, build, .venv, venv, .cache',
  );
  const [excludePaths, setExcludePaths] = useState('');
  const [scan, setScan] = useState<ScanState>();
  const [checkedPaths, setCheckedPaths] = useState<Set<string>>(new Set());
  const [preview, setPreview] = useState<Preview>();
  const [writeResult, setWriteResult] = useState<WriteResult>();
  const [editName, setEditName] = useState('');
  const [editEmail, setEditEmail] = useState('');
  const [changeName, setChangeName] = useState(false);
  const [changeEmail, setChangeEmail] = useState(false);
  const [policyName, setPolicyName] = useState('');
  const [policyEmail, setPolicyEmail] = useState('');
  const [policyDomains, setPolicyDomains] = useState('');
  const [nameRule, setNameRule] = useState(false);
  const [emailRule, setEmailRule] = useState(false);
  const [domainRule, setDomainRule] = useState(false);
  const [alias, setAlias] = useState('');
  const [entryGroup, setEntryGroup] = useState('');
  const [gitInput, setGitInput] = useState('git');
  const generation = useRef(0);
  const refresh = useCallback(async (list: Entry[]) => {
    const current = ++generation.current;
    setRefreshing(true);
    let index = 0;
    async function worker() {
      while (index < list.length && generation.current === current) {
        const entry = list[index++];
        try {
          const s = await api.snapshot(entry.id);
          if (generation.current === current) setSnapshots((prev) => ({ ...prev, [entry.id]: s }));
        } catch (e) {
          if (generation.current === current) setError(errorText(e));
        }
      }
    }
    await Promise.all(Array.from({ length: Math.min(3, list.length) }, worker));
    if (generation.current === current) setRefreshing(false);
  }, []);
  const reload = useCallback(async () => {
    const [list, groupList, d] = await Promise.all([
      api.entries(),
      api.groups(),
      api.diagnostics(),
    ]);
    setEntries(list);
    setGroups(groupList);
    setDiagnostics(d);
    setGitInput(d.git);
    await refresh(list);
  }, [refresh]);
  useEffect(() => {
    if (desktop) void reload().catch((e) => setError(errorText(e)));
    return () => {
      generation.current++;
    };
  }, [reload]);
  useEffect(() => {
    const focus = () => {
      if (desktop && !busy && !refreshing && !dialog && !preview) void refresh(entries);
    };
    window.addEventListener('focus', focus);
    return () => window.removeEventListener('focus', focus);
  }, [entries, busy, refreshing, dialog, preview, refresh]);
  useEffect(() => {
    if (!scan || scan.state !== 'running') return;
    let active = true;
    const timer = setInterval(() => {
      void api
        .pollScan(scan.id)
        .then((s) => {
          if (active) setScan(s);
        })
        .catch((e) => {
          if (active) setError(errorText(e));
        });
    }, 500);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [scan?.id, scan?.state]);
  const action = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await fn();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  const filtered = useMemo(
    () =>
      filterEntries(entries, snapshots, {
        query,
        view,
        kind,
        identity: identityFilter,
        work: workFilter,
      }),
    [entries, snapshots, query, view, kind, identityFilter, workFilter],
  );
  const entry = entries.find((e) => e.id === selected);
  const snapshot = selected ? snapshots[selected] : undefined;
  const attention = entries.filter((e) => needsCheck(snapshots[e.id])).length;
  const passed = entries.filter((e) => identityState(snapshots[e.id]) === 'passed').length;
  const errorCount = entries.filter((e) => readErrors(snapshots[e.id])).length;
  const close = () => {
    if (!busy) {
      setDialog(null);
      setError('');
    }
  };
  const choosePath = async () => {
    const path = await api.chooseDirectory();
    if (path) {
      setPathInput(path);
      setCandidate(undefined);
    }
  };
  const openEdit = () => {
    if (!entry) return;
    setEditName(configValue(snapshot, 'user.name') ?? '');
    setEditEmail(configValue(snapshot, 'user.email') ?? '');
    setChangeName(false);
    setChangeEmail(false);
    setDialog('edit');
    setError('');
  };
  const openPolicy = () => {
    if (!entry) return;
    setAlias(entry.alias);
    setEntryGroup(entry.group ?? '');
    setPolicyName(entry.policy.name ?? '');
    setPolicyEmail(entry.policy.email ?? '');
    setPolicyDomains(entry.policy.domains.join(', '));
    setNameRule(entry.policy.name !== null);
    setEmailRule(entry.policy.email !== null);
    setDomainRule(entry.policy.domains.length > 0);
    setDialog('policy');
    setError('');
  };
  const makePreview = async (changes: Change[]) => {
    if (!entry) return;
    const p = await api.preview(entry.id, changes);
    setPreview(p);
    setError('');
  };
  const afterAdd = async (list: Entry[]) => {
    setEntries(list);
    setDialog(null);
    setCandidate(undefined);
    setMessage('仓库已收录。扫描和添加不会修改 Git 配置。');
    if (list.length) setSelected(list.at(-1)!.id);
    await refresh(list);
  };
  const title =
    view === 'attention'
      ? '需要检查'
      : view === 'errors'
        ? '读取异常'
        : view.startsWith('group:')
          ? view.slice(6)
          : '全部仓库';

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">
            <Layers size={21} />
          </span>
          <span>
            RepoLens<small>LOCAL GIT WORKSPACE</small>
          </span>
        </div>
        <div className="workspace-label">工作台</div>
        <nav aria-label="仓库导航">
          <button className={view === 'all' ? 'nav active' : 'nav'} onClick={() => setView('all')}>
            <FolderGit2 size={17} />
            全部仓库<span>{entries.length}</span>
          </button>
          <button
            className={view === 'attention' ? 'nav active' : 'nav'}
            onClick={() => setView('attention')}
          >
            <ShieldCheck size={17} />
            需要检查{attention > 0 && <span className="nav-count">{attention}</span>}
          </button>
          <button
            className={view === 'errors' ? 'nav active' : 'nav'}
            onClick={() => setView('errors')}
          >
            <Activity size={17} />
            读取异常{errorCount > 0 && <span>{errorCount}</span>}
          </button>
        </nav>
        <div className="workspace-label group-heading">
          用户分组
          <button
            className="icon-button"
            aria-label="新建分组"
            onClick={() => {
              setGroupInput('');
              setDialog('group');
            }}
          >
            <Plus size={14} />
          </button>
        </div>
        <nav aria-label="用户分组">
          {groups.map((g) => (
            <div className="group-row" key={g}>
              <button
                className={view === `group:${g}` ? 'nav active' : 'nav'}
                onClick={() => setView(`group:${g}`)}
              >
                <Folder size={16} />
                <span className="group-name">{g}</span>
                <span>{entries.filter((e) => e.group === g).length}</span>
              </button>
              <button
                className="group-delete icon-button"
                aria-label={`删除分组 ${g}，保留仓库`}
                onClick={() =>
                  void action(async () => {
                    await api.deleteGroup(g);
                    setView('all');
                    await reload();
                  })
                }
              >
                <X size={12} />
              </button>
            </div>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="offline-card">
            <span className="status-dot" />
            <div>
              本地工作模式<small>默认离线 · 不自动同步</small>
            </div>
          </div>
          <button
            className="nav"
            onClick={() => {
              setError('');
              setDialog('settings');
            }}
          >
            <Settings2 size={17} />
            设置<span>{diagnostics?.version || 'alpha'}</span>
          </button>
        </div>
      </aside>
      <main className="main">
        <header className="topbar">
          <div className="breadcrumb">
            工作台
            <ChevronRight size={13} />
            <strong>{title}</strong>
          </div>
          <span className="local-chip">
            <span className="status-dot" />
            仅本地读取
          </span>
        </header>
        <section className="page-intro">
          <div>
            <div className="eyebrow">REPOSITORY WORKSPACE</div>
            <h1>
              {title}
              <span>{entries.length}</span>
            </h1>
            <p>看清每个仓库的状态，让提交配置符合你的期望。</p>
          </div>
          <div className="intro-actions">
            <button
              className="button"
              onClick={() => {
                setDialog('scan');
                setError('');
              }}
            >
              <ScanSearch size={16} />
              扫描目录
            </button>
            <button
              className="button primary"
              onClick={() => {
                setPathInput('');
                setCandidate(undefined);
                setError('');
                setDialog('add');
              }}
            >
              <Plus size={16} />
              添加仓库
            </button>
          </div>
        </section>
        {!desktop && (
          <div className="notice">
            <CircleAlert size={16} />
            当前是界面预览。请通过桌面应用访问本地仓库；这里不展示模拟仓库数据。
          </div>
        )}
        {diagnostics?.gitError && (
          <div className="notice warning">
            <CircleAlert size={16} />
            {diagnostics.gitError.message}
            <button onClick={() => setDialog('settings')}>配置 Git</button>
          </div>
        )}
        {error && !dialog && !preview && (
          <div role="alert" className="notice warning">
            {error}
            <button onClick={() => setError('')}>关闭</button>
          </div>
        )}
        {message && (
          <div className="notice success" role="status">
            {message}
            <button onClick={() => setMessage('')}>关闭</button>
          </div>
        )}
        <div className="summary-grid">
          <div className="summary-card">
            <span>
              <FolderGit2 size={16} />
              已收录条目
            </span>
            <strong>
              {entries.length}
              <small>个本地仓库 / 工作树</small>
            </strong>
          </div>
          <div className="summary-card">
            <span>
              <CheckCircle2 size={16} />
              配置检查通过
            </span>
            <strong>
              {passed}
              <small>仅评价已检查配置</small>
            </strong>
          </div>
          <div className="summary-card">
            <span>
              <CircleAlert size={16} />
              需要检查
            </span>
            <strong className={attention ? 'amber' : ''}>
              {attention}
              <small>缺失、不匹配或待核实</small>
            </strong>
          </div>
        </div>
        <section className="repository-panel">
          <div className="toolbar">
            <div className="search">
              <Search size={16} />
              <input
                aria-label="搜索仓库"
                placeholder="搜索仓库、路径、身份或远程主机…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
              <kbd>⌕</kbd>
            </div>
            <button
              className={`button ${showFilters ? 'selected' : ''}`}
              onClick={() => setShowFilters(!showFilters)}
            >
              <SlidersHorizontal size={15} />
              筛选
            </button>
            <button
              className="button"
              disabled={refreshing || !desktop || busy}
              onClick={() => void action(() => refresh(entries))}
            >
              <RefreshCw size={15} className={refreshing ? 'spin' : ''} />
              {refreshing ? '读取中' : '刷新'}
            </button>
          </div>
          {showFilters && (
            <div className="filters">
              <label>
                类型
                <select value={kind} onChange={(e) => setKind(e.target.value)}>
                  <option value="">全部类型</option>
                  {Object.entries(kindLabels).map(([k, v]) => (
                    <option key={k} value={k}>
                      {v}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                配置检查
                <select value={identityFilter} onChange={(e) => setIdentityFilter(e.target.value)}>
                  <option value="">全部结果</option>
                  {Object.entries(statusLabels)
                    .filter(([k]) => !['loading', 'unavailable'].includes(k))
                    .map(([k, v]) => (
                      <option key={k} value={k}>
                        {v}
                      </option>
                    ))}
                </select>
              </label>
              <label>
                工作区
                <select value={workFilter} onChange={(e) => setWorkFilter(e.target.value)}>
                  <option value="">全部状态</option>
                  <option value="changed">有修改</option>
                  <option value="clean">无修改</option>
                </select>
              </label>
              <button
                className="text-button"
                onClick={() => {
                  setKind('');
                  setIdentityFilter('');
                  setWorkFilter('');
                  setQuery('');
                }}
              >
                重置
              </button>
            </div>
          )}
          <div className={`workbench ${entry ? 'has-detail' : ''}`}>
            <div className="table-wrap">
              <table className="repo-table">
                <thead>
                  <tr>
                    <th>仓库 / 分支</th>
                    <th>工作区</th>
                    <th>提交身份</th>
                    <th>远程</th>
                    <th>配置检查</th>
                  </tr>
                </thead>
                <tbody>
                  {filtered.map((e) => {
                    const s = snapshots[e.id];
                    const w = s?.work.data;
                    return (
                      <tr
                        key={e.id}
                        className={selected === e.id ? 'selected-row' : ''}
                        tabIndex={0}
                        aria-selected={selected === e.id}
                        onClick={() => {
                          setSelected(e.id);
                          setTab('overview');
                        }}
                        onKeyDown={(event) => {
                          if (event.key === 'Enter' || event.key === ' ') {
                            event.preventDefault();
                            setSelected(e.id);
                          }
                        }}
                      >
                        <td>
                          <div className="repo-name">
                            <FolderGit2 size={17} />
                            <strong>{entryName(e)}</strong>
                          </div>
                          <small className="repo-path" title={e.path}>
                            {e.path}
                          </small>
                          <span className="branch">
                            <GitBranch size={11} />
                            {s?.head.data?.unborn
                              ? '暂无提交'
                              : s?.head.data?.branch || s?.head.data?.oid?.slice(0, 8) || '—'}
                            <em>{kindLabels[s?.context.data?.kind || e.kind]}</em>
                          </span>
                        </td>
                        <td>
                          {w ? (
                            <div className="work-counts">
                              {w.staged + w.unstaged + w.untracked === 0 ? (
                                <span className="clean-dot">无修改</span>
                              ) : (
                                <>
                                  <span title="已暂存">暂存 {w.staged}</span>
                                  <span title="未暂存">修改 {w.unstaged}</span>
                                  <span title="未跟踪">未跟踪 {w.untracked}</span>
                                </>
                              )}
                            </div>
                          ) : (
                            <span className="muted">
                              {s?.work.state === 'notApplicable'
                                ? '不适用'
                                : s?.work.error
                                  ? '读取失败'
                                  : s
                                    ? '不可读取'
                                    : '读取中'}
                            </span>
                          )}
                        </td>
                        <td>
                          <span>{configValue(s, 'user.name') ?? '—'}</span>
                          <small>{configValue(s, 'user.email') ?? '—'}</small>
                        </td>
                        <td>
                          <span className="platform">{platforms(s)}</span>
                        </td>
                        <td>
                          <Badge state={identityState(s)} />
                          {readErrors(s) && <small className="amber">存在模块读取异常</small>}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
              {filtered.length === 0 && (
                <div className="empty-state">
                  <span className="empty-icon">
                    <FolderGit2 size={35} />
                  </span>
                  <h2>{entries.length ? '没有符合条件的仓库' : '把你的仓库放在同一个工作台'}</h2>
                  <p>
                    {entries.length
                      ? '调整搜索或筛选条件，查看其他仓库。'
                      : '添加一个仓库，或扫描项目目录，开始检查提交配置。'}
                  </p>
                  {entries.length ? (
                    <button
                      className="button"
                      onClick={() => {
                        setQuery('');
                        setKind('');
                        setIdentityFilter('');
                        setWorkFilter('');
                        setView('all');
                      }}
                    >
                      查看全部仓库
                    </button>
                  ) : (
                    <div>
                      <button className="button primary" onClick={() => setDialog('add')}>
                        <FolderPlus size={16} />
                        添加第一个仓库
                      </button>
                      <button className="text-button" onClick={() => setDialog('scan')}>
                        扫描目录
                        <ArrowUpRight size={14} />
                      </button>
                    </div>
                  )}
                  <div className="empty-assurance">
                    <ShieldCheck size={14} />
                    默认只读检查，修改前由你确认
                  </div>
                </div>
              )}
            </div>
            {entry && (
              <aside className="detail">
                <div className="detail-heading">
                  <div>
                    <span className="eyebrow">REPOSITORY DETAILS</span>
                    <h2>{entryName(entry)}</h2>
                  </div>
                  <button
                    className="icon-button"
                    aria-label="关闭详情"
                    onClick={() => setSelected(undefined)}
                  >
                    <X size={17} />
                  </button>
                </div>
                <p className="detail-path">{entry.path}</p>
                <Badge state={identityState(snapshot)} />
                <div className="tabs" role="tablist" aria-label="仓库详情">
                  {[
                    ['overview', '概览'],
                    ['identity', '提交身份'],
                    ['auth', '认证概览'],
                  ].map(([id, label]) => (
                    <button
                      key={id}
                      role="tab"
                      aria-selected={tab === id}
                      className={tab === id ? 'active' : ''}
                      onClick={() => setTab(id)}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                <div className="detail-content">
                  {tab === 'overview' && (
                    <>
                      <h3>仓库上下文</h3>
                      <ModuleState module={snapshot?.context}>
                        <dl>
                          <dt>类型</dt>
                          <dd>{kindLabels[snapshot?.context.data?.kind || entry.kind]}</dd>
                          <dt>Git 目录</dt>
                          <dd>{snapshot?.context.data?.gitDir}</dd>
                          <dt>共享目录</dt>
                          <dd>{snapshot?.context.data?.commonDir}</dd>
                          {snapshot?.context.data?.parent && (
                            <>
                              <dt>所属主项目</dt>
                              <dd>{snapshot.context.data.parent}</dd>
                            </>
                          )}
                        </dl>
                      </ModuleState>
                      <h3>最近提交</h3>
                      <ModuleState module={snapshot?.history}>
                        <p className="commit-subject">{snapshot?.history.data?.subject}</p>
                        <p>
                          {snapshot?.history.data?.author}
                          <small>{snapshot?.history.data?.email}</small>
                        </p>
                        <small>{snapshot?.history.data?.date}</small>
                      </ModuleState>
                      <h3>上游状态</h3>
                      <ModuleState module={snapshot?.upstream}>
                        {snapshot?.upstream.data?.state === 'available' ? (
                          <p>
                            {snapshot.upstream.data.name}
                            <br />
                            领先 {snapshot.upstream.data.ahead} · 落后{' '}
                            {snapshot.upstream.data.behind}
                          </p>
                        ) : (
                          <p className="muted">
                            {snapshot?.upstream.data?.state === 'missing'
                              ? '上游本地引用缺失'
                              : '未配置上游'}
                          </p>
                        )}
                      </ModuleState>
                      <p className="hint">
                        根据本地引用计算，未连接远程更新，可能与远端当前状态不同。
                      </p>
                      <h3>整理与检查规则</h3>
                      <p className="muted">
                        分组：{entry.group || '未分组'}
                        <br />
                        规则独立于分组，不自动写入 Git。
                      </p>
                      <button className="button full" onClick={openPolicy}>
                        <Settings2 size={14} />
                        设置期望身份与条目
                      </button>
                    </>
                  )}
                  {tab === 'identity' && (
                    <>
                      <p className="hint">
                        检查范围是当前上下文的 Git
                        身份配置。实际提交还可能受环境变量、本次命令参数或其他工具影响。
                      </p>
                      <ModuleState module={snapshot?.identity}>
                        <p className="hint">{snapshot?.identity.data?.checked}</p>
                        <div className="findings">
                          {snapshot?.identity.data?.findings.map((f) => (
                            <div className="finding" key={f.field}>
                              <code>{f.field}</code>
                              <strong>{f.value ?? '未配置'}</strong>
                              <small
                                className={
                                  ['mismatch', 'missing', 'review'].includes(f.state) ? 'amber' : ''
                                }
                              >
                                {f.reason}
                              </small>
                            </div>
                          ))}
                        </div>
                        <h3>配置来源与覆盖顺序</h3>
                        <p className="hint">
                          按读取顺序展示，最后一项为该键生效值。未提供完整包含链。
                        </p>
                        {snapshot?.identity.data?.keys.map((k) => (
                          <ConfigSource key={k.key} config={k} />
                        ))}
                        <button className="button primary full" onClick={openEdit}>
                          修改仓库级配置
                        </button>
                        <div className="remove-config">
                          <button
                            className="text-button"
                            onClick={() =>
                              void action(() => makePreview([{ key: 'user.name', value: null }]))
                            }
                          >
                            移除 local 姓名
                          </button>
                          <button
                            className="text-button"
                            onClick={() =>
                              void action(() => makePreview([{ key: 'user.email', value: null }]))
                            }
                          >
                            移除 local 邮箱
                          </button>
                        </div>
                      </ModuleState>
                      <button className="button full" onClick={openPolicy}>
                        设置期望身份规则
                      </button>
                    </>
                  )}
                  {tab === 'auth' && (
                    <>
                      <p className="hint">
                        这里回答“配置了什么”。实际认证账号未知，连接未测试，不提供账号切换。
                      </p>
                      <ModuleState module={snapshot?.remotes}>
                        {snapshot?.remotes.data?.length === 0 ? (
                          <p className="muted">没有配置远程端点</p>
                        ) : (
                          snapshot?.remotes.data?.map((r, i) => (
                            <div className="remote-card" key={i}>
                              <div>
                                <strong>{r.remote}</strong>
                                <span className="scope">
                                  {r.usage === 'fetch' ? '获取' : '推送'}
                                </span>
                                <span className="scope">{r.protocol}</span>
                              </div>
                              <p>{r.address}</p>
                              {r.address !== r.configuredAddress && (
                                <small>配置地址：{r.configuredAddress}</small>
                              )}
                              <small>
                                主机：{r.host || '不适用'} · 平台：{r.platform || '未知'}
                              </small>
                              {r.username && (
                                <small>SSH 用户：{r.username}（不等于平台账号）</small>
                              )}
                              <div className="remote-meta">
                                {r.metadata.map((m, j) => (
                                  <small key={j}>{m}</small>
                                ))}
                              </div>
                              {r.limitations.map((m, j) => (
                                <p className="hint" key={j}>
                                  {m}
                                </p>
                              ))}
                              <Badge state="unconfigured">未测试 · 账号未知</Badge>
                            </div>
                          ))
                        )}
                      </ModuleState>
                    </>
                  )}
                </div>
                <footer className="detail-footer">
                  <small>
                    {snapshot
                      ? `读取于 ${new Date(snapshot.readAt * 1000).toLocaleTimeString()}`
                      : '等待首次读取'}
                  </small>
                  <button
                    className="icon-button danger"
                    onClick={() => setDialog('remove')}
                    aria-label="移出列表，保留本地文件"
                  >
                    <Trash2 size={15} />
                  </button>
                </footer>
              </aside>
            )}
          </div>
          <div className="table-footer">
            <span>
              显示 {filtered.length} / {entries.length} 个条目
            </span>
            <span>
              <ShieldCheck size={12} />
              配置检查不保证实际提交身份
            </span>
          </div>
        </section>
        <footer className="statusbar">
          <span>
            {refreshing ? (
              <>
                <LoaderCircle size={12} className="spin" />
                正在读取本地信息
              </>
            ) : (
              <>
                <span className="status-dot" />
                工作台就绪
              </>
            )}
          </span>
          <span>
            {diagnostics?.gitVersion || '本机 Git'}
            <span className="separator">|</span>不会自动 fetch / pull / push
          </span>
        </footer>
      </main>

      {dialog === 'add' && (
        <Modal title="添加本地仓库" onClose={close} busy={busy}>
          <div className="modal-body">
            <p className="hint">可以选择仓库内部目录。确认后将收录实际工作区根路径。</p>
            <label>
              本地路径
              <div className="input-action">
                <input
                  autoFocus
                  value={pathInput}
                  onChange={(e) => {
                    setPathInput(e.target.value);
                    setCandidate(undefined);
                  }}
                  placeholder="选择或输入仓库路径"
                />
                <button className="button" onClick={() => void action(choosePath)}>
                  选择目录
                </button>
              </div>
            </label>
            {candidate && (
              <div className="confirmation-box">
                <strong>{kindLabels[candidate.kind]}</strong>
                <p>{candidate.path}</p>
                <small>仅收录此条目，不修改 Git 配置。</small>
              </div>
            )}
            {error && (
              <p role="alert" className="inline-error">
                {error}
              </p>
            )}
          </div>
          <footer>
            <button className="button" disabled={busy} onClick={close}>
              取消
            </button>
            <button
              className="button primary"
              disabled={busy || !pathInput}
              onClick={() =>
                void action(async () => {
                  if (!candidate) {
                    setCandidate(await api.inspect(pathInput));
                  } else {
                    await afterAdd(await api.add([candidate.path]));
                  }
                })
              }
            >
              {busy ? '处理中…' : candidate ? '确认收录' : '识别仓库'}
            </button>
          </footer>
        </Modal>
      )}

      {dialog === 'scan' && (
        <Modal
          title="扫描本地目录"
          onClose={() => {
            if (scan?.state === 'running') {
              void api.stopScan(scan.id).catch((e) => setError(errorText(e)));
            }
            close();
          }}
          busy={busy}
          wide
        >
          <div className="modal-body">
            <p className="hint">
              扫描不会自动收录。默认不跟随目录链接；停止后仍可添加已发现的仓库。
            </p>
            {scan?.state !== 'running' && (
              <>
                <label>
                  扫描根目录
                  <div className="input-action">
                    <input
                      value={pathInput}
                      onChange={(e) => setPathInput(e.target.value)}
                      placeholder="输入目录后添加，也可使用选择器"
                    />
                    <button
                      className="button"
                      onClick={() => {
                        if (pathInput && !scanRoots.includes(pathInput)) {
                          setScanRoots([...scanRoots, pathInput]);
                          setPathInput('');
                        }
                      }}
                    >
                      加入
                    </button>
                    <button
                      className="button"
                      onClick={() =>
                        void action(async () => {
                          const p = await api.chooseDirectory();
                          if (p && !scanRoots.includes(p)) setScanRoots([...scanRoots, p]);
                        })
                      }
                    >
                      选择
                    </button>
                  </div>
                </label>
                {scanRoots.map((p) => (
                  <div className="root-item" key={p}>
                    <Folder size={14} />
                    <span>{p}</span>
                    <button
                      className="icon-button"
                      onClick={() => setScanRoots(scanRoots.filter((r) => r !== p))}
                      aria-label={`移除扫描目录 ${p}`}
                    >
                      <X size={13} />
                    </button>
                  </div>
                ))}
                <div className="form-grid">
                  <label>
                    最大深度（根目录为 0）
                    <input
                      type="number"
                      min={0}
                      max={64}
                      value={depth}
                      onChange={(e) => setDepth(Number(e.target.value))}
                    />
                  </label>
                  <label>
                    排除目录名（逗号分隔）
                    <input value={excludeNames} onChange={(e) => setExcludeNames(e.target.value)} />
                  </label>
                </div>
                <label>
                  排除相对路径（每行一个，不支持通配符）
                  <textarea
                    value={excludePaths}
                    onChange={(e) => setExcludePaths(e.target.value)}
                    placeholder={'例如：archive/old-projects'}
                  />
                </label>
              </>
            )}
            {scan && (
              <>
                <div className="scan-progress">
                  <span>
                    {scan.state === 'running' ? (
                      <LoaderCircle className="spin" size={15} />
                    ) : scan.state === 'failed' ? (
                      <CircleAlert size={15} />
                    ) : (
                      <CheckCircle2 size={15} />
                    )}{' '}
                    {scan.state === 'running'
                      ? '扫描中'
                      : scan.state === 'cancelled'
                        ? '扫描已停止'
                        : scan.state === 'failed'
                          ? '扫描异常结束，已保留发现结果'
                          : '扫描完成'}
                  </span>
                  <span>
                    已访问 {scan.visited} 个目录 · 发现 {scan.found.length} 个仓库
                  </span>
                </div>
                <div className="scan-results">
                  {scan.found.map((c) => {
                    const added = entries.some((e) => e.path === c.path);
                    return (
                      <label className="scan-result" key={c.path}>
                        <input
                          type="checkbox"
                          disabled={added || scan.state === 'running'}
                          checked={checkedPaths.has(c.path)}
                          onChange={(e) =>
                            setCheckedPaths((prev) => {
                              const next = new Set(prev);
                              if (e.target.checked) next.add(c.path);
                              else next.delete(c.path);
                              return next;
                            })
                          }
                        />
                        <span>
                          <strong>{c.path}</strong>
                          <small>
                            {kindLabels[c.kind]}
                            {added ? ' · 已添加' : ''}
                          </small>
                        </span>
                      </label>
                    );
                  })}
                  {scan.found.length === 0 && (
                    <p className="muted">尚未发现仓库，可调整深度或排除项。</p>
                  )}
                </div>
                {scan.issues.length > 0 && (
                  <details>
                    <summary>{scan.issues.length} 项扫描问题</summary>
                    {scan.issues.map((issue, i) => (
                      <p className="hint" key={i}>
                        {issue}
                      </p>
                    ))}
                  </details>
                )}
              </>
            )}
            {error && (
              <p className="inline-error" role="alert">
                {error}
              </p>
            )}
          </div>
          <footer>
            <button
              className="button"
              disabled={busy || scan?.state === 'running'}
              onClick={() => {
                if (scan)
                  setCheckedPaths(
                    new Set(
                      scan.found
                        .filter((c) => !entries.some((e) => e.path === c.path))
                        .map((c) => c.path),
                    ),
                  );
              }}
            >
              选择全部新仓库
            </button>
            <div className="footer-actions">
              {scan?.state === 'running' ? (
                <button
                  className="button"
                  onClick={() =>
                    void action(async () => {
                      await api.stopScan(scan.id);
                    })
                  }
                >
                  停止扫描
                </button>
              ) : (
                <button
                  className="button"
                  disabled={busy || scanRoots.length === 0}
                  onClick={() =>
                    void action(async () => {
                      setCheckedPaths(new Set());
                      const id = await api.scan({
                        roots: scanRoots,
                        maxDepth: depth,
                        excludedNames: excludeNames
                          .split(',')
                          .map((s) => s.trim())
                          .filter(Boolean),
                        excludedPaths: excludePaths
                          .split('\n')
                          .map((s) => s.trim())
                          .filter(Boolean),
                      });
                      setScan(await api.pollScan(id));
                    })
                  }
                >
                  开始扫描
                </button>
              )}
              <button
                className="button primary"
                disabled={busy || scan?.state === 'running' || checkedPaths.size === 0}
                onClick={() =>
                  void action(async () => {
                    await afterAdd(await api.add([...checkedPaths]));
                    setScan(undefined);
                    setCheckedPaths(new Set());
                  })
                }
              >
                添加所选仓库 ({checkedPaths.size})
              </button>
            </div>
          </footer>
        </Modal>
      )}

      {dialog === 'edit' && entry && (
        <Modal title="修改仓库级配置" onClose={close} busy={busy}>
          <div className="modal-body">
            <p className="hint">
              只修改 local
              中的基础配置。作者、提交者或工作树专属覆盖保持原值；共享配置可能影响其他工作树。
            </p>
            <div className="confirmation-box">
              <strong>{entryName(entry)}</strong>
              <small>{entry.path}</small>
              {snapshot?.identity.data?.keys
                .filter(
                  (k) =>
                    k.values.some((v) => v.scope === 'worktree' || v.scope === 'command') ||
                    ((k.key.startsWith('author.') || k.key.startsWith('committer.')) &&
                      k.values.length > 0),
                )
                .map((k) => (
                  <p className="amber" key={k.key}>
                    {k.key}: {k.values.at(-1)?.value} · 存在覆盖配置
                  </p>
                ))}
            </div>
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={changeName}
                onChange={(e) => setChangeName(e.target.checked)}
              />
              修改提交姓名
            </label>
            <input
              aria-label="新的提交姓名"
              value={editName}
              onChange={(e) => {
                setEditName(e.target.value);
                setChangeName(true);
              }}
            />
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={changeEmail}
                onChange={(e) => setChangeEmail(e.target.checked)}
              />
              修改提交邮箱
            </label>
            <input
              aria-label="新的提交邮箱"
              value={editEmail}
              onChange={(e) => {
                setEditEmail(e.target.value);
                setChangeEmail(true);
              }}
            />
            <p className="hint">
              姓名、邮箱按输入内容保存，不自动修改期望规则。空字符串不能用于移除配置。
            </p>
            {error && (
              <p role="alert" className="inline-error">
                {error}
              </p>
            )}
          </div>
          <footer>
            <button className="button" disabled={busy} onClick={close}>
              取消
            </button>
            <button
              className="button primary"
              disabled={busy || (!changeName && !changeEmail)}
              onClick={() =>
                void action(() =>
                  makePreview([
                    ...(changeName ? [{ key: 'user.name' as const, value: editName }] : []),
                    ...(changeEmail ? [{ key: 'user.email' as const, value: editEmail }] : []),
                  ]),
                )
              }
            >
              预览修改
            </button>
          </footer>
        </Modal>
      )}

      {dialog === 'policy' && entry && (
        <Modal title="条目与期望身份规则" onClose={close} busy={busy}>
          <div className="modal-body">
            <p className="hint">
              这些设置只保存在 RepoLens，不修改 Git。工作树规则独立，分组不继承规则。
            </p>
            <div className="form-grid">
              <label>
                条目别名
                <input
                  value={alias}
                  onChange={(e) => setAlias(e.target.value)}
                  placeholder="使用仓库目录名"
                />
              </label>
              <label>
                用户分组
                <select value={entryGroup} onChange={(e) => setEntryGroup(e.target.value)}>
                  <option value="">未分组</option>
                  {groups.map((g) => (
                    <option key={g}>{g}</option>
                  ))}
                </select>
              </label>
            </div>
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={nameRule}
                onChange={(e) => setNameRule(e.target.checked)}
              />
              期望姓名（精确匹配）
            </label>
            <input
              aria-label="期望姓名"
              disabled={!nameRule}
              value={policyName}
              onChange={(e) => setPolicyName(e.target.value)}
            />
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={emailRule}
                onChange={(e) => setEmailRule(e.target.checked)}
              />
              期望邮箱（精确匹配）
            </label>
            <input
              aria-label="期望邮箱"
              disabled={!emailRule}
              value={policyEmail}
              onChange={(e) => setPolicyEmail(e.target.value)}
            />
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={domainRule}
                onChange={(e) => setDomainRule(e.target.checked)}
              />
              允许的完整邮箱域名
            </label>
            <input
              aria-label="允许域名"
              disabled={!domainRule}
              value={policyDomains}
              onChange={(e) => setPolicyDomains(e.target.value)}
              placeholder="company.com, example.org"
            />
            <p className="hint">
              启用的约束全部满足；域名列表匹配任意一个。域名忽略大小写，不自动包含子域名。规则同时检查基础、作者和提交者配置。
            </p>
            {error && (
              <p className="inline-error" role="alert">
                {error}
              </p>
            )}
          </div>
          <footer>
            <button className="button" onClick={close} disabled={busy}>
              取消
            </button>
            <button
              className="button primary"
              disabled={busy}
              onClick={() =>
                void action(async () => {
                  const policy: Policy = {
                    name: nameRule ? policyName : null,
                    email: emailRule ? policyEmail : null,
                    domains: domainRule ? policyDomains.split(',').map((s) => s.trim()) : [],
                  };
                  await api.update(entry.id, alias, entryGroup || null, policy);
                  setDialog(null);
                  await reload();
                  setMessage('条目和规则已保存，并重新检查配置。');
                })
              }
            >
              保存规则
            </button>
          </footer>
        </Modal>
      )}

      {dialog === 'settings' && (
        <Modal title="设置" onClose={close} busy={busy}>
          <div className="modal-body">
            <h3>本机 Git</h3>
            <p className="hint">
              使用 Git 2.46 或更高版本。修改此项会改变读取上下文，保存前验证可执行文件。
            </p>
            <label>
              Git 路径
              <input
                autoFocus
                value={gitInput}
                onChange={(e) => setGitInput(e.target.value)}
                placeholder="git 或可执行文件绝对路径"
              />
            </label>
            <small>{diagnostics?.gitVersion || diagnostics?.gitError?.message || '尚未检测'}</small>
            <h3>应用数据</h3>
            <p className="data-dir">
              <Database size={15} />
              {diagnostics?.dataDir || '由桌面应用确定'}
            </p>
            <p className="hint">
              SQLite
              保存条目、分组和规则。默认保留应用数据；移出列表或卸载不会删除仓库。当前不提供凭据管理、连接测试或自动更新。
            </p>
            <small>RepoLens {diagnostics?.version || '0.1.0-alpha.1'} · 简体中文 / 浅色</small>
            {error && (
              <p className="inline-error" role="alert">
                {error}
              </p>
            )}
          </div>
          <footer>
            <button className="button" onClick={close} disabled={busy}>
              关闭
            </button>
            <button
              className="button primary"
              disabled={busy || !desktop}
              onClick={() =>
                void action(async () => {
                  setDiagnostics(await api.setGit(gitInput));
                  await reload();
                  setMessage('Git 已验证并保存。');
                  setDialog(null);
                })
              }
            >
              验证并保存 Git
            </button>
          </footer>
        </Modal>
      )}

      {dialog === 'group' && (
        <Modal title="新建用户分组" onClose={close} busy={busy}>
          <div className="modal-body">
            <label>
              分组名称
              <input autoFocus value={groupInput} onChange={(e) => setGroupInput(e.target.value)} />
            </label>
            <p className="hint">分组只用于整理条目，不移动文件，不继承身份规则。</p>
            {error && <p className="inline-error">{error}</p>}
          </div>
          <footer>
            <button className="button" disabled={busy} onClick={close}>
              取消
            </button>
            <button
              className="button primary"
              disabled={busy || !groupInput}
              onClick={() =>
                void action(async () => {
                  await api.saveGroup(groupInput);
                  setGroups(await api.groups());
                  setDialog(null);
                })
              }
            >
              创建分组
            </button>
          </footer>
        </Modal>
      )}

      {dialog === 'remove' && entry && (
        <Modal title="移出列表" onClose={close} busy={busy}>
          <div className="modal-body">
            <p>
              从 RepoLens 移出 <strong>{entryName(entry)}</strong>？
            </p>
            <p className="hint">
              本地仓库文件完整保留，不执行 worktree remove 或
              prune。此条目的别名、分组关联和期望规则会移除。
            </p>
            {error && <p className="inline-error">{error}</p>}
          </div>
          <footer>
            <button className="button" disabled={busy} onClick={close}>
              取消
            </button>
            <button
              className="button danger"
              disabled={busy}
              onClick={() =>
                void action(async () => {
                  await api.remove(entry.id);
                  setSelected(undefined);
                  setDialog(null);
                  await reload();
                  setMessage('已移出列表，本地文件保持完整。');
                })
              }
            >
              确认移出列表
            </button>
          </footer>
        </Modal>
      )}

      {preview && (
        <Modal
          title="确认配置修改"
          onClose={() => {
            if (!busy) setPreview(undefined);
          }}
          busy={busy}
          wide
        >
          <div className="modal-body">
            <p className="hint">
              仅授权以下具体修改。预览在 5 分钟后过期；配置变化时需要重新预览。
            </p>
            <div className="confirmation-box">
              <strong>local · 目标配置文件</strong>
              <p>{preview.target}</p>
              <small>工作树：{preview.context.path}</small>
            </div>
            {preview.items.map((i) => (
              <div className="preview-item" key={i.key}>
                <code>{i.key}</code>
                <dl>
                  <dt>目标中的全部原值</dt>
                  <dd>
                    {i.old.length
                      ? i.old.map((s, j) => <div key={j}>{s || '显式空值'}</div>)
                      : '未直接设置'}
                  </dd>
                  <dt>当前生效值</dt>
                  <dd>{i.effective ?? '未设置'}</dd>
                  <dt>{i.new === null ? '执行操作' : '保存新值'}</dt>
                  <dd className="new-value">
                    {i.new === null ? '移除目标文件中此键的全部直接设置' : i.new}
                  </dd>
                  <dt>预计生效 / 继承结果</dt>
                  <dd>未知；保存后重新读取验证</dd>
                </dl>
              </div>
            ))}
            <h3>共享影响与限制</h3>
            {preview.warnings.map((w, i) => (
              <p className="hint" key={i}>
                {w}
              </p>
            ))}
            {preview.affected.map((p) => (
              <small className="affected-path" key={p}>
                {p}
              </small>
            ))}
            {error && (
              <p className="inline-error" role="alert">
                {error}
              </p>
            )}
          </div>
          <footer>
            <button
              className="button"
              disabled={busy}
              onClick={() => {
                setPreview(undefined);
                setError('');
              }}
            >
              <ArrowLeft size={14} />
              返回
            </button>
            <button
              className="button primary"
              disabled={busy}
              onClick={() =>
                void action(async () => {
                  const r = await api.apply(preview.token);
                  setWriteResult(r);
                  setPreview(undefined);
                  setDialog(null);
                  await reload();
                })
              }
            >
              {busy ? '正在保存与验证…' : '确认保存'}
            </button>
          </footer>
        </Modal>
      )}
      {writeResult && (
        <Modal
          title={resultLabels[writeResult.state] || writeResult.state}
          onClose={() => setWriteResult(undefined)}
        >
          <div className="modal-body">
            {writeResult.items.map((i) => (
              <div className="preview-item" key={i.key}>
                <strong>{i.key}</strong>
                <p>{i.message}</p>
                <Badge state={i.state}>{resultLabels[i.state]}</Badge>
              </div>
            ))}
            <p className="hint">已刷新收录条目。请结合身份检查结果查看作者、提交者及工作树覆盖。</p>
          </div>
          <footer>
            <button className="button primary" onClick={() => setWriteResult(undefined)}>
              完成
            </button>
          </footer>
        </Modal>
      )}
    </div>
  );
}
