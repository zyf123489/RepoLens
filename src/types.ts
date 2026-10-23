export interface AppError {
  code: string;
  message: string;
}
export interface Policy {
  name: string | null;
  email: string | null;
  domains: string[];
}
export interface Entry {
  id: string;
  path: string;
  alias: string;
  group: string | null;
  kind: string;
  policy: Policy;
}
export interface Context {
  path: string;
  gitDir: string;
  commonDir: string;
  kind: string;
  parent: string | null;
}
export interface Module<T> {
  state: string;
  data: T | null;
  error: AppError | null;
  readAt: number;
}
export interface ConfigValue {
  value: string;
  scope: string;
  origin: string;
}
export interface ConfigKey {
  key: string;
  values: ConfigValue[];
}
export interface Finding {
  field: string;
  value: string | null;
  state: string;
  reason: string;
}
export interface Identity {
  keys: ConfigKey[];
  findings: Finding[];
  status: string;
  checked: string;
}
export interface Head {
  branch: string | null;
  oid: string | null;
  detached: boolean;
  unborn: boolean;
}
export interface Work {
  staged: number;
  unstaged: number;
  untracked: number;
  conflicts: number;
}
export interface Commit {
  oid: string;
  date: string;
  subject: string;
  author: string;
  email: string;
}
export interface Upstream {
  state: string;
  name: string | null;
  ahead: number | null;
  behind: number | null;
}
export interface Endpoint {
  remote: string;
  usage: string;
  address: string;
  configuredAddress: string;
  protocol: string;
  host: string;
  platform: string | null;
  username: string | null;
  metadata: string[];
  limitations: string[];
  configState: string;
  verification: string;
}
export interface Snapshot {
  entryId: string;
  git: string;
  readAt: number;
  context: Module<Context>;
  head: Module<Head>;
  work: Module<Work>;
  history: Module<Commit>;
  upstream: Module<Upstream>;
  identity: Module<Identity>;
  remotes: Module<Endpoint[]>;
}
export interface Diagnostics {
  git: string;
  gitVersion: string | null;
  gitError: AppError | null;
  dataDir: string;
  version: string;
}
export interface ScanOptions {
  roots: string[];
  maxDepth: number;
  excludedNames: string[];
  excludedPaths: string[];
}
export interface ScanState {
  id: string;
  state: string;
  visited: number;
  found: Context[];
  issues: string[];
}
export interface Change {
  key: 'user.name' | 'user.email';
  value: string | null;
}
export interface Preview {
  token: string;
  entryId: string;
  target: string;
  context: Context;
  items: { key: string; old: string[]; new: string | null; effective: string | null }[];
  warnings: string[];
  affected: string[];
  createdAt: number;
}
export interface WriteResult {
  entryId: string;
  state: string;
  items: { key: string; state: string; message: string }[];
}
