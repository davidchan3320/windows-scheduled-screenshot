'use strict';
const {spawn} = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const root = path.resolve(__dirname, '../..');
const output = path.join(root, 'artifacts', 'browser-ci');
fs.mkdirSync(output, {recursive:true});
const host = spawn(path.join(root, 'target/x86_64-pc-windows-msvc/debug/examples/ui-test-host.exe'), [], {stdio:['pipe','pipe','inherit'], windowsHide:true});
let started = false;
const timer = setTimeout(() => { host.kill(); throw new Error('UI test host did not start'); }, 20000);
host.stdout.setEncoding('utf8');
host.stdout.on('data', data => {
  if (started || !data.includes('#token=')) return;
  started = true; clearTimeout(timer);
  const test = spawn(process.execPath, [path.join(__dirname, 'suite.cjs')], {stdio:'inherit', windowsHide:true, env:{...process.env, SCHEDULER_URL:data.trim(), SMOKE_OUTPUT:output}});
  test.on('exit', code => { host.stdin.end('\n'); process.exitCode = code || 0; });
});
host.on('exit', code => { clearTimeout(timer); if (!started || code) process.exitCode = 1; });
