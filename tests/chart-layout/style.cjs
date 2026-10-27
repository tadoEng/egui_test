// Public authored presentation regression; no user project/results access.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');
const root = path.resolve(__dirname, '../..');
const output = path.join(root, 'output/playwright/cached-style');
const stories = Array.from({ length: 69 }, (_, i) => `Story ${69 - i}`);
const packet = {
  version: 1, source: 'a'.repeat(64), generation: 4, selected_story: null,
  drift: { title: 'Cached wind drift X · LAB presentation fixture · Authored case', unit: 'ratio', stories, reference: 0.001,
    series: [{ name: 'Stored positive envelope', values: stories.map((_, i) => i === 5 ? null : (69 - i) / 100000) }, { name: 'Stored negative envelope', values: stories.map((_, i) => i === 5 ? null : -(69 - i) / 100000) }] },
  shear: { title: 'Cached signed shear X · LAB authored case', unit: 'kip', stories, reference: null,
    series: [{ name: 'LAB authored case', values: stories.map((_, i) => (35 - i) * 10) }] }
};
(async () => {
  fs.mkdirSync(output, { recursive: true });
  const template = fs.readFileSync(path.join(root, 'examples/native_results/chart.html'), 'utf8');
  const runtime = fs.readFileSync(path.join(root, 'assets/vendor/echarts/echarts.min.js'), 'utf8');
  const html = template.replace('<head>', '<head><script>window.messages=[];window.ipc={postMessage(raw){messages.push(JSON.parse(raw));}};</script>').replace('/*__ECHARTS_RUNTIME__*/', runtime);
  const server = await chromium.launchServer({ headless: true });
  const owned = server.process();
  let browser;
  const cases = [];
  try {
    browser = await chromium.connect(server.wsEndpoint());
    for (const ratio of [1, 1.25, 1.5]) {
      const context = await browser.newContext({ viewport: { width: 1320, height: 900 }, deviceScaleFactor: ratio });
      try {
        const page = await context.newPage();
        const errors = [];
        page.on('pageerror', error => errors.push(error.message));
        await page.setContent(html);
        await page.evaluate(p => window.resultBridge.apply(p), packet);
        for (const width of [1320, 760, 420]) {
          await page.setViewportSize({ width, height: 900 });
          await page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
          const result = await page.evaluate(() => {
            const drift = charts.drift.getOption();
            const shear = charts.shear.getOption();
            const visible = ['drift', 'shear'].map(kind => {
              const plot = document.getElementById(kind).getBoundingClientRect();
              const title = document.getElementById(kind + '-title').getBoundingClientRect();
              return { heading_above_plot: title.bottom <= plot.top, canvas_fits: charts[kind].getWidth() <= plot.width, right: plot.right };
            });
            return { overflow: document.documentElement.scrollWidth > innerWidth, visible,
              drift: drift.series.filter(s => s.id !== 'stored-reference').map(s => s.data), shear: shear.series[0].data,
              palette: drift.color.slice(0, 2), limit: drift.series.find(s => s.id === 'stored-reference'),
              legend_top: drift.legend[0].top, title: drift.title, x_name: drift.xAxis[0].name, y_name: drift.yAxis[0].name,
              unit: shear.xAxis[0].name, inverse: drift.yAxis[0].inverse,
              dom_title: document.getElementById('drift-title').textContent, full_title: document.getElementById('drift-title').title,
              notice: document.querySelector('.notice').textContent };
          });
          assert.equal(result.overflow, false);
          assert.ok(result.visible.every(v => v.heading_above_plot && v.canvas_fits && v.right <= width));
          assert.deepEqual(result.palette, ['#1f77b4', '#ff7f0e']);
          assert.deepEqual(result.drift, packet.drift.series.map(s => stories.map((story, i) => [s.values[i], story])));
          assert.deepEqual(result.shear, stories.map((story, i) => [packet.shear.series[0].values[i], story]));
          assert.equal(result.limit.lineStyle.color, '#cc0000');
          assert.equal(result.limit.lineStyle.type, 'dashed');
          assert.equal(result.limit.name, 'Stored allowable ratio');
          assert.deepEqual(result.limit.data, stories.map(story => [0.001, story]));
          assert.equal(result.legend_top, '1%');
          assert.ok(!result.title || result.title.length === 0, 'heading must be outside ECharts canvas');
          assert.equal(result.x_name, 'Drift Ratio');
          assert.equal(result.y_name, 'Story');
          assert.equal(result.unit, 'Shear (kip)');
          assert.equal(result.inverse, true);
          assert.equal(result.dom_title, packet.drift.title);
          assert.equal(result.full_title, packet.drift.title);
          assert.match(result.notice, /freshness.*not established/);
          cases.push({ width, browser_pixel_ratio: ratio, assessment: 'PASS' });
        }
        await page.setViewportSize({ width: 1320, height: 900 });
        await page.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
        const point = await page.evaluate(p => {
          const bounds = document.getElementById('drift').getBoundingClientRect();
          const value = charts.drift.convertToPixel({ seriesIndex: 0 }, [p.drift.series[0].values[10], p.drift.stories[10]]);
          return { x: bounds.left + value[0], y: bounds.top + value[1] };
        }, packet);
        await page.mouse.click(point.x, point.y);
        const message = await page.evaluate(() => messages.findLast(m => m.type === 'select_story'));
        assert.equal(message?.story, stories[10]);
        assert.equal(message?.source, packet.source);
        assert.equal(message?.generation, packet.generation);
        const retained = await page.evaluate(p => {
          charts.drift.dispatchAction({ type: 'dataZoom', dataZoomIndex: 0, start: 10, end: 80 });
          charts.drift.dispatchAction({ type: 'legendUnSelect', name: p.drift.series[1].name });
          window.resultBridge.apply({ ...p, selected_story: p.drift.stories[10] });
          const option = charts.drift.getOption();
          return { start: option.dataZoom[0].start, end: option.dataZoom[0].end, selected: option.legend[0].selected[p.drift.series[1].name], marker: option.series[0].markLine.data[0].yAxis, limit: option.series.find(s => s.id === 'stored-reference').data };
        }, packet);
        assert.equal(retained.start, 10);
        assert.equal(retained.end, 80);
        assert.equal(retained.selected, false);
        assert.equal(retained.marker, stories[10]);
        assert.deepEqual(retained.limit, stories.map(story => [0.001, story]));
        assert.deepEqual(errors, []);
        if (ratio === 1) {
          await page.evaluate(p => { charts.drift.dispatchAction({ type: 'dataZoom', start: 0, end: 100 }); charts.drift.dispatchAction({ type: 'legendSelect', name: p.drift.series[1].name }); }, packet);
          await page.mouse.move(1, 1);
          await page.evaluate(() => Object.values(charts).forEach(chart => chart.dispatchAction({ type: 'hideTip' })));
          await page.screenshot({ path: path.join(output, 'public-styled-charts.png') });
        }
      } finally { await context.close(); }
    }
  } finally {
    try { if (browser) await browser.close(); } finally { await server.close(); }
  }
  assert.ok(owned.exitCode !== null || owned.signalCode !== null);
  const report = { assessment: 'PASS', fixture: 'LAB-CACHED-STYLE-01-r1', cases,
    scope: 'Owned headless Chromium presentation/semantic events only; no private input, physical Windows DPI/input, native WebView, analysis parity or SDK lifecycle claim',
    values_order_null_signs_unchanged: true, selected_story_retains_zoom_legend_limit: true,
    headless_point_selection_message_checked: true, owned_browser_exit_observed: true };
  fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
})().catch(error => { console.error(error); process.exitCode = 1; });
