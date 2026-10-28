// Headless check of apps/talos-native/assets/results_chart.html in Chromium
// (the engine WebView2 is built on). Public synthetic values only.
//
//   node docs/native/evidence/app-01c-echarts/chart_page_test.js <out-dir>
//
// Writes messages.json (what the page posted, for the Rust bridge test) and
// light/dark PNGs, and exits non-zero on the first failed check.
const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright');

const root = path.resolve(__dirname, '../../../..');
const out = path.resolve(process.argv[2] || '.');
const page = fs.readFileSync(path.join(root, 'apps/talos-native/assets/results_chart.html'), 'utf8');
const echarts = fs.readFileSync(path.join(root, 'assets/vendor/echarts/echarts.min.js'), 'utf8');
// WebView2's window.ipc stands in as a recorder, installed before the page runs.
const recorder = '<script>window.__posted=[];window.ipc={postMessage:(m)=>window.__posted.push(m)};</script>';
const html = page.replace('/*__ECHARTS_RUNTIME__*/', () => echarts).replace('<head>', () => '<head>' + recorder);

// The same frame the Rust bridge test builds (`content("Story shear", None)`).
const style = (dark) => ({
  background: dark ? 'rgba(22,27,34,1.000)' : 'rgba(255,255,255,1.000)',
  text: dark ? 'rgba(230,233,238,1.000)' : 'rgba(24,24,27,1.000)',
  muted: 'rgba(128,128,128,1.000)', grid: dark ? 'rgba(40,46,55,1.000)' : 'rgba(228,228,231,1.000)',
  axis: 'rgba(128,128,128,1.000)', selected: 'rgba(0,128,128,0.251)',
  reference: 'rgba(220,70,70,1.000)', series: ['rgba(0,0,255,1.000)'],
});
const frame = (generation, selected, dark = false, title = 'Story shear') => ({
  protocol: 1, version: 'main/v2', source: 'abc123', generation, selected_story: selected,
  plot: { title, unit: 'kN', stories: ['Roof', 'L2'], series: [{ name: 'Vx', values: [-12.5, null] }], reference: null },
  style: style(dark),
});

function check(ok, what) {
  if (!ok) { console.error('FAIL', what); process.exit(1); }
  console.log('ok  ', what);
}

(async () => {
  const browser = await chromium.launch();
  const tab = await browser.newPage({ viewport: { width: 900, height: 420 } });
  const errors = [];
  tab.on('pageerror', (e) => errors.push(String(e)));
  await tab.setContent(html, { waitUntil: 'load' });
  const raw = [];
  const take = async () => { const list = await tab.evaluate(() => window.__posted.splice(0)); raw.push(...list); return list.map((m) => JSON.parse(m)); };

  check(JSON.stringify(await take()) === '[{"protocol":1,"type":"ready"}]', 'ready is the first and only message on load');
  await tab.evaluate((f) => window.talosChart.apply(f), frame(1, null));
  check((await tab.getAttribute('#chart', 'aria-label')) === 'Chart: Story shear', 'frame applied');

  // Click the middle of each story row inside the grid.
  const row = async (index) => tab.evaluate((i) => {
    const c = echarts.getInstanceByDom(document.getElementById('chart'));
    const y = c.convertToPixel({ yAxisIndex: 0 }, i);
    return { x: c.getWidth() / 2, y };
  }, index);
  let at = await row(1);
  await tab.mouse.click(at.x, at.y);
  let got = await take();
  check(got.length === 1 && got[0].type === 'select_story' && got[0].story === 'L2'
    && got[0].version === 'main/v2' && got[0].source === 'abc123' && got[0].generation === 1,
    'clicking a row selects that story, with version, source and generation');

  // The host echoes the selection; a second click on it clears.
  await tab.evaluate((f) => window.talosChart.apply(f), frame(1, 'L2'));
  await tab.mouse.click(at.x, at.y);
  got = await take();
  check(got.length === 1 && got[0].type === 'clear_selection' && got[0].story === null, 'clicking the selected story clears it');

  // Outside the plot grid nothing is posted.
  await tab.mouse.click(4, 4);
  check((await take()).length === 0, 'a click outside the plot posts nothing');

  // Keyboard: Escape clears; Ctrl+2 / Ctrl+PageDown are forwarded; other keys are not.
  await tab.focus('#chart');
  await tab.keyboard.press('Escape');
  await tab.keyboard.press('Control+Digit2');
  await tab.keyboard.press('Control+PageDown');
  await tab.keyboard.press('KeyQ');
  await tab.keyboard.press('Control+KeyR');
  got = await take();
  check(got.map((m) => m.type + ':' + (m.key || m.story)).join(',') === 'clear_selection:null,shortcut:Digit2,shortcut:PageDown',
    'Escape clears; only workspace shortcuts are forwarded');

  // An older generation is ignored; a new one replaces the chart.
  await tab.evaluate((f) => window.talosChart.apply(f), frame(2, null, false, 'Story moment'));
  await tab.evaluate((f) => window.talosChart.apply(f), frame(1, null, false, 'Story shear'));
  check((await tab.getAttribute('#chart', 'aria-label')) === 'Chart: Story moment', 'an older generation is ignored');
  at = await row(0);
  await tab.mouse.click(at.x, at.y);
  got = await take();
  check(got.length === 1 && got[0].generation === 2 && got[0].story === 'Roof', 'messages name the generation shown');

  // A malformed frame throws and changes nothing.
  const threw = await tab.evaluate(() => { try { window.talosChart.apply({ protocol: 2 }); return false; } catch { return true; } });
  check(threw && (await tab.getAttribute('#chart', 'aria-label')) === 'Chart: Story moment', 'a malformed frame is refused');

  // The page cannot reach the network or navigate (CSP); the host also denies navigation.
  const fetched = await tab.evaluate(async () => { try { await fetch('https://example.com'); return 'fetched'; } catch { return 'blocked'; } });
  check(fetched === 'blocked', 'network access is blocked by the page CSP');
  check(errors.length === 0, 'no page errors: ' + errors.join('; '));

  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, 'messages.json'), JSON.stringify(raw, null, 1) + '\n');
  // Evidence images: an authored synthetic drift frame (made-up values, a
  // gap at L3, a stored reference), light and dark, one story selected.
  for (const dark of [false, true]) {
    const authored = frame(dark ? 4 : 3, 'L4', dark, 'Story drift · wind X (authored)');
    authored.plot = {
      title: 'Story drift · wind X (authored)', unit: '', reference: 0.004,
      stories: ['Roof', 'L6', 'L5', 'L4', 'L3', 'L2', 'L1'],
      series: [
        { name: 'Positive', values: [0.0021, 0.0026, 0.0031, 0.0034, null, 0.0024, 0.0012] },
        { name: 'Negative', values: [-0.0019, -0.0024, -0.0029, -0.0033, null, -0.0022, -0.0011] },
      ],
    };
    authored.style.series = dark ? ['rgba(94,154,255,1.000)', 'rgba(240,170,60,1.000)'] : ['rgba(37,99,235,1.000)', 'rgba(217,119,6,1.000)'];
    await tab.evaluate((f) => window.talosChart.apply(f), authored);
    await tab.screenshot({ path: path.join(out, `chart-${dark ? 'dark' : 'light'}.png`) });
  }
  await browser.close();
  console.log('all checks passed');
})().catch((e) => { console.error(e); process.exit(1); });
