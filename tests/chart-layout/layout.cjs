// Optional headless HTML regression; never an OS/WebView2 manual-input proof.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const { chromium } = require('playwright');
const root = path.resolve(__dirname, '../..');
const output = path.join(root, 'output/playwright/chart-layout');
const baseline = '9f1e27c92a1d31c7f069331a65f588f8aa6c97ef';
const sourcePath = 'examples/native_dashboard/chart.html';
const runtime = fs.readFileSync(path.join(root, 'assets/vendor/echarts/echarts.min.js'), 'utf8');
const packet = {
  version: 2, dataset: 'LAB-DASH-01', revision: 1, generation: 1,
  first: 1, last: 69, case: 'Fixture A', direction: 'X',
  drift_unit: 'mm', shear_unit: 'kN', drift_reference: 3,
  shear_reference: 500, selected_story: 23,
  rows: Array.from({ length: 69 }, (_, i) => ({ story: i + 1, drift: (i + 1) * 0.1, shear: (69 - i) * 10 }))
};
const widths = [1100, 832, 420, 900];
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
function pageSource(html) {
  assert.equal(html.split('/*__ECHARTS_RUNTIME__*/').length, 2);
  return html.replace('<head>', '<head><script>window.ipc={postMessage(){}};</script>')
    .replace('/*__ECHARTS_RUNTIME__*/', runtime);
}
async function readLayout(page) {
  return page.evaluate(() => {
    const box = r => ({ x: r.x, y: r.y, width: r.width, height: r.height });
    const overlap = (a, b) => a && b && a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
    return {
      width: innerWidth, height: innerHeight,
      document_width: document.documentElement.scrollWidth,
      document_height: document.documentElement.scrollHeight,
      plots: Object.entries(charts).map(([kind, chart]) => {
        const dom = chart.getDom().getBoundingClientRect();
        const text = chart.getZr().storage.getDisplayList()
          .filter(el => el.style && typeof el.style.text === 'string')
          .map(el => { const rect = el.getBoundingRect().clone(); if (el.transform) rect.applyTransform(el.transform); return { text: el.style.text, ...box(rect) }; });
        const title = text.find(el => el.text.startsWith('Synthetic story'));
        const story = text.find(el => el.text === 'Story');
        return { kind, dom: box(dom), canvas_width: chart.getWidth(), title_story_overlap: !!overlap(title, story) };
      })
    };
  });
}
async function scenario(browser, html, scale, legacy) {
  const context = await browser.newContext({ viewport: { width: 1100, height: 720 }, deviceScaleFactor: scale });
  try {
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setContent(pageSource(html), { waitUntil: 'load' });
    await page.evaluate(p => window.dashboardBridge.apply(p), packet);
    const layouts = [];
    for (const width of widths) {
      await page.setViewportSize({ width, height: width < 500 ? 440 : 676 });
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      const layout = await readLayout(page);
      layouts.push(layout);
      if (!legacy) {
        assert.ok(layout.document_width <= width, `document overflows at width ${width}, scale ${scale}`);
        assert.ok(layout.document_height <= layout.height, 'document height overflows');
        for (const plot of layout.plots) {
          assert.ok(plot.dom.width <= width && plot.dom.x >= 0, `${plot.kind} grid item does not shrink`);
          assert.ok(Math.abs(plot.canvas_width - plot.dom.width) <= 1, 'canvas and container widths differ');
          assert.equal(plot.title_story_overlap, false, 'Story axis title overlaps chart heading');
        }
      }
      if (width === 832 && scale === 1) await page.screenshot({ path: path.join(output, legacy ? 'before.png' : 'after.png') });
    }
    if (!legacy) {
      const preserved = await page.evaluate(p => {
        charts.drift.dispatchAction({ type: 'dataZoom', start: 20, end: 80 });
        charts.drift.dispatchAction({ type: 'legendUnSelect', name: 'drift' });
        window.dashboardBridge.apply({ ...p, selected_story: 17 });
        const option = charts.drift.getOption();
        return { selected: JSON.parse(window.dashboardBridge.snapshot()).selected_story, start: option.dataZoom[0].start, end: option.dataZoom[0].end, legend: option.legend[0].selected.drift };
      }, packet);
      assert.deepEqual(preserved, { selected: 17, start: 20, end: 80, legend: false }, 'selection update reset zoom or legend');
    }
    assert.deepEqual(errors, [], 'page errors');
    return { scale, layouts };
  } finally { await context.close(); }
}
(async () => {
  fs.mkdirSync(output, { recursive: true });
  const current = fs.readFileSync(path.join(root, sourcePath), 'utf8');
  const old = execFileSync('git', ['show', `${baseline}:${sourcePath}`], { cwd: root, encoding: 'utf8' });
  const server = await chromium.launchServer({ headless: true });
  const ownedProcess = server.process();
  let browser;
  let report;
  try {
    browser = await chromium.connect(server.wsEndpoint());
    const before = await scenario(browser, old, 1, true);
    assert.ok(before.layouts.some(l => l.document_width > l.width || l.plots.some(p => p.title_story_overlap)), 'legacy regression was not reproduced');
    const after = [];
    for (const scale of [1, 1.25, 1.5]) after.push(await scenario(browser, current, scale, false));
    report = { assessment: 'PASS', scope: 'Headless Chromium HTML resize/layout only; not native WebView2/physical input/DPI proof', baseline, source_commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(), html_sha256: sha(current), playwright: require('playwright/package.json').version, browser: browser.version(), before, after };
  } finally {
    try { if (browser) await browser.close(); } finally { await server.close(); }
  }
  assert.ok(ownedProcess.exitCode !== null || ownedProcess.signalCode !== null, 'owned browser process did not exit');
  report.owned_browser_exit_observed = true;
  fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ assessment: report.assessment, scenarios: report.after.length, resize_samples: report.after.length * widths.length, browser: report.browser, owned_browser_exit_observed: true }));
})().catch(error => { console.error(error); process.exitCode = 1; });
