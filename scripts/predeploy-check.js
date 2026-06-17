#!/usr/bin/env node
/**
 * predeploy-check.js — production 部署前置守卫（T-191 / 堵 RED-009 的本地半机械层）
 *
 * 背景：production 用本地手动 `flyctl deploy`，不经 CI。Cedar task-gate 只 gate PR；
 * 直接 deploy 是零治理通过的（CASE-001 即此）。本脚本在 deploy 前做本地校验。
 *
 * 用法（部署前必跑）：
 *   node scripts/predeploy-check.js && flyctl deploy
 *
 * 诚实定级：⚠️ 半机械（honor-based）。它拦的是"手滑 / 过度尽责"的越界发版；
 * 真要绕过直接调 flyctl 仍可——真机械层是 main 分支保护(require PR + task-gate) + 未来的 CI 部署。
 * 见 docs/retro-cases/redteam-catalog.html RED-009。
 *
 * 校验项（任一失败即拒绝部署）：
 *   1. 当前在 main 分支（production 只从 main 发版）
 *   2. 工作树干净（无未提交改动）
 *   3. 本地 HEAD == origin/main（已 fetch；不许拿未推送/落后的本地状态发版）
 *   4. HEAD 提交可溯源到任务令（commit message 含 Task: T-xxx）
 *   5.（best-effort）GitHub 上该 commit 的 task-gate check 已通过（gh 可用时）
 */
const { execFileSync } = require('child_process');

function git(args) {
  return execFileSync('git', args, { encoding: 'utf8' }).trim();
}
function tryRun(cmd, args) {
  try { return execFileSync(cmd, args, { encoding: 'utf8' }).trim(); }
  catch { return null; }
}
function fail(msg) {
  console.error(`\n⛔ 部署前置校验失败：${msg}`);
  console.error('   production 只从 main、经任务令、已推送、过 task-gate 才能发。');
  console.error('   如确属合理例外，自行确认后直接 flyctl deploy（你在绕过本守卫，请知悉）。\n');
  process.exit(1);
}

// 1. 分支
const branch = git(['rev-parse', '--abbrev-ref', 'HEAD']);
if (branch !== 'main') fail(`当前在 '${branch}'，production 只从 main 发版`);

// 2. 工作树干净（只看已跟踪改动；忽略未跟踪文件，如有意保留的本地草稿）
if (git(['status', '--porcelain', '--untracked-files=no'])) fail('工作树有未提交的已跟踪改动');

// 3. 与 origin/main 同步
try { execFileSync('git', ['fetch', 'origin', 'main'], { stdio: 'ignore' }); }
catch { console.error('⚠️  git fetch 失败，跳过同步校验（网络？）'); }
const head = git(['rev-parse', 'HEAD']);
const remote = tryRun('git', ['rev-parse', 'origin/main']);
if (remote && head !== remote) fail(`本地 HEAD(${head.slice(0,8)}) ≠ origin/main(${remote.slice(0,8)})，先 push/对齐`);

// 4. 可溯源到任务令
const msg = git(['log', '-1', '--pretty=%B']);
if (!/(Task:\s*T-\d+|T-\d+)/.test(msg)) fail('HEAD 提交无任务令引用（Task: T-xxx）');

// 5. best-effort：GitHub task-gate 结论
const gh = tryRun('gh', ['api', `repos/Boris-hbx/ergouWeb/commits/${head}/check-runs`, '--jq',
  '[.check_runs[] | select(.name=="task-gate")] | (if length==0 then "none" else (.[0].conclusion // "pending") end)']);
if (gh === null) {
  console.error('⚠️  gh 不可用，跳过远端 task-gate 校验（仅本地校验通过）');
} else if (gh === 'none') {
  console.error('⚠️  该 commit 未见 task-gate check（可能 PR 尚未跑过）；本地校验已通过，请自行确认');
} else if (gh !== 'success') {
  fail(`GitHub task-gate 结论为 '${gh}'，未通过`);
} else {
  console.log('✅ GitHub task-gate: success');
}

console.log(`✅ 部署前置校验通过（main @ ${head.slice(0,8)}）。可 flyctl deploy。`);
