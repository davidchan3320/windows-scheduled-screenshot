'use strict';
(async () => {
await I18n.load();
const t = I18n.t;
const $ = id => document.getElementById(id);
let token = new URLSearchParams(location.hash.slice(1)).get('token');
if (token) {
  sessionStorage.setItem('schedulerToken', token);
  history.replaceState(null, '', '/');
}
token ||= sessionStorage.getItem('schedulerToken');
let snapshot, editing, editRevision, initialDraft, previewTimer, deleteId, refreshing = false;
let taskListSignature;
let manualRequestPending = false;
let manualFolderDirty = false, manualFolderRevision, manualFolderSaving;
let pendingImport, importing = false;
let shortcutDirty = false, shortcutRevision, shortcutSaving = false;
let restarting = false;
$('capture-settings').open = sessionStorage.getItem('captureSettingsOpen') === 'true';
$('capture-settings').addEventListener('toggle', () => sessionStorage.setItem('captureSettingsOpen', String($('capture-settings').open)));
for (const key of [...'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789', ...Array.from({length:11}, (_, index) => 'F' + (index + 1))]) {
  const option = document.createElement('option'); option.value = option.textContent = key; $('shortcut-key').append(option);
}
const dayNames = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
function el(tag, text, cls) {
  const node = document.createElement(tag);
  if (text != null) node.textContent = text;
  if (cls) node.className = cls;
  return node;
}
dayNames.forEach(day => {
  const label = el('label');
  const input = el('input');
  input.type = 'checkbox';
  input.value = day;
  label.append(input, el('span', t(day)));
  document.querySelector('.days').append(label);
});
const tokenDescriptions = {
  timestamp: 'Date and time', date: 'Date', time: 'Time', task: 'Task name',
  taskId: 'Full task ID', taskId8: 'Short task ID', display: 'Monitor name', displayIndex: 'Monitor number'
};
Object.entries(tokenDescriptions).forEach(([name, description]) => {
  const button = el('button', '{' + name + '}');
  button.type = 'button';
  button.dataset.tokenDescription = description;
  button.title = t(description);
  button.setAttribute('aria-label', t('Insert ' + description.toLowerCase() + ' token'));
  button.onclick = () => {
    const input = $('template');
    input.setRangeText(button.textContent, input.selectionStart, input.selectionEnd, 'end');
    input.focus();
    draftChanged();
  };
  $('tokens').append(button);
});
function error(message, form = false, localized = false) {
  const node = $(form ? 'form-error' : 'notice');
  node.dataset.message = message || '';
  node.textContent = localized ? message : I18n.message(message);
  node.hidden = !message;
}
async function api(path, body = {}) {
  const response = await fetch('/api/' + path, {
    method: 'POST', headers: {'Content-Type': 'application/json', 'X-App-Token': token || ''},
    body: JSON.stringify(body), cache: 'no-store'
  });
  const value = await response.json();
  if (!response.ok) throw new Error(value.error || t('The request could not be completed.'));
  return value;
}
function timeLabel(value) { return value ? new Date(value).toLocaleString(I18n.locale) : t('No upcoming run'); }
function clockLabel(value) { return value?.endsWith(':00') && value.length === 8 ? value.slice(0, 5) : value; }
function durationLabel(seconds) {
  for (const [unit, label] of [[86400, 'day'], [3600, 'hour'], [60, 'minute'], [1, 'second']]) {
    if (seconds >= unit && seconds % unit === 0) {
      const amount = seconds / unit;
      return t('{count} ' + label + (amount === 1 ? '' : 's'), {count: amount});
    }
  }
  return t('Choose an interval');
}
function daysLabel(days) {
  if (days.length === 7) return t('Every day');
  if (days.length === 5 && dayNames.slice(0, 5).every(day => days.includes(day))) return t('Monday to Friday');
  if (days.length === 2 && days.includes('Sat') && days.includes('Sun')) return t('Weekends');
  return days.map(day => t(day)).join(', ') || t('Choose at least one day');
}
function scheduleLabel(task) {
  const schedule = task.schedule;
  if (schedule.type === 'once') return schedule.atLocal ? new Date(schedule.atLocal).toLocaleString(I18n.locale) : t('Choose a date and time');
  if (schedule.type === 'interval') return t('Every {duration}', {duration: durationLabel(schedule.intervalSeconds)});
  return schedule.times.map(clockLabel).join(', ') || t('Add a capture time');
}
function stopLabel(task) {
  const stop = task.stopCondition;
  if (stop.mode === 'at') return stop.endAtLocal ? new Date(stop.endAtLocal).toLocaleString(I18n.locale) : t('Choose a deadline');
  if (stop.mode === 'duration') return t('After {duration}', {duration: durationLabel(stop.durationSeconds)});
  if (stop.mode === 'count') return t('After {count} successful captures', {count: stop.captureCount || '?'});
  return task.schedule.type === 'once' ? t('After this capture') : task.schedule.type === 'fixedOnce' ? t('After each time runs once') : t('No additional limit');
}
function render(value) {
  if (restarting) return;
  snapshot = value;
  const settings = value.settings;
  const languageChanged = I18n.locale !== settings.language;
  if (languageChanged) {
    I18n.setLocale(settings.language);
    document.querySelectorAll('.days label').forEach(label => {
      label.querySelector('span').textContent = t(label.querySelector('input').value);
    });
    document.querySelectorAll('[data-token-description]').forEach(button => {
      const description = button.dataset.tokenDescription;
      button.title = t(description);
      button.setAttribute('aria-label', t('Insert ' + description.toLowerCase() + ' token'));
    });
    for (const id of ['notice', 'form-error']) {
      const node = $(id);
      if (node.dataset.message) node.textContent = I18n.message(node.dataset.message);
    }
  }
  $('language').value = settings.language;
  $('status').textContent = settings.paused ? t('Scheduling paused') : !value.captureAllowed ? t('Desktop unavailable') : value.busy ? t('Capturing...') : t('Scheduler running');
  $('status').classList.toggle('paused', settings.paused || !value.captureAllowed);
  $('pause').textContent = settings.paused ? t('Resume scheduling') : t('Pause scheduling');
  $('startup').checked = settings.startAtSignIn;
  $('capture-now').disabled = manualRequestPending || value.busy || !value.captureAllowed;
  $('capture-now').textContent = value.manualCaptureRunning ? t('Capturing...') : t('Capture now');
  if (!manualFolderDirty) {
    $('manual-folder').value = settings.manualOutputFolder;
    $('manual-split').checked = settings.manualSplitByMonitor === true;
  }
  updateManualFolderState();
  $('manual-output-path').textContent = value.manualOutputFolder;
  if (!shortcutDirty) {
    $('shortcut-enabled').checked = settings.captureShortcut.enabled;
    $('shortcut-modifiers').value = settings.captureShortcut.modifiers;
    $('shortcut-key').value = settings.captureShortcut.key;
  }
  updateShortcutState();
  $('capture-settings-warning').hidden = !value.shortcutError;
  $('capture-shortcut').hidden = !value.shortcutActive;
  $('capture-shortcut').textContent = value.shortcutActive || '';
  if (value.shortcutActive) $('capture-now').setAttribute('aria-keyshortcuts', value.shortcutActive.replace('Ctrl', 'Control'));
  else $('capture-now').removeAttribute('aria-keyshortcuts');
  const manualMessage = value.manualCaptureRunning ? t('Capturing...')
    : value.manualCapture ? I18n.message(value.manualCapture.result) + ' · ' + timeLabel(value.manualCapture.finishedAt) : '';
  if ($('manual-result').textContent !== manualMessage) $('manual-result').textContent = manualMessage;
  $('manual-result').hidden = !manualMessage;
  $('manual-result').classList.toggle('failed', !value.manualCaptureRunning && value.manualCapture?.success === false);
  $('location').textContent = value.applicationDirectory;
  $('count').textContent = settings.tasks.length;
  $('empty').hidden = !!settings.tasks.length;
  $('task-head').hidden = !settings.tasks.length;
  // Keep keyboard focus intact during status refreshes when the rows have not changed.
  const signature = JSON.stringify([settings.language, settings.tasks, value.tasks, value.busy, value.captureAllowed]);
  if (signature === taskListSignature) {
    if (value.error) error(value.error);
    return;
  }
  taskListSignature = signature;
  $('tasks').replaceChildren();
  for (const task of settings.tasks) {
    const state = value.tasks.find(state => state.id === task.id), progress = state?.progress;
    const row = el('tr'), name = el('td');
    const title = el('button', task.name, 'task-title');
    title.type = 'button';
    title.setAttribute('aria-label', t('Edit task {name}', {name: task.name}));
    title.onclick = () => openEditor(task);
    const label = el('label', null, 'task-toggle'), toggle = el('input');
    toggle.type = 'checkbox';
    toggle.checked = task.enabled;
    toggle.setAttribute('aria-label', t('Enable {name}', {name: task.name}));
    toggle.onchange = () => action(async () => {
      const copy = structuredClone(snapshot.settings);
      copy.tasks.find(item => item.id === task.id).enabled = toggle.checked;
      try { render(await api('save', {settings: copy, revision: snapshot.settings.revision})); }
      catch (e) { toggle.checked = task.enabled; throw e; }
    });
    label.append(toggle, task.enabled ? t('Enabled') : t('Disabled'));
    const heading = el('div', null, 'task-heading');
    heading.append(title, label);
    name.append(heading, el('span', task.capture.outputFolder, 'task-path'));
    const openFolder = el('button', t('Open folder'), 'text-button open-folder');
    openFolder.type = 'button';
    openFolder.title = t('Open this output folder in File Explorer');
    openFolder.onclick = () => action(async () => {
      openFolder.disabled = true;
      try { await api('open-folder', {id: task.id}); }
      finally { openFolder.disabled = false; }
    });
    name.append(openFolder);
    const schedule = el('td');
    schedule.append(el('span', scheduleLabel(task), 'schedule-main'));
    if (task.schedule.type !== 'once') schedule.append(el('span', daysLabel(task.schedule.weekdays), 'task-details'));
    if (task.schedule.type === 'fixedOnce') schedule.append(el('span', t('Each time runs once'), 'task-details'));
    if (task.schedule.activeHoursEnabled && task.schedule.type === 'interval') {
      schedule.append(el('span', clockLabel(task.schedule.activeStart) + ' - ' + clockLabel(task.schedule.activeEnd), 'task-details'));
    }
    const next = el('td', task.enabled ? timeLabel(state?.nextExecution) : t('Disabled'));
    if (!task.enabled) next.append(el('span', t('Enable to run on schedule'), 'task-details'));
    const result = el('td', progress?.lastResult ? I18n.message(progress.lastResult) : t('No captures yet'), 'result');
    if (progress?.lastRunUtc) result.append(el('span', timeLabel(progress.lastRunUtc), 'task-details'));
    if (task.stopCondition.mode === 'count') result.append(el('span', t('{done} / {total} scheduled captures', {done: progress?.completedCaptures || 0, total: task.stopCondition.captureCount}), 'task-details'));
    const actions = el('td'), buttons = el('div', null, 'row-actions');
    const add = (text, callback, disabled = false, cls = '') => {
      const button = el('button', text, cls);
      button.type = 'button';
      button.disabled = disabled;
      button.onclick = callback;
      buttons.append(button);
    };
    add(t('Edit'), () => openEditor(task), false, 'edit-task');
    add(t('Duplicate'), () => {
      const copy = structuredClone(task);
      copy.id = crypto.randomUUID();
      copy.name = t('{name} copy', {name: task.name}).slice(0, 64);
      copy.enabled = false;
      openEditor(copy, true);
    });
    add(t('Delete'), () => {
      deleteId = task.id;
      $('delete-name').textContent = task.name;
      $('delete-dialog').returnValue = '';
      $('delete-dialog').showModal();
    });
    actions.append(buttons);
    [name, schedule, next, result, actions].forEach((cell, i) => {
      cell.dataset.label = [t('Task'), t('Schedule'), t('Next capture'), t('Last result'), t('Actions')][i];
      row.append(cell);
    });
    $('tasks').append(row);
  }
  if (value.error) error(value.error);
}
async function action(fn, form = false) {
  try { error('', form); await fn(); }
  catch (e) { if (!restarting) error(e.message, form); }
}
async function refresh() {
  if (refreshing || restarting) return;
  refreshing = true;
  try { render(await api('status')); }
  finally { refreshing = false; }
}
function normalizeTime(value) { return value.length === 5 ? value + ':00' : value; }
function normalizeDate(value) { return value.length === 16 ? value + ':00' : value; }
function fields() {
  const task = structuredClone(editing);
  task.name = $('name').value.trim();
  task.enabled = $('enabled').checked;
  Object.assign(task.capture, {
    outputFolder: $('folder').value.trim(), dateSubfolders: $('dated').checked,
    fileNameTemplate: $('template').value, imageFormat: $('format').value,
    jpegQuality: Number($('quality').value), includeCursor: $('cursor').checked,
    splitByMonitor: $('split').checked
  });
  Object.assign(task.schedule, {
    type: $('kind').value, atLocal: normalizeDate($('at').value) || null,
    intervalSeconds: Number($('interval').value) * Number($('interval-unit').value),
    weekdays: [...document.querySelectorAll('.days input:checked')].map(input => input.value),
    activeHoursEnabled: $('active').checked,
    activeStart: normalizeTime($('start').value), activeEnd: normalizeTime($('end').value),
    times: [...$('times').querySelectorAll('input')].map(input => normalizeTime(input.value))
  });
  Object.assign(task.stopCondition, {
    mode: $('stop').value, endAtLocal: normalizeDate($('deadline').value) || null,
    durationSeconds: Number($('duration').value) * Number($('duration-unit').value) || null,
    captureCount: Number($('captures').value) || null
  });
  return task;
}
function showPanel(name, focus = false) {
  document.querySelectorAll('[role=tab]').forEach(tab => {
    const active = tab.dataset.panel === name;
    tab.setAttribute('aria-selected', String(active));
    tab.tabIndex = active ? 0 : -1;
    $('panel-' + tab.dataset.panel).hidden = !active;
    if (active && focus) tab.focus();
  });
  document.querySelector('.editor-body').scrollTop = 0;
}
const tabs = [...document.querySelectorAll('[role=tab]')];
tabs.forEach((tab, index) => {
  tab.onclick = () => showPanel(tab.dataset.panel);
  tab.onkeydown = event => {
    let next;
    if (event.key === 'ArrowRight') next = (index + 1) % tabs.length;
    if (event.key === 'ArrowLeft') next = (index + tabs.length - 1) % tabs.length;
    if (event.key === 'Home') next = 0;
    if (event.key === 'End') next = tabs.length - 1;
    if (next != null) { event.preventDefault(); showPanel(tabs[next].dataset.panel, true); }
  };
});
function visibility() {
  const kind = $('kind').value;
  const sections = {
    'once-fields': kind === 'once', 'interval-fields': kind === 'interval',
    'fixed-fields': ['fixed', 'fixedOnce'].includes(kind),
    hours: kind === 'interval' && $('active').checked,
    'quality-label': $('format').value === 'jpeg',
    'deadline-label': $('stop').value === 'at',
    'duration-label': $('stop').value === 'duration',
    'captures-label': $('stop').value === 'count'
  };
  // Disable only inactive options, never controls merely hidden by another tab.
  Object.entries(sections).forEach(([id, visible]) => {
    $(id).hidden = !visible;
    $(id).querySelectorAll('input, select').forEach(input => { input.disabled = !visible; });
  });
  $('weekdays').hidden = kind === 'once';
  $('legacy-note').hidden = kind !== 'fixedOnce';
  $('interval').max = Math.floor(31536000 / Number($('interval-unit').value));
  $('duration').max = Math.floor(31536000 / Number($('duration-unit').value));
}
function addTime(value) {
  if (value === undefined) {
    const now = new Date();
    value = [now.getHours(), now.getMinutes(), now.getSeconds()]
      .map(part => String(part).padStart(2, '0')).join(':');
  }
  const row = el('div', null, 'time-entry'), input = el('input'), remove = el('button', '\u00d7', 'text-button');
  input.type = 'time';
  input.step = '1';
  input.required = true;
  input.value = value;
  remove.type = 'button';
  remove.onclick = () => {
    const index = [...$('times').children].indexOf(row);
    row.remove();
    renumberTimes();
    const rows = [...$('times').children];
    (rows[Math.min(index, rows.length - 1)]?.querySelector('input') || $('add-time')).focus();
    draftChanged();
  };
  row.append(input, remove);
  $('times').append(row);
  renumberTimes();
  return input;
}
function renumberTimes() {
  [...$('times').children].forEach((row, index) => {
    row.querySelector('input').setAttribute('aria-label', t('Capture time {number}', {number: index + 1}));
    row.querySelector('button').setAttribute('aria-label', t('Remove capture time {number}', {number: index + 1}));
  });
  $('add-time').disabled = $('times').children.length >= 128;
}
function setUnits(id, seconds) {
  const units = [...$(id + '-unit').options].map(option => Number(option.value)).reverse();
  const unit = units.find(unit => seconds >= unit && seconds % unit === 0) || 1;
  $(id + '-unit').value = unit;
  $(id).value = seconds / unit;
}
function openEditor(task, isNew = false) {
  editing = structuredClone(task);
  editRevision = snapshot.settings.revision;
  $('editor-title').textContent = isNew ? t('New task') : t('Edit task');
  error('', true);
  const capture = task.capture, schedule = task.schedule, stop = task.stopCondition;
  const values = {
    name: task.name, kind: schedule.type, at: schedule.atLocal || '',
    start: schedule.activeStart, end: schedule.activeEnd, folder: capture.outputFolder,
    template: capture.fileNameTemplate, format: capture.imageFormat, quality: capture.jpegQuality,
    stop: stop.mode, deadline: stop.endAtLocal || '', captures: stop.captureCount || 10
  };
  Object.entries(values).forEach(([id, value]) => { $(id).value = value; });
  setUnits('interval', schedule.intervalSeconds);
  setUnits('duration', stop.durationSeconds || 3600);
  $('times').replaceChildren();
  schedule.times.forEach(value => addTime(value));
  renumberTimes();
  $('kind').querySelector('[value=fixedOnce]').hidden = schedule.type !== 'fixedOnce';
  $('enabled').checked = task.enabled;
  $('active').checked = schedule.activeHoursEnabled;
  $('dated').checked = capture.dateSubfolders;
  $('cursor').checked = capture.includeCursor;
  $('split').checked = capture.splitByMonitor === true;
  document.querySelectorAll('.days input').forEach(input => { input.checked = schedule.weekdays.includes(input.value); });
  visibility();
  initialDraft = JSON.stringify(fields());
  showPanel('schedule');
  updateSummary();
  $('editor').showModal();
  $('name').focus();
  updatePreview();
}
function updateSummary() {
  const task = fields();
  $('summary-state').textContent = task.enabled ? t('Enabled after saving') : t('Disabled after saving');
  $('summary-state').classList.toggle('enabled', task.enabled);
  $('summary-schedule').textContent = scheduleLabel(task);
  if (task.schedule.type === 'interval' && task.schedule.activeHoursEnabled) {
    $('summary-schedule').textContent += ', ' + clockLabel(task.schedule.activeStart) + ' - ' + clockLabel(task.schedule.activeEnd);
  }
  $('summary-days').textContent = task.schedule.type === 'once' ? t('One time only') : daysLabel(task.schedule.weekdays);
  $('summary-stop').textContent = stopLabel(task);
  $('summary-image').textContent = task.capture.imageFormat.toUpperCase()
    + (task.capture.imageFormat === 'jpeg' ? t(' / quality {quality}', {quality: task.capture.jpegQuality}) : '')
    + (task.capture.includeCursor ? t(' / with cursor') : '')
    + t(task.capture.splitByMonitor ? ' / one image per screen' : ' / all screens in one image');
  $('preview-label').textContent = t(task.capture.splitByMonitor ? 'Example file · monitor 1' : 'Example file · all monitors');
  $('save-hint').textContent = task.enabled ? t('Runs on schedule after saving.') : t('Saved as disabled. Enable when ready.');
}
let previewSerial = 0;
async function updatePreview() {
  const serial = ++previewSerial;
  try {
    const value = await api('preview', {task: fields()});
    if (serial === previewSerial) $('preview').textContent = value.path;
  } catch (e) {
    if (serial === previewSerial) $('preview').textContent = I18n.message(e.message);
  }
}
function draftChanged() {
  visibility();
  updateSummary();
  clearTimeout(previewTimer);
  previewTimer = setTimeout(updatePreview, 250);
}
$('task-form').addEventListener('input', draftChanged);
$('add-time').onclick = () => { addTime().focus(); draftChanged(); };
function selectDays(days) {
  document.querySelectorAll('.days input').forEach(input => { input.checked = days.includes(input.value); });
  draftChanged();
}
$('days-all').onclick = () => selectDays(dayNames);
$('days-work').onclick = () => selectDays(dayNames.slice(0, 5));
function validateDraft() {
  for (const input of $('task-form').querySelectorAll('input, select')) {
    if (!input.disabled && !input.checkValidity()) {
      const panel = input.closest('[role=tabpanel]');
      if (panel) showPanel(panel.id.replace('panel-', ''));
      const validity = input.validity;
      error(validity.valueMissing ? t('Fill in this field.')
        : validity.rangeUnderflow || validity.rangeOverflow
          ? t('Enter a value from {min} to {max}.', {min: input.min, max: input.max})
          : t('Enter a valid value.'), true, true);
      input.focus();
      return false;
    }
  }
  const task = fields(), schedule = task.schedule;
  if (schedule.type !== 'once' && !schedule.weekdays.length) {
    showPanel('schedule');
    error('Choose at least one day for this schedule.', true);
    document.querySelector('.days input').focus();
    return false;
  }
  if (['fixed', 'fixedOnce'].includes(schedule.type) && (!schedule.times.length || new Set(schedule.times).size !== schedule.times.length)) {
    showPanel('schedule');
    error(schedule.times.length ? 'Each capture time must be different. Remove or change the repeated time.' : 'Add at least one capture time.', true);
    $('add-time').focus();
    return false;
  }
  return true;
}
$('task-form').addEventListener('submit', event => {
  event.preventDefault();
  error('', true);
  if (!validateDraft()) return;
  action(async () => {
    $('save').disabled = true;
    try {
      const settings = structuredClone(snapshot.settings), task = fields();
      const index = settings.tasks.findIndex(item => item.id === task.id);
      if (index < 0) settings.tasks.push(task); else settings.tasks[index] = task;
      render(await api('save', {settings, revision: editRevision}));
      $('editor').close();
    } finally { $('save').disabled = false; }
  }, true);
});
$('browse').onclick = () => action(async () => {
  $('browse').disabled = true;
  try {
    const result = await api('folder');
    if (result.path !== null) { $('folder').value = result.path; draftChanged(); }
  } finally { $('browse').disabled = false; }
}, true);
function closeEditor() {
  if (JSON.stringify(fields()) !== initialDraft && !window.confirm(t('Discard your unsaved changes?'))) return;
  $('editor').close();
}
$('close').onclick = $('cancel').onclick = closeEditor;
$('editor').addEventListener('cancel', event => { event.preventDefault(); closeEditor(); });
$('editor').addEventListener('close', () => { clearTimeout(previewTimer); ++previewSerial; });
$('create').onclick = $('empty-create').onclick = () => {
  if (snapshot) openEditor({...structuredClone(snapshot.defaultTask), id: crypto.randomUUID(), name: t('New task')}, true);
};
$('pause').onclick = () => action(async () => render(await api('pause', {paused: !snapshot.settings.paused, revision: snapshot.settings.revision})));
function updateShortcutState() {
  updateCaptureSettingsDraft();
  $('shortcut-save').disabled = !shortcutDirty || shortcutSaving;
  $('shortcut-enabled').disabled = shortcutSaving;
  $('shortcut-modifiers').disabled = $('shortcut-key').disabled = shortcutSaving || !$('shortcut-enabled').checked;
  const value = snapshot;
  $('shortcut-status').textContent = shortcutDirty ? t('Unsaved shortcut. Save to apply it.')
    : value.shortcutError ? t(value.shortcutError)
    : !value.settings.captureShortcut.enabled ? t('Capture shortcut is off.')
    : value.shortcutActive ? t('Shortcut active: {shortcut}', {shortcut:value.shortcutActive})
    : t('Applying capture shortcut...');
  $('shortcut-status').classList.toggle('failed', !!value.shortcutError && !shortcutDirty);
}
$('shortcut-form').onchange = () => {
  if (!shortcutDirty) shortcutRevision = snapshot.settings.revision;
  shortcutDirty = true;
  updateShortcutState();
};
$('shortcut-form').onsubmit = event => {
  event.preventDefault();
  action(async () => {
    const settings = structuredClone(snapshot.settings);
    settings.captureShortcut = {enabled:$('shortcut-enabled').checked, modifiers:$('shortcut-modifiers').value, key:$('shortcut-key').value};
    shortcutSaving = true; updateShortcutState();
    try {
      const value = await api('save', {settings, revision:shortcutRevision});
      shortcutDirty = false; render(value);
      await refresh();
    } finally { shortcutSaving = false; updateShortcutState(); }
  });
};
function updateCaptureSettingsDraft() {
  $('capture-settings-draft').hidden = !manualFolderDirty && !shortcutDirty;
}
function updateManualFolderState() {
  updateCaptureSettingsDraft();
  $('manual-folder-save').disabled = !manualFolderDirty || !!manualFolderSaving;
  $('manual-folder-state').textContent = manualFolderDirty
    ? t('Unsaved capture settings. Capture now and Open captures folder save them first.')
    : t('Capture settings saved for browser, shortcut, and tray captures.');
  $('manual-output-path').hidden = manualFolderDirty;
}
function manualFolderChanged() {
  if (!manualFolderDirty) manualFolderRevision = snapshot.settings.revision;
  manualFolderDirty = true;
  updateManualFolderState();
}
async function saveManualFolder() {
  if (manualFolderSaving) return manualFolderSaving;
  if (!manualFolderDirty) return;
  $('capture-settings').open = true;
  if (!$('manual-folder').reportValidity()) throw new Error(t('Enter a valid output folder.'));
  const settings = structuredClone(snapshot.settings);
  settings.manualOutputFolder = $('manual-folder').value;
  settings.manualSplitByMonitor = $('manual-split').checked;
  $('manual-folder').disabled = $('manual-split').disabled = $('manual-browse').disabled = $('manual-folder-save').disabled = true;
  manualFolderSaving = (async () => {
    try {
      const value = await api('save', {settings, revision: manualFolderRevision});
      manualFolderDirty = false;
      render(value);
    } finally {
      manualFolderSaving = undefined;
      $('manual-folder').disabled = $('manual-split').disabled = $('manual-browse').disabled = false;
      updateManualFolderState();
    }
  })();
  return manualFolderSaving;
}
$('manual-folder').oninput = manualFolderChanged;
$('manual-split').oninput = manualFolderChanged;
$('manual-folder-form').onsubmit = event => { event.preventDefault(); action(saveManualFolder); };
$('manual-browse').onclick = () => action(async () => {
  $('manual-browse').disabled = true;
  try {
    const result = await api('folder');
    if (result.path !== null) { $('manual-folder').value = result.path; manualFolderChanged(); }
  } finally { $('manual-browse').disabled = false; }
});
$('capture-now').onclick = () => action(async () => {
  manualRequestPending = true;
  $('capture-now').disabled = true;
  try { await saveManualFolder(); await api('capture'); }
  finally {
    manualRequestPending = false;
    await refresh();
  }
});
$('open-captures').onclick = () => action(async () => {
  $('open-captures').disabled = true;
  try { await saveManualFolder(); await api('open-captures'); }
  finally { $('open-captures').disabled = false; }
});
$('language').onchange = () => action(async () => {
  const settings = structuredClone(snapshot.settings);
  settings.language = $('language').value;
  $('language').disabled = true;
  try { render(await api('save', {settings, revision: snapshot.settings.revision})); }
  catch (e) { $('language').value = snapshot.settings.language; throw e; }
  finally { $('language').disabled = false; }
});
$('startup').onchange = () => action(async () => {
  const settings = structuredClone(snapshot.settings);
  settings.startAtSignIn = $('startup').checked;
  try { render(await api('save', {settings, revision: snapshot.settings.revision})); }
  catch (e) { $('startup').checked = snapshot.settings.startAtSignIn; throw e; }
});
$('restart').onclick = () => action(async () => {
  if (!snapshot || restarting) return;
  if (manualFolderDirty || shortcutDirty || manualFolderSaving || shortcutSaving) {
    throw new Error(t('Save or discard your capture settings changes before restarting.'));
  }
  restarting = true;
  $('restart').disabled = true;
  try {
    await api('restart');
    $('restart-result').textContent = t('Restart requested. The task manager will reopen in a new tab. You can close this tab.');
    $('restart-result').hidden = false;
    document.querySelectorAll('button, input, select').forEach(node => { node.disabled = true; });
  } catch (e) {
    restarting = false;
    $('restart').disabled = false;
    throw e;
  }
});
$('export-config').onclick = () => action(async () => {
  $('export-config').disabled = true;
  try {
    const settings = await api('export');
    const url = URL.createObjectURL(new Blob([JSON.stringify(settings, null, 2) + '\n'], {type: 'application/json'}));
    const link = el('a');
    link.href = url;
    link.download = 'scheduled-screenshot-config-' + new Date().toISOString().slice(0, 10) + '.json';
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  } finally { $('export-config').disabled = false; }
});
$('import-config').onclick = () => {
  if (!snapshot) return;
  $('config-file').value = '';
  $('config-file').click();
};
$('config-file').onchange = () => action(async () => {
  const file = $('config-file').files[0];
  if (!file || !snapshot) return;
  const revision = snapshot.settings.revision;
  $('import-config').disabled = true;
  $('config-result').hidden = true;
  try {
    if (file.size > 1048576) throw new Error(t('Configuration file must be 1 MB or smaller.'));
    let settings;
    try { settings = JSON.parse((await file.text()).replace(/^\uFEFF/, '')); }
    catch { throw new Error(t('Choose a valid JSON configuration file.')); }
    // Include the request envelope when checking the server's body limit.
    if (new Blob([JSON.stringify({settings, revision})]).size > 1048576) {
      throw new Error(t('Configuration file must be 1 MB or smaller.'));
    }
    settings = await api('validate-import', {settings});
    pendingImport = {settings, revision};
    $('import-summary').textContent = t('Import {count} task(s) from {file}?', {count: settings.tasks.length, file: file.name});
    $('import-error').hidden = true;
    $('import-dialog').showModal();
    $('import-cancel').focus();
  } finally {
    $('import-config').disabled = false;
    $('config-file').value = '';
  }
});
$('import-cancel').onclick = () => $('import-dialog').close();
$('import-dialog').addEventListener('cancel', event => { if (importing) event.preventDefault(); });
$('import-dialog').addEventListener('close', () => { pendingImport = undefined; });
$('import-confirm').onclick = async () => {
  if (!pendingImport || importing) return;
  importing = true;
  $('import-confirm').disabled = $('import-cancel').disabled = true;
  $('import-error').hidden = true;
  try {
    render(await api('import', pendingImport));
    $('import-dialog').close();
    error('');
    $('config-result').textContent = t('Configuration imported. Review your tasks, then resume scheduling when ready.');
    $('config-result').hidden = false;
  } catch (e) {
    $('import-error').textContent = I18n.message(e.message);
    $('import-error').hidden = false;
  } finally {
    importing = false;
    $('import-confirm').disabled = $('import-cancel').disabled = false;
  }
};
$('delete-dialog').addEventListener('close', () => {
  if ($('delete-dialog').returnValue === 'delete') action(async () => {
    const settings = structuredClone(snapshot.settings);
    settings.tasks = settings.tasks.filter(task => task.id !== deleteId);
    render(await api('save', {settings, revision: snapshot.settings.revision}));
  });
});
document.addEventListener('visibilitychange', () => { if (!document.hidden && !$('editor').open && !$('import-dialog').open) action(refresh); });
setInterval(() => { if (!document.hidden && !$('editor').open && !$('delete-dialog').open && !$('import-dialog').open) action(refresh); }, 3000);
action(refresh);

})().catch(error => { const notice = document.getElementById('notice'); notice.textContent = error.message; notice.hidden = false; });
