# Screenshot-driven chart resize regression

The owner supplied a synthetic LAB-DASH-01 screenshot showing consistent story23 selection in the explorer/table/two chart markers, alongside a horizontal WebView scrollbar, clipped right-side chart furniture and a Story axis caption overlapping the plot heading. A still image does not establish which selection direction was exercised or pass mouse/DPI/focus gates. No private analysis input is used in this regression.

The grid's automatic minimum sizes retained the canvas's old width after a viewport shrink. Default canvas/DOM rounding also left a vertical overflow. The layout now uses zero minimum grid/item sizes and bounded page dimensions; charts resize to their actual allocation. Story captions move to the middle of the y-axis, legends below the plot, redundant right-end unit captions are removed (units remain in each chart title), reference labels move inside the grid and tooltips are confined. Numerical packets, selection protocol, thresholds and engineering logic are unchanged.

Optional headless test (Node20+, separately locked Playwright1.62.1):

```powershell
npm ci --prefix tests/chart-layout
npm exec --prefix tests/chart-layout -- playwright install chromium
npm test --prefix tests/chart-layout
```

The test loads only the local bundled ECharts/HTML and authored synthetic packet. It reproduces the pre-fix overflow/caption overlap at source9f1e27c, then checks1100→832→420→900pixel resize sequences at browser device pixel ratios1/1.25/1.5, canvas/container agreement, no document overflow, heading/axis separation and selection-only zoom/legend preservation. These are **headless Chromium ratios, not Windows display-DPI acceptance**. It creates only owned browser contexts/server, closes them in finally and observes the owned browser process exit before writing PASS. Generated reports/images stay under ignored `output/playwright/`; no owner app/shared browser is closed.

Native WebView2 visual confirmation remains pending; this test does not close #2/#5/#28, prove actual mouse input, certify cleanup of WebView2 subprocesses or resolve #18 packaging. The real-results viewer requested later is a separate optional read-only artifact-consumption track, not a change to the synthetic fixture or public input policy.
