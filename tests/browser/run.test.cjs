'use strict';
const assert = require('node:assert/strict');
const {EventEmitter} = require('node:events');
const fs = require('node:fs');
const path = require('node:path');
const {test} = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, 'run.cjs'), 'utf8');

function startRunner() {
  const children = [];
  const runnerProcess = {execPath: process.execPath, env: {}};
  let shutdownRequested = false;
  const spawn = () => {
    const child = new EventEmitter();
    child.stdout = new EventEmitter();
    child.stdout.setEncoding = () => {};
    child.stdin = {end: () => { shutdownRequested = true; }};
    child.kill = () => {};
    children.push(child);
    return child;
  };
  vm.runInNewContext(source, {
    __dirname,
    process: runnerProcess,
    require: name => {
      if (name === 'node:child_process') return {spawn};
      if (name === 'node:fs') return {mkdirSync: () => {}};
      return require(name);
    },
    setTimeout: () => 1,
    clearTimeout: () => {},
  });
  return {children, runnerProcess, shutdownRequested: () => shutdownRequested};
}

for (const scenario of [
  {name: 'successful suite and host', suite: 0, host: 0, expected: 0},
  {name: 'failed suite', suite: 2, host: 0, expected: 1},
  {name: 'signal-terminated suite', suite: null, host: 0, expected: 1},
  {name: 'failed host', suite: 0, host: 2, expected: 1},
  {name: 'signal-terminated host', suite: 0, host: null, expected: 1},
]) {
  for (const hostFirst of [false, true]) {
    test(`${scenario.name}; host exits ${hostFirst ? 'first' : 'last'}`, () => {
      const runner = startRunner();
      const host = runner.children[0];
      host.stdout.emit('data', 'http://127.0.0.1:1234/#token=test');
      assert.equal(runner.children.length, 2);
      const suite = runner.children[1];
      const exit = (child, code) => child.emit('exit', code, code === null ? 'SIGTERM' : null);
      if (hostFirst) exit(host, scenario.host);
      exit(suite, scenario.suite);
      if (!hostFirst) exit(host, scenario.host);
      assert.equal(runner.runnerProcess.exitCode ?? 0, scenario.expected);
      assert.equal(runner.shutdownRequested(), true);
    });
  }
}

test('host exits before becoming ready', () => {
  const runner = startRunner();
  runner.children[0].emit('exit', 0, null);
  assert.equal(runner.runnerProcess.exitCode, 1);
  assert.equal(runner.children.length, 1);
});
