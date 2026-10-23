import { chromium } from 'playwright-core';
import { spawn, execFileSync } from 'node:child_process';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';

// Windows-only real WebView2/IPC smoke. No mock invoke or user's Git config.
const root = path.resolve('artifacts', `smoke-${Date.now()}`);
const repo = path.join(root, '中文 sample');
await mkdir(repo, { recursive: true });
const global = path.join(root, 'empty-global');
await writeFile(global, '');
const env = { ...process.env, GIT_CONFIG_GLOBAL: global, GIT_CONFIG_NOSYSTEM: '1' };
delete env.GIT_CONFIG_COUNT;
delete env.GIT_CONFIG_PARAMETERS;
function git(...args) {
  return execFileSync('git', args, { cwd: repo, env, windowsHide: true, encoding: 'utf8' }).trim();
}
git('init', '--initial-branch=main');
git('config', 'user.name', 'Smoke User');
git('config', 'user.email', 'before@example.com');
git(
  'config',
  'remote.origin.url',
  'https://login:private-sentinel@example.com/group/repo?token=private-sentinel',
);
const config = path.join(repo, '.git', 'config');
const original = await readFile(config, 'utf8');
const port = 19223;
const app = spawn(path.resolve('src-tauri/target/release/repolens.exe'), [], {
  env: {
    ...env,
    WEBVIEW2_USER_DATA_FOLDER: path.join(root, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
  },
  stdio: 'ignore',
  windowsHide: true,
});
let appExit;
app.on('exit', (code, signal) => {
  appExit = { code, signal };
});
let browser, page, entryId;
const steps = [];
function passed(step) {
  steps.push(step);
  console.log(`PASS ${step}`);
}
try {
  const deadline = Date.now() + 30_000;
  while (true) {
    if (appExit) throw new Error(`Desktop exited during startup: ${JSON.stringify(appExit)}`);
    try {
      const r = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (r.ok) break;
    } catch {
      /* child startup */
    }
    if (Date.now() > deadline) throw new Error('WebView2 debugging endpoint not ready');
    await new Promise((r) => setTimeout(r, 250));
  }
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  page = browser.contexts()[0].pages()[0];
  await page.getByRole('button', { name: '添加仓库', exact: true }).waitFor();
  const jsErrors = [];
  page.on('pageerror', (error) => jsErrors.push(error.message));
  await page.getByRole('button', { name: '扫描目录', exact: true }).first().click();
  const scanDialog = page.getByRole('dialog', { name: '扫描本地目录', exact: true });
  await scanDialog.getByPlaceholder('输入目录后添加，也可使用选择器').fill(root);
  await scanDialog.getByRole('button', { name: '加入', exact: true }).click();
  await scanDialog.getByLabel('最大深度（根目录为 0）').fill('1');
  await scanDialog.getByRole('button', { name: '开始扫描', exact: true }).click();
  await scanDialog.getByText('扫描完成', { exact: true }).waitFor();
  await scanDialog.locator('.scan-result').filter({ hasText: '中文 sample' }).waitFor();
  const beforeImport = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('list_entries'));
  assert.ok(!beforeImport.some((e) => e.path.includes(path.basename(root))));
  await scanDialog.getByRole('button', { name: '关闭', exact: true }).click();
  passed('scan finds fixture without automatically collecting it');
  await page.getByRole('button', { name: '添加仓库', exact: true }).click();
  await page.getByPlaceholder('选择或输入仓库路径').fill(repo);
  await page.getByRole('button', { name: '识别仓库', exact: true }).click();
  await page.getByRole('button', { name: '确认收录', exact: true }).click();
  const row = page.getByRole('row').filter({ hasText: '中文 sample' });
  await row.waitFor();
  await row.click();
  await page.getByRole('tab', { name: '提交身份', exact: true }).click();
  await page.getByText('before@example.com', { exact: true }).first().waitFor();
  assert.equal(await readFile(config, 'utf8'), original);
  passed('actual desktop import/read uses IPC and does not write Git configuration');
  const entries = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('list_entries'));
  entryId = entries.find(
    (e) => e.path.replaceAll('\\', '/').toLowerCase() === repo.replaceAll('\\', '/').toLowerCase(),
  )?.id;
  assert.ok(entryId, 'fixture entry found');
  await page.screenshot({ path: path.join(root, 'identity.png') });
  await page.getByRole('tab', { name: '认证概览', exact: true }).click();
  await page.getByText('https://example.com/group/repo', { exact: true }).first().waitFor();
  assert.ok(!(await page.locator('body').innerText()).includes('private-sentinel'));
  passed('authentication endpoint is redacted before reaching the interface');
  await page.getByRole('tab', { name: '提交身份', exact: true }).click();
  await page.getByRole('button', { name: '设置期望身份规则', exact: true }).click();
  await page.getByRole('checkbox', { name: '期望邮箱（精确匹配）', exact: true }).check();
  await page.getByRole('textbox', { name: '期望邮箱', exact: true }).fill('after@example.com');
  await page.getByRole('button', { name: '保存规则', exact: true }).click();
  await row.getByText('配置不匹配', { exact: true }).waitFor();
  assert.equal(await readFile(config, 'utf8'), original);
  passed('entry policy is saved in application data and detects a mismatch without editing Git');
  // Locate controls by their user-facing names; keep this test sensitive to actual UI.
  await page.getByRole('button', { name: '修改仓库级配置', exact: true }).click();
  await page.getByLabel('修改提交邮箱').check();
  await page.getByLabel('新的提交邮箱').fill('after@example.com');
  await page.getByRole('button', { name: '预览修改', exact: true }).click();
  const confirmation = page.getByRole('dialog', { name: '确认配置修改', exact: true });
  await confirmation.waitFor();
  assert.equal(await readFile(config, 'utf8'), original);
  await confirmation.screenshot({ path: path.join(root, 'preview.png') });
  passed('preview displays exact change while target remains untouched');
  await confirmation.getByRole('button', { name: '返回', exact: true }).click();
  assert.equal(await readFile(config, 'utf8'), original);
  await page.getByRole('button', { name: '预览修改', exact: true }).click();
  await confirmation.waitFor();
  git('config', 'user.email', 'external@example.com');
  await confirmation.getByRole('button', { name: '确认保存', exact: true }).click();
  await confirmation.getByRole('alert').waitFor();
  assert.equal(git('config', '--local', 'user.email'), 'external@example.com');
  await confirmation.getByRole('button', { name: '返回', exact: true }).click();
  assert.equal(await page.getByLabel('新的提交邮箱').inputValue(), 'after@example.com');
  await page.getByRole('button', { name: '预览修改', exact: true }).click();
  await confirmation.waitFor();
  passed('cancel is read-only and stale preview rejects external changes while preserving input');
  await confirmation.getByRole('button', { name: '确认保存', exact: true }).click();
  await page.getByRole('button', { name: '完成', exact: true }).waitFor();
  assert.equal(git('config', '--local', 'user.email'), 'after@example.com');
  assert.equal(git('config', '--local', 'user.name'), 'Smoke User');
  await page.getByRole('button', { name: '完成', exact: true }).click();
  await row.getByText('配置检查通过', { exact: true }).waitFor();
  passed('confirmed save writes only the selected local identity key and refreshes');
  assert.deepEqual(jsErrors, []);
  await page.screenshot({ path: path.join(root, 'saved.png') });
  await writeFile(
    path.join(root, 'result.json'),
    JSON.stringify({ passed: true, appPid: app.pid, steps, jsErrors }, null, 2),
  );
  console.log(`Artifacts: ${root}`);
} catch (error) {
  if (page) {
    await page.screenshot({ path: path.join(root, 'failure.png') }).catch(() => {});
    await writeFile(path.join(root, 'failure.txt'), await page.locator('body').innerText()).catch(
      () => {},
    );
  }
  throw error;
} finally {
  if (page && entryId)
    await page
      .evaluate((id) => window.__TAURI_INTERNALS__.invoke('remove_entry', { id }), entryId)
      .catch(() => {});
  if (browser) await browser.close().catch(() => {});
  app.kill();
}
