'use strict';
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs/promises');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
(async () => {
  const browser = await chromium.launch({headless:true, executablePath:process.env.CHROME_PATH || undefined});
  const context = await browser.newContext({viewport:{width:1440,height:1000}});
  const page = await context.newPage();
  const errors = [], remote = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', req => { if (!req.url().startsWith(new URL(process.env.SCHEDULER_URL).origin)) remote.push(req.url()); });
  try {
    await page.goto(process.env.SCHEDULER_URL);
    assert.equal(await page.getByRole('link', {name:'Third-party notices'}).getAttribute('href'), '/licenses.txt');
    const notices = await page.request.get(new URL('/licenses.txt', process.env.SCHEDULER_URL).href);
    assert.equal(notices.status(), 200);
    const noticeText = await notices.text();
    assert(noticeText.includes('LICENSE TEXT') && noticeText.includes('Tailwind CSS 4.3.3'));
    const styles = await page.request.get(new URL('/style.css', process.env.SCHEDULER_URL).href);
    const css = await styles.text();
    assert(css.includes('tailwindcss v4.3.3') && !css.includes('@apply'), 'Serve compiled offline Tailwind CSS');
    // Capture now is a global action, available without creating any task.
    await page.route('**/api/status', async route => {
      const response = await route.fetch();
      const value = await response.json(); value.captureAllowed = true;
      await route.fulfill({json:value});
    });
    await page.reload();
    await page.waitForFunction(() => !document.getElementById('capture-now').disabled);
    let captureBody;
    await page.route('**/api/capture', route => {
      captureBody = route.request().postDataJSON();
      return route.fulfill({json:{accepted:true}});
    });
    await page.locator('#capture-now').click();
    await page.waitForFunction(() => !document.getElementById('capture-now').disabled);
    assert.deepEqual(captureBody,{});
    assert.equal(await page.locator('#tasks tr').count(),0);
    // Capture settings are collapsed so tasks stay prominent; keyboard disclosure works.
    assert.equal(await page.locator('#capture-settings').getAttribute('open'), null);
    await page.locator('#capture-settings summary').focus();
    await page.keyboard.press('Enter');
    await page.locator('#manual-folder').waitFor({state:'visible'});
    // Standalone folder drafts survive status refresh; Browse cancellation preserves them.
    await page.locator('#manual-folder').fill('manual captures/\u622a\u5716');
    assert(await page.locator('#capture-settings-draft').isVisible());
    await page.locator('#capture-settings summary').click();
    assert(await page.locator('#capture-settings-draft').isVisible(), 'Collapsed settings expose unsaved changes');
    await page.locator('#capture-settings summary').click();
    await page.waitForResponse('**/api/status');
    assert.equal(await page.locator('#manual-folder').inputValue(), 'manual captures/\u622a\u5716');
    await page.route('**/api/folder', route => route.fulfill({json:{path:null}}));
    await page.locator('#manual-browse').click();
    await page.waitForFunction(() => !document.getElementById('manual-browse').disabled);
    assert.equal(await page.locator('#manual-folder').inputValue(), 'manual captures/\u622a\u5716');
    await page.unroute('**/api/folder');
    // Capture saves the visible destination before submitting its standalone request.
    await page.locator('#capture-now').click();
    await page.waitForFunction(() => !document.getElementById('capture-now').disabled);
    await page.reload();
    await page.waitForFunction(() => document.getElementById('manual-folder').value === 'manual captures/\u622a\u5716');
    assert.match(await page.locator('#manual-output-path').textContent(), /manual captures/);
    await page.route('**/api/folder', route => route.fulfill({json:{path:'manual captures/selected'}}));
    await page.locator('#manual-browse').click();
    await page.waitForFunction(() => document.getElementById('manual-folder').value.endsWith('/selected'));
    let opened = false;
    await page.route('**/api/open-captures', route => { opened = true; return route.fulfill({json:{opened:true}}); });
    await page.locator('#open-captures').click();
    await page.waitForFunction(() => !document.getElementById('open-captures').disabled);
    assert(opened);
    await page.reload();
    await page.waitForFunction(() => document.getElementById('manual-folder').value === 'manual captures/selected');
    await page.unroute('**/api/folder');
    await page.unroute('**/api/open-captures');
    // Failed saves preserve the draft and never submit a capture to the old folder.
    captureBody = undefined;
    await page.locator('#manual-folder').fill('unsaved folder');
    await page.route('**/api/save', route => route.fulfill({status:409,json:{error:'CONFLICT: Settings changed in another window. Reload before saving.'}}));
    await page.locator('#capture-now').click();
    await page.waitForFunction(() => !document.getElementById('capture-now').disabled);
    assert.equal(captureBody, undefined);
    assert.equal(await page.locator('#manual-folder').inputValue(), 'unsaved folder');
    await page.unroute('**/api/save');
    await page.locator('#manual-folder').fill('captures');
    await page.locator('#manual-folder-save').click();
    await page.waitForFunction(() => document.getElementById('manual-folder-save').disabled && !document.getElementById('manual-folder').disabled);
    await page.unroute('**/api/status');
    await page.unroute('**/api/capture');
    await page.reload();
    // A configurable global shortcut is saved independently of tasks.
    await page.locator('#shortcut-key').selectOption('F9');
    await page.locator('#shortcut-modifiers').selectOption('ctrl+alt+shift');
    await page.waitForResponse('**/api/status');
    assert.equal(await page.locator('#shortcut-key').inputValue(), 'F9');
    await page.locator('#shortcut-save').click();
    await page.waitForFunction(() => document.getElementById('shortcut-save').disabled && !document.getElementById('shortcut-enabled').disabled);
    await page.reload();
    await page.waitForFunction(() => document.getElementById('shortcut-key').value === 'F9');
    assert.equal(await page.locator('#shortcut-modifiers').inputValue(), 'ctrl+alt+shift');
    // Registration problems are visible; the UI never advertises an inactive key.
    await page.route('**/api/status', async route => {
      const response = await route.fetch(); const value = await response.json();
      value.shortcutActive = null;
      value.shortcutError = 'Shortcut unavailable. It may be in use by another app. Choose another combination and save.';
      await route.fulfill({json:value});
    });
    await page.reload();
    await page.waitForFunction(() => document.getElementById('shortcut-status').textContent.includes('Shortcut unavailable'));
    assert.equal(await page.locator('#capture-shortcut').isVisible(), false);
    await page.unroute('**/api/status');
    await page.locator('#shortcut-enabled').uncheck();
    assert(await page.locator('#shortcut-key').isDisabled());
    await page.locator('#shortcut-save').click();
    await page.waitForFunction(() => document.getElementById('shortcut-status').textContent === 'Capture shortcut is off.');
    await page.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'capture-settings.png'),fullPage:true});
    await page.setViewportSize({width:390,height:844});
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    await page.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'capture-settings-mobile.png'),fullPage:true});
    await page.setViewportSize({width:1440,height:1000});
    await page.locator('#capture-settings summary').click();
    assert.equal(await page.locator('#manual-folder').isVisible(), false);
    await page.locator('#empty-create').click();
    assert.equal(await page.locator('#enabled').isChecked(),false);
    await page.locator('#name').fill('Workday reference');
    // Human-readable units persist as seconds; shortcuts select actual weekdays.
    await page.locator('#interval').fill('15');
    await page.locator('#interval-unit').selectOption('60');
    await page.locator('#days-work').click();
    assert.equal(await page.locator('.days input:checked').count(),5);
    assert.match(await page.locator('#summary-schedule').textContent(), /Every 15 minutes/);
    await page.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'editor-schedule.png'),fullPage:true});
    // Arrow navigation follows the accessible tab pattern.
    await page.locator('#tab-schedule').focus();
    await page.keyboard.press('ArrowRight');
    assert.equal(await page.locator('#tab-files').getAttribute('aria-selected'),'true');
    await page.locator('#folder').fill('captures/review');
    await page.locator('#template').fill('{date}_{task}_{display}');
    await page.waitForFunction(() => document.getElementById('preview').textContent.includes('Workday reference_DISPLAY1.jpg'));
    // Native dialog contract test here; smoke.py also cancels an actual Windows dialog.
    await page.route('**/api/folder', route => route.fulfill({json:{path:null}}));
    await page.locator('#browse').click();
    assert.equal(await page.locator('#folder').inputValue(),'captures/review');
    await page.unroute('**/api/folder');
    await page.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'editor.png'),fullPage:true});
    await page.setViewportSize({width:1280,height:720});
    const compactSave = await page.locator('#save').boundingBox();
    assert(compactSave.y >= 0 && compactSave.y + compactSave.height <= 720);
    await page.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'editor-compact.png'),fullPage:true});
    await page.setViewportSize({width:1440,height:1000});
    await page.locator('#save').click();
    await page.waitForFunction(() => !document.getElementById('editor').open);
    assert.equal(await page.locator('#tasks tr').count(),1);
    assert.equal(await page.locator('#tasks').getByRole('button',{name:'Capture now',exact:true}).count(),0);
    assert.match(await page.locator('#tasks tr').first().textContent(), /Every 15 minutes/);
    await page.locator('#tasks tr').first().getByRole('button',{name:'Edit',exact:true}).click();
    assert.equal(await page.locator('#interval').inputValue(),'15');
    assert.equal(await page.locator('#interval-unit').inputValue(),'60');
    // Required fields in another tab reveal the right section before saving.
    await page.locator('#tab-files').click();
    await page.locator('#folder').fill('');
    await page.locator('#tab-schedule').click();
    await page.locator('#save').click();
    assert.equal(await page.locator('#tab-files').getAttribute('aria-selected'),'true');
    assert.equal(await page.locator('#folder').evaluate(node => node === document.activeElement),true);
    await page.locator('#folder').fill('captures/review');
    // Insert a filename token at the caret without replacing the rest of the name.
    await page.locator('#template').fill('sample_');
    await page.getByRole('button',{name:'Insert monitor name token',exact:true}).click();
    assert.equal(await page.locator('#template').inputValue(),'sample_{display}');
    await page.locator('#template').fill('{date}_{task}_{display}');
    await page.locator('#save').click();
    await page.waitForFunction(() => !document.getElementById('editor').open);
    await page.getByRole('button',{name:'Duplicate',exact:true}).click();
    assert.equal(await page.locator('#enabled').isChecked(),false);
    await page.locator('#kind').selectOption('fixed');
    await page.locator('#times input').first().fill('09:30:15');
    await page.locator('#add-time').click();
    await page.locator('#times input').last().fill('17:00');
    await page.locator('#tab-limits').click();
    await page.locator('#stop').selectOption('duration');
    await page.locator('#duration').fill('2');
    await page.locator('#duration-unit').selectOption('86400');
    await page.locator('#save').click();
    await page.waitForFunction(() => document.querySelectorAll('#tasks tr').length===2);
    await page.locator('#tasks tr').last().getByRole('button',{name:'Edit',exact:true}).click();
    assert.equal(await page.locator('#times input').first().inputValue(),'09:30:15');
    assert.equal(await page.locator('#times input').last().inputValue(),'17:00:00');
    await page.locator('#tab-limits').click();
    assert.equal(await page.locator('#duration').inputValue(),'2');
    assert.equal(await page.locator('#duration-unit').inputValue(),'86400');
    await page.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'editor-limits.png'),fullPage:true});
    await page.locator('#cancel').click();
    await page.locator('#tasks tr').first().getByRole('button',{name:'Edit',exact:true}).click();
    await page.locator('#name').fill('<img src=x onerror=alert(1)>');
    await page.locator('#save').click();
    await page.waitForFunction(() => !document.getElementById('editor').open);
    assert.equal(await page.locator('img').count(),0);
    // Save from another window while a draft remains open: reject stale draft.
    await page.locator('#tasks tr').first().getByRole('button',{name:'Edit',exact:true}).click();
    const second = await context.newPage(); await second.goto(process.env.SCHEDULER_URL);
    await second.locator('#pause').click();
    await second.waitForFunction(() => document.getElementById('pause').textContent==='Resume scheduling');
    await page.locator('#save').click();
    await page.waitForFunction(() => document.getElementById('form-error').textContent.includes('Settings changed'));
    assert(await page.locator('#editor').isVisible());
    await page.locator('#cancel').click(); await second.close();
    await page.reload(); await page.waitForFunction(() => document.querySelectorAll('#tasks tr').length===2);
    await page.locator('#tasks tr').first().getByRole('button',{name:'Delete',exact:true}).click();
    await page.locator('#delete-dialog').getByRole('button',{name:'Delete task',exact:true}).click();
    await page.waitForFunction(() => document.querySelectorAll('#tasks tr').length===1);
    await page.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'task-manager.png'),fullPage:true});
    // Closing every browser page does not stop the server.
    await page.close(); const reopened = await context.newPage(); await reopened.goto(process.env.SCHEDULER_URL);
    await reopened.waitForFunction(() => document.querySelectorAll('#tasks tr').length===1);
    await reopened.setViewportSize({width:320,height:720});
    assert(await reopened.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'No horizontal overflow at 320px');
    await reopened.getByRole('button',{name:'Edit',exact:true}).click();
    assert(await reopened.locator('#editor').evaluate(node => node.scrollWidth <= node.clientWidth), 'Editor fits at 320px');
    const narrowSave = await reopened.locator('#save').boundingBox();
    assert(narrowSave.x >= 0 && narrowSave.x + narrowSave.width <= 320 && narrowSave.y + narrowSave.height <= 720);
    await reopened.locator('#cancel').click();
    await reopened.setViewportSize({width:390,height:844});
    await reopened.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'mobile.png'),fullPage:true});
    assert(await reopened.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    await reopened.getByRole('button',{name:'Edit',exact:true}).click();
    const saveBox = await reopened.locator('#save').boundingBox();
    assert(saveBox.y >= 0 && saveBox.y + saveBox.height <= 844, 'Save stays in the viewport');
    await reopened.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'editor-mobile.png'),fullPage:false});
    // Removing the last time provides a readable error, not a failed request.
    while (await reopened.getByRole('button',{name:/Remove capture time/}).count()) {
      await reopened.getByRole('button',{name:/Remove capture time/}).first().click();
    }
    await reopened.locator('#save').click();
    assert.match(await reopened.locator('#form-error').textContent(), /Add at least one capture time/);
    // Escape protects a changed draft. Declining discard keeps the editor open.
    reopened.once('dialog', dialog => dialog.dismiss());
    await reopened.keyboard.press('Escape');
    assert(await reopened.locator('#editor').isVisible());
    reopened.once('dialog', dialog => dialog.accept());
    await reopened.locator('#cancel').click();
    assert.equal(await reopened.locator('#editor').isVisible(),false);
    // Language lives in portable settings, rather than a browser origin/port.
    await reopened.locator('#language').selectOption('zh-Hant');
    await reopened.waitForFunction(() => document.documentElement.lang === 'zh-Hant');
    assert.match(await reopened.locator('#tasks-title').textContent(), /擷取工作/);
    assert.equal(await reopened.getByRole('button',{name:'開啟資料夾',exact:true}).count(),1);
    await reopened.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'zh-Hant-mobile.png'),fullPage:true});
    assert(await reopened.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    await reopened.setViewportSize({width:1440,height:1000});
    await reopened.getByRole('button',{name:'編輯',exact:true}).click();
    assert.equal(await reopened.locator('#editor-title').textContent(),'編輯工作');
    assert.equal(await reopened.locator('#tab-files').textContent(),'檔案與影像');
    assert.equal(await reopened.locator('#times input').first().inputValue(),'09:30:15');
    assert.equal(await reopened.locator('.days input:checked').count(),5);
    await reopened.locator('#name').fill('工作日截圖');
    await reopened.locator('#tab-files').click();
    assert.equal(await reopened.locator('#template').inputValue(),'{date}_{task}_{display}');
    await reopened.locator('#template').fill('../bad');
    await reopened.locator('#save').click();
    await reopened.waitForFunction(() => document.getElementById('form-error').textContent.includes('無效字元'));
    await reopened.locator('#template').fill('{date}_{task}_{display}');
    await reopened.locator('#save').click();
    await reopened.waitForFunction(() => !document.getElementById('editor').open);
    await reopened.reload();
    await reopened.waitForFunction(() => document.documentElement.lang === 'zh-Hant');
    assert.equal(await reopened.locator('#language').inputValue(),'zh-Hant');
    assert.equal(await reopened.locator('.task-title').textContent(),'工作日截圖');
    await reopened.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'zh-Hant-tasks.png'),fullPage:true});
    await reopened.getByRole('button',{name:'編輯',exact:true}).click();
    await reopened.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'zh-Hant-editor.png'),fullPage:true});
    await reopened.locator('#cancel').click();
    const fresh = await browser.newContext();
    const freshPage = await fresh.newPage();
    await freshPage.goto(process.env.SCHEDULER_URL);
    await freshPage.waitForFunction(() => document.documentElement.lang === 'zh-Hant');
    assert.equal(await freshPage.locator('.task-title').textContent(),'工作日截圖');
    await freshPage.locator('#language').selectOption('en');
    await freshPage.waitForFunction(() => document.documentElement.lang === 'en');
    assert.equal(await freshPage.getByRole('button',{name:'Open folder',exact:true}).count(),1);
    assert.equal(await freshPage.locator('.task-title').textContent(),'工作日截圖');
    await fresh.close();
    // Export downloads only the saved configuration, with Unicode intact.
    await reopened.reload();
    await reopened.waitForFunction(() => document.documentElement.lang === 'en');
    const downloadEvent = reopened.waitForEvent('download');
    await reopened.getByRole('button', {name:'Export config', exact:true}).click();
    const download = await downloadEvent;
    assert.match(download.suggestedFilename(), /^scheduled-screenshot-config-\d{4}-\d{2}-\d{2}\.json$/);
    const exported = JSON.parse(await fs.readFile(await download.path(), 'utf8'));
    assert.equal(exported.schemaVersion, 2);
    assert.equal(exported.tasks[0].name, '工作日截圖');
    assert.equal(exported.runtime, undefined);
    assert.equal(exported.tasks[0].progress, undefined);
    const upload = async contents => {
      const chooserEvent = reopened.waitForEvent('filechooser');
      await reopened.getByRole('button', {name:/^(Import config|匯入設定)$/}).click();
      const chooser = await chooserEvent;
      await chooser.setFiles({name:'設定.json', mimeType:'application/json', buffer:Buffer.from(contents)});
    };
    const readSettings = async () => {
      const response = await context.request.post(new URL('/api/export', process.env.SCHEDULER_URL).href, {
        headers: {'Origin':new URL(process.env.SCHEDULER_URL).origin, 'X-App-Token':new URLSearchParams(new URL(process.env.SCHEDULER_URL).hash.slice(1)).get('token')}, data:{}
      });
      assert.equal(response.status(), 200);
      return response.json();
    };
    // Cancellation, including selecting the same file again, changes nothing.
    await upload(JSON.stringify(exported));
    await reopened.locator('#import-dialog').waitFor({state:'visible'});
    assert.match(await reopened.locator('#import-summary').textContent(), /1 task\(s\).*設定.json/);
    await reopened.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'import-config.png'),fullPage:true});
    await reopened.locator('#import-cancel').click();
    assert.deepEqual(await readSettings(), exported);
    for (const [contents, message] of [
      ['{broken', 'valid JSON'],
      ['{}', 'schema version 2'],
      [JSON.stringify({...exported, tasks:[...exported.tasks, ...exported.tasks]}), 'unique UUIDs'],
      [' '.repeat(1048577), '1 MB']
    ]) {
      await upload(contents);
      await reopened.waitForFunction(text => document.getElementById('notice').textContent.includes(text), message);
      assert.equal(await reopened.locator('#import-dialog').isVisible(), false);
      assert.deepEqual(await readSettings(), exported);
    }
    // Another window's edit after preview must not be overwritten.
    await upload(JSON.stringify(exported));
    await reopened.locator('#import-dialog').waitFor({state:'visible'});
    const other = await context.newPage();
    await other.goto(process.env.SCHEDULER_URL);
    await other.locator('#language').selectOption('zh-Hant');
    await other.waitForFunction(() => document.documentElement.lang === 'zh-Hant');
    await reopened.locator('#import-confirm').click();
    await reopened.waitForFunction(() => document.getElementById('import-error').textContent.includes('Settings changed'));
    assert.equal((await readSettings()).language, 'zh-Hant');
    await reopened.locator('#import-cancel').click();
    await other.close();
    await reopened.reload();
    await reopened.waitForFunction(() => document.documentElement.lang === 'zh-Hant');
    assert.equal(await reopened.locator('#export-config').textContent(), '匯出設定');
    // UTF-8 BOM and a revision from another installation are accepted.
    const replacement = {...exported, revision:99999, paused:false};
    await upload('\uFEFF' + JSON.stringify(replacement));
    await reopened.locator('#import-dialog').waitFor({state:'visible'});
    assert.equal(await reopened.locator('#import-title').textContent(), '取代目前設定？');
    await reopened.setViewportSize({width:390,height:844});
    await reopened.screenshot({path:path.join(process.env.SMOKE_OUTPUT,'import-config-zh-mobile.png'),fullPage:true});
    assert(await reopened.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    await reopened.locator('#import-confirm').click();
    await reopened.waitForFunction(() => !document.getElementById('import-dialog').open);
    assert.match(await reopened.locator('#config-result').textContent(), /Configuration imported/);
    let restored = await readSettings();
    assert.deepEqual(restored.tasks, exported.tasks);
    assert.equal(restored.language, 'en');
    assert.equal(restored.paused, true);
    assert.equal(restored.revision, exported.revision + 2);
    await reopened.reload();
    await reopened.waitForFunction(() => document.querySelectorAll('#tasks tr').length === 1);
    // Empty configurations explicitly clear tasks; round-trip restores them.
    await upload(JSON.stringify({schemaVersion:2, tasks:[]}));
    await reopened.locator('#import-confirm').click();
    await reopened.waitForFunction(() => !document.getElementById('import-dialog').open);
    assert.equal(await reopened.locator('#tasks tr').count(), 0);
    await upload(JSON.stringify(exported));
    await reopened.locator('#import-confirm').click();
    await reopened.waitForFunction(() => !document.getElementById('import-dialog').open);
    restored = await readSettings();
    assert.deepEqual(restored.tasks, exported.tasks);
    // Restart protects drafts, reports errors, and stops polling the old process.
    await reopened.locator('#capture-settings').evaluate(node => { node.open = true; });
    await reopened.locator('#manual-folder').fill('unsaved restart folder');
    let restartCalls = 0;
    await reopened.route('**/api/restart', route => {
      restartCalls++;
      return route.fulfill({status:400, json:{error:'Restart test failure'}});
    });
    await reopened.locator('#restart').click();
    assert.match(await reopened.locator('#notice').textContent(), /Save or discard/);
    assert.equal(restartCalls, 0);
    await reopened.reload();
    await reopened.waitForResponse('**/api/status');
    await reopened.locator('#restart').click();
    await reopened.waitForFunction(() => document.getElementById('notice').textContent.includes('Restart test failure'));
    assert.equal(await reopened.locator('#restart').isDisabled(), false);
    await reopened.unroute('**/api/restart');
    await reopened.route('**/api/restart', route => {
      assert.deepEqual(route.request().postDataJSON(), {});
      return route.fulfill({json:{accepted:true}});
    });
    await reopened.locator('#restart').click();
    await reopened.locator('#restart-result').waitFor({state:'visible'});
    assert.equal(await reopened.locator('#restart').isDisabled(), true);
    let pollsAfterRestart = 0;
    reopened.on('request', request => { if (request.url().endsWith('/api/status')) pollsAfterRestart++; });
    await reopened.waitForTimeout(3500);
    assert.equal(pollsAfterRestart, 0);
    assert.deepEqual(errors,[]); assert.deepEqual(remote,[]);
    console.log('Browser smoke passed.');
  } finally { await browser.close(); }
})().catch(e=>{console.error(e);process.exitCode=1;});
