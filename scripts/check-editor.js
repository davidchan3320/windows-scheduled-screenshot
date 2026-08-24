const fs = require('fs');
const path = require('path');

const editorPath = path.resolve(__dirname, '..', 'src', 'ScheduledScreenshot', 'config-editor.html');
const html = fs.readFileSync(editorPath, 'utf8');
const matches = [...html.matchAll(/<script>([\s\S]*?)<\/script>/gi)];

if (matches.length !== 1) {
  throw new Error(`Expected one inline script in config-editor.html, found ${matches.length}.`);
}

new Function(matches[0][1]);

for (const required of ['Content-Security-Policy', 'showOpenFilePicker', 'fileNameTemplate', 'intervalSeconds']) {
  if (!html.includes(required)) {
    throw new Error(`config-editor.html is missing required content: ${required}`);
  }
}

process.stdout.write('config-editor.html syntax and required-content checks passed.\n');
