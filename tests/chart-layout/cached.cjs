// Independent synthetic packet only; never read project/result files here.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');
const root = path.resolve(__dirname, '../..');
const output = path.join(root, 'output/playwright/cached-layout');
let packet = {
  version: 1, source: '0'.repeat(64), generation: 2, selected_story: 'Roof',
  drift: { title: 'Cached synthetic wind drift · G · W', unit: 'ratio', stories: ['Roof', 'L1'], reference: 0.01,
    series: [{ name: 'Stored positive envelope', values: [0.002, 0.001] }, { name: 'Stored negative envelope', values: [-0.0025, null] }] },
  shear: { title: 'Cached signed synthetic shear · FX', unit: 'kip', stories: ['Roof', 'L1'], reference: null,
    series: [{ name: 'FX', values: [7, -2] }] }
};
const publicArg = process.argv.find(arg => arg.startsWith('--public-probe='));
if (publicArg) {
  const directory = publicArg.slice('--public-probe='.length);
  const status = JSON.parse(fs.readFileSync(path.join(directory, 'status.json'), 'utf8'));
  assert.equal(status.fixture, 'LAB-CACHED-DTO-01-gap-r1');
  assert.equal(status.assessment, 'PASS');
  assert.equal(status.analysis_freshness_verified, false);
  assert.equal(status.inputs_unchanged, true);
  packet = JSON.parse(fs.readFileSync(path.join(directory, 'packet.json'), 'utf8'));
  assert.equal(packet.source, status.source);
  assert.equal(packet.drift.title, 'Cached wind drift X · G · W');
  assert.equal(packet.shear.title, 'Cached signed shear X · FX');
}
(async () => {
  fs.mkdirSync(output, { recursive: true });
  const template = fs.readFileSync(path.join(root, 'examples/native_results/chart.html'), 'utf8');
  const runtime = fs.readFileSync(path.join(root, 'assets/vendor/echarts/echarts.min.js'), 'utf8');
  const html = template.replace('<head>', '<head><script>window.ipc={postMessage(){}};</script>').replace('/*__ECHARTS_RUNTIME__*/', runtime);
  const server = await chromium.launchServer({ headless: true });
  const owned = server.process();
  let browser;
  const cases = [];
  try {
    browser = await chromium.connect(server.wsEndpoint());
    const context = await browser.newContext({ viewport: { width: 1000, height: 700 } });
    try {
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', e => errors.push(e.message));
      await page.setContent(html);
      await page.evaluate(p => window.resultBridge.apply(p), packet);
      for (const width of [1000, 420, 832]) {
        await page.setViewportSize({ width, height: 700 });
        await page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
        const value = await page.evaluate(() => ({ width: innerWidth, scroll: document.documentElement.scrollWidth,
          drift: charts.drift.getOption().series.filter(s => s.id !== 'stored-reference').map(s => s.data), shear: charts.shear.getOption().series[0].data,
          inverse: charts.drift.getOption().yAxis[0].inverse,
          drift_axis_extent: charts.drift.getModel().getComponent('xAxis').axis.scale.getExtent(),
          canvas: Object.values(charts).map(c => c.getWidth()) }));
        assert.ok(value.scroll <= width && value.canvas.every(w => w <= width));
        assert.deepEqual(value.drift, [[[0.002, 'Roof'], [publicArg ? null : 0.001, 'L1']], [[-0.0025, 'Roof'], [null, 'L1']]]);
        assert.deepEqual(value.shear, [[7, 'Roof'], [-2, 'L1']]);
        assert.equal(value.inverse, true);
        assert.ok(value.drift_axis_extent[1] >= packet.drift.reference, 'stored reference clipped outside the visible domain');
        cases.push({ width, assessment: 'PASS' });
      }
      const stale = await page.evaluate(p => {
        window.resultBridge.apply({ ...p, generation: 1, selected_story: 'L1' });
        return active.selected_story;
      }, packet);
      assert.equal(stale, 'Roof');
      assert.deepEqual(errors, []);
      await page.screenshot({ path: path.join(output, 'synthetic-cached-charts.png') });
    } finally { await context.close(); }
  } finally {
    try { if (browser) await browser.close(); } finally { await server.close(); }
  }
  assert.ok(owned.exitCode !== null || owned.signalCode !== null);
  const result = { assessment: 'PASS', scope: 'Independently authored cached-packet HTML only; no private input, native window, analysis or SDK lifecycle claim', native_public_packet_consumed: !!publicArg, cases, signed_values_and_nulls_preserved: true, stale_packet_refused: true, owned_browser_exit_observed: true };
  fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
})().catch(e => { console.error(e); process.exitCode = 1; });
