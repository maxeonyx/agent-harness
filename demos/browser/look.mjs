// What the harness's browser shows, read over DevTools as Max's own
// browser would show it: each tab's address, and the claim form's lines.
// Usage: node look.mjs PROFILE (the launched browser's profile directory)
import { readFileSync } from 'node:fs';

const [port, path] = readFileSync(`${process.argv[2]}/DevToolsActivePort`, 'utf8').split('\n');
const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const pages = tabs.filter(t => t.type === 'page');
if (pages.length === 0) console.log('  no tabs open');
for (const t of pages) {
  console.log(`  tab ${t.id.slice(0, 8)}  ${t.url.replace(/^file:\/\/.*\//, '')}  "${t.title}"`);
  if (!t.url.endsWith('claim.html')) continue;
  const ws = new WebSocket(t.webSocketDebuggerUrl);
  await new Promise(r => ws.onopen = r);
  const expression = `JSON.stringify({
    period: document.querySelector('[name=period]').value,
    lines: [...document.querySelectorAll('#lines .line')].map(l =>
      [...l.querySelectorAll('input')].map(i => i.value).join(' | ')),
    total: document.getElementById('total').textContent,
    submitted: document.getElementById('done').textContent || 'not submitted',
  })`;
  ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression, returnByValue: true } }));
  const answer = await new Promise(r => ws.onmessage = m => r(JSON.parse(m.data)));
  const form = JSON.parse(answer.result.result.value);
  console.log(`    period: ${form.period || '(none)'}`);
  form.lines.forEach((l, i) => console.log(`    ${i + 1}. ${l}`));
  console.log(`    total: $${form.total}; ${form.submitted}`);
  ws.close();
}
