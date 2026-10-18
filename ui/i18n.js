'use strict';
// Language packs are bundled in the executable; no network services are used.
const I18n = (() => {
  let locale = 'en';
  const packs = {}, textBindings = [], attributeBindings = [];
  function t(key, values = {}) {
    const template = packs[locale]?.[key] ?? packs.en?.[key] ?? key;
    return template.replace(/\{([a-zA-Z]+)\}/g, (match, name) => String(values[name] ?? match));
  }
  function bindStatic() {
    const walker = document.createTreeWalker(document.documentElement, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const node = walker.currentNode;
      if (['SCRIPT', 'STYLE'].includes(node.parentElement?.tagName)) continue;
      const key = node.nodeValue.trim();
      if (Object.hasOwn(packs.en, key)) {
        textBindings.push({node, key, before: node.nodeValue.match(/^\s*/)[0], after: node.nodeValue.match(/\s*$/)[0]});
      }
    }
    document.querySelectorAll('[aria-label], [placeholder], [title]').forEach(node => {
      for (const attribute of ['aria-label', 'placeholder', 'title']) {
        const key = node.getAttribute(attribute);
        if (key && Object.hasOwn(packs.en, key)) attributeBindings.push({node, attribute, key});
      }
    });
  }
  function setLocale(language) {
    locale = Object.hasOwn(packs, language) ? language : 'en';
    document.documentElement.lang = locale;
    for (const {node, key, before, after} of textBindings) {
      if (node.isConnected) node.nodeValue = before + t(key) + after;
    }
    for (const {node, attribute, key} of attributeBindings) node.setAttribute(attribute, t(key));
  }
  function message(value) {
    if (!value || locale === 'en') return value || '';
    if (Object.hasOwn(packs[locale], value)) return t(value);
    if (Object.values(packs[locale]).includes(value)) return value;
    let match = value.match(/^Saved (\d+) (?:monitor )?image\(s\)\.$/);
    if (match) return t('Saved {count} image(s).', {count: match[1]});
    if (value.startsWith('Failed: ')) return t('Failed: {detail}', {detail: message(value.slice(8))});
    match = value.match(/^Unknown filename token: (.+)\.$/);
    if (match) return t('Unknown filename token: {token}.', {token: match[1]});
    // Validation errors may have a user-defined task-name prefix. Preserve that name.
    for (const key of Object.keys(packs[locale])) {
      if (value.endsWith(': ' + key)) return value.slice(0, -key.length) + t(key);
    }
    return t('Operation failed. Details: {detail}', {detail: value});
  }
  return {
    t, message, setLocale,
    get locale() { return locale; },
    async load() {
      await Promise.all(['en', 'zh-Hant'].map(async language => {
        const response = await fetch('/locales/' + language + '.json', {cache: 'no-store'});
        if (!response.ok) throw new Error('Could not load language pack: ' + language);
        packs[language] = await response.json();
      }));
      bindStatic();
    }
  };
})();
