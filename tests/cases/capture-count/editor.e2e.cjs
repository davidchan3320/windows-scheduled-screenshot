#!/usr/bin/env node
'use strict';

const fs = require('node:fs/promises');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

function loadPlaywright() {
  const candidates = [
    process.env.PLAYWRIGHT_MODULE,
    'playwright',
    '/Users/chandavid/.npm/_npx/e41f203b7505f1fb/node_modules/playwright'
  ].filter(Boolean);
  for (const candidate of candidates) {
    try { return require(candidate); }
    catch (error) {
      if (candidate === process.env.PLAYWRIGHT_MODULE ||
          (candidate !== 'playwright' && !error.message.includes('Cannot find module'))) throw error;
    }
  }
  throw new Error('Playwright not found. Set PLAYWRIGHT_MODULE to its installed module path.');
}

const { chromium } = loadPlaywright();
const root = path.resolve(__dirname, '../../..');
const editor = path.join(root, 'src/ScheduledScreenshot/config-editor.html');
const stamp = new Date().toISOString().replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z');
const artifacts = path.join(__dirname, 'artifacts', stamp);

async function importSettings(page, filePath, fileName) {
  await page.locator('#fileInput').setInputFiles(filePath);
  await page.waitForFunction(name =>
    document.getElementById('fileState').textContent.startsWith(`${name} loaded.`), fileName);
}

async function fillTimes(page, values) {
  while (await page.locator('input[data-time-index]').count() > values.length) {
    await page.locator('[data-time-action="remove"]').last().click();
  }
  while (await page.locator('input[data-time-index]').count() < values.length) {
    await page.getByRole('button', { name: 'Add time', exact: true }).click();
  }
  for (let index = 0; index < values.length; index++) {
    await page.locator(`input[data-time-index="${index}"]`).fill(values[index]);
  }
}

async function main() {
  await fs.mkdir(artifacts, { recursive: true });
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
  });
  try {
    const page = await browser.newPage({ acceptDownloads: true });
    await page.goto(pathToFileURL(editor).href);

    await page.getByRole('button', { name: 'Add task' }).click();
    await page.locator('#task-name').fill('Capture count E2E');
    await page.locator('#task-schedule-type').selectOption('fixed');
    await fillTimes(page, ['08:15:00', '12:30:45']);
    await page.locator('#task-stop-mode').selectOption('count');
    const count = page.locator('#task-stop-captureCount');
    await count.fill('2');

    for (const invalid of ['0', '-1', '1.5', '2147483648']) {
      await count.fill(invalid);
      await page.getByRole('button', { name: 'Save changes' }).click();
      const alert = page.getByRole('alert');
      if (!(await alert.isVisible()) || !(await alert.innerText()).includes('capture count must be a whole number')) {
        throw new Error(`Expected capture count ${invalid} to be rejected.`);
      }
    }

    await count.fill('2');
    await page.getByRole('button', { name: 'Save changes' }).click();
    if (await page.getByRole('alert').isVisible()) throw new Error('Valid capture-count task failed validation.');

    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Download JSON' }).click();
    const download = await downloadPromise;
    const exportedPath = path.join(artifacts, 'settings.json');
    await download.saveAs(exportedPath);
    const exported = JSON.parse(await fs.readFile(exportedPath, 'utf8'));
    const exportedTask = exported.tasks[0];
    if (exportedTask.schedule.type !== 'fixed' ||
        JSON.stringify(exportedTask.schedule.times) !== JSON.stringify(['08:15:00', '12:30:45']) ||
        exportedTask.stopCondition.mode !== 'count' || exportedTask.stopCondition.captureCount !== 2) {
      throw new Error('Export did not preserve fixed times and capture-count settings.');
    }

    await importSettings(page, exportedPath, 'settings.json');
    await page.locator(`[data-select="${exportedTask.id}"]`).click();
    if (await page.locator('#task-stop-mode').inputValue() !== 'count' ||
        await page.locator('#task-stop-captureCount').inputValue() !== '2' ||
        JSON.stringify(await page.locator('input[data-time-index]').evaluateAll(inputs => inputs.map(input => input.value))) !== JSON.stringify(['08:15:00', '12:30:45'])) {
      throw new Error('Import did not restore exported fixed times and capture-count settings.');
    }

    const legacy = JSON.parse(JSON.stringify(exported));
    delete legacy.tasks[0].stopCondition.captureCount;
    const legacyPath = path.join(artifacts, 'legacy-count-settings.json');
    await fs.writeFile(legacyPath, JSON.stringify(legacy, null, 2));
    await importSettings(page, legacyPath, 'legacy-count-settings.json');
    await page.locator(`[data-select="${exportedTask.id}"]`).click();
    await page.getByRole('button', { name: 'Save changes' }).click();
    const legacyAlert = page.getByRole('alert');
    if (!(await legacyAlert.isVisible()) || !(await legacyAlert.innerText()).includes('capture count must be a whole number')) {
      throw new Error('Imported count configuration without captureCount should be rejected on save.');
    }

    await importSettings(page, exportedPath, 'settings.json');
    await page.locator(`[data-select="${exportedTask.id}"]`).click();
    if (await page.getByRole('alert').isVisible()) throw new Error('Reloading valid export did not clear import validation errors.');

    await page.getByRole('button', { name: 'Add task' }).click();
    await page.locator('#task-name').fill('Fixed times once E2E');
    const scheduleType = page.locator('#task-schedule-type');
    const onceOption = scheduleType.locator('option[value="fixedOnce"]');
    if (await onceOption.count() !== 1 || await onceOption.innerText() !== 'Fixed local times — run once') {
      throw new Error('Fixed local times — run once schedule option is missing.');
    }
    await scheduleType.selectOption('fixedOnce');
    const onceTimes = page.getByRole('list', { name: 'Execution times', exact: true });
    if (!(await onceTimes.isVisible())) throw new Error('Run once schedule did not show the times field.');
    await fillTimes(page, ['08:15:00', '10:00:00', '12:30:45']);
    await page.getByRole('button', { name: 'Remove time 2', exact: true }).click();
    if (await page.locator('input[data-time-index]').count() !== 2) throw new Error('Removing a time did not update the list.');
    await page.getByRole('button', { name: 'Add time', exact: true }).click();
    await page.getByRole('button', { name: 'Save changes' }).click();
    if (!(await page.getByRole('alert').innerText()).includes('fixed times must use HH:mm:ss')) {
      throw new Error('Blank execution time should be rejected.');
    }
    await page.getByRole('button', { name: 'Remove time 3', exact: true }).click();
    const onceDownloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Download JSON' }).click();
    const onceDownload = await onceDownloadPromise;
    const onceExportPath = path.join(artifacts, 'fixed-once-settings.json');
    await onceDownload.saveAs(onceExportPath);
    const onceExport = JSON.parse(await fs.readFile(onceExportPath, 'utf8'));
    const onceTask = onceExport.tasks.find(task => task.name === 'Fixed times once E2E');
    if (!onceTask || onceTask.schedule.type !== 'fixedOnce' ||
        JSON.stringify(onceTask.schedule.times) !== JSON.stringify(['08:15:00', '12:30:45'])) {
      throw new Error('Export did not preserve the fixed-once schedule and selected times.');
    }

    await page.screenshot({ path: path.join(artifacts, 'editor.png'), fullPage: true });
    await page.setViewportSize({ width: 390, height: 844 });
    if (await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth)) {
      throw new Error('Time list overflows the mobile viewport.');
    }
    await page.screenshot({ path: path.join(artifacts, 'editor-mobile.png'), fullPage: true });
    process.stdout.write(`Editor E2E passed. Artifacts: ${artifacts}\n`);
  } finally {
    await browser.close();
  }
}

main().catch(error => {
  process.stderr.write(`${error.stack || error}\n`);
  process.exitCode = 1;
});
