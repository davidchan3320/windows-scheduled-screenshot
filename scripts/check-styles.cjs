'use strict';
// Keep cargo-only builds usable: the generated offline stylesheet is committed.
const {mkdtempSync, readFileSync, existsSync, unlinkSync, rmdirSync} = require('node:fs');
const {tmpdir} = require('node:os');
const path = require('node:path');
const {spawnSync} = require('node:child_process');
const root = path.resolve(__dirname, '..');
const temporary = mkdtempSync(path.join(tmpdir(), 'screenshot-css-'));
try {
  const output = path.join(temporary, 'style.css');
  const cli = path.join(root, 'node_modules/@tailwindcss/cli/dist/index.mjs');
  const result = spawnSync(process.execPath, [cli, '-i', 'ui/tailwind.css', '-o', output, '--minify'], {cwd:root, stdio:'inherit'});
  if (result.status !== 0) throw new Error('Tailwind compilation failed. Run npm ci first.');
  if (!readFileSync(output).equals(readFileSync(path.join(root, 'ui/style.css')))) {
    throw new Error('ui/style.css is out of date. Run npm run build:css and include the generated file.');
  }
  console.log('Bundled Tailwind stylesheet is current.');
} finally {
  const output = path.join(temporary, 'style.css');
  if (existsSync(output)) unlinkSync(output);
  rmdirSync(temporary);
}
