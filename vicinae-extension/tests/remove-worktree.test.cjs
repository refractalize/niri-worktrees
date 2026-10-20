const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { test } = require('node:test');
const vm = require('node:vm');
const ts = require('typescript');
const code = ts.transpileModule(readFileSync(`${__dirname}/../src/utils.ts`, 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText;
const protectedError = JSON.stringify({ error: {
  code: 'worktree_protected_paths', message: 'Worktree contains protected paths: mise.local.toml',
} });

async function removeWithError(error) {
  const events = [];
  const api = {
    confirmAlert: async () => true,
    Alert: { ActionStyle: { Destructive: 'destructive' } },
    Toast: { Style: { Animated: 'animated', Failure: 'failure' } },
    showToast: async options => {
      events.push(options);
      return { hide: async () => events.push('hide') };
    },
  };
  const exports = {};
  vm.runInNewContext(code, { exports, Error, require: name => {
    if (name === '@vicinae/api') return api;
    if (name === 'child_process') return { exec: (_command, callback) => callback(error, '', error.stderr ?? '') };
    return require(name);
  } });
  assert.equal(await exports.removeWorktree('/test/worktree'), false);
  assert.equal(events[0].style, 'animated');
  assert.equal(events[1], 'hide');
  assert.equal(events[2].style, 'failure');
  return events[2];
}

for (const [name, error] of [
  ['JSON stderr', Object.assign(new Error('exit 1'), { stderr: protectedError })],
  ['buffer stderr', Object.assign(new Error('exit 1'), { stderr: Buffer.from(protectedError) })],
  ['surrounding output', Object.assign(new Error('exit 1'), { stderr: `teardown output\n${protectedError}\ntrailer` })],
  ['JSON in error message', new Error(`Command failed: niri-worktrees\n${protectedError}`)],
]) {
  test(name + ' renders the protected-file title and explanation together', async () => {
    const toast = await removeWithError(error);
    assert.equal(toast.title, 'Worktree contains protected files');
    assert.equal(toast.message, 'Worktree contains protected paths: mise.local.toml');
  });
}

test('plain stderr remains visible', async () => {
  const toast = await removeWithError(Object.assign(new Error('exit 1'), { stderr: 'Permission denied' }));
  assert.equal(toast.message, 'Permission denied');
});
test('unknown structured errors retain their explanation', async () => {
  const toast = await removeWithError(Object.assign(new Error('exit 1'), { stderr: JSON.stringify({ error: { code: 'new_code', message: 'A specific explanation' } }) }));
  assert.equal(toast.title, 'Failed to remove worktree');
  assert.equal(toast.message, 'A specific explanation');
});
