// Render the design target with the preinstalled Playwright Chromium.
// node capture.cjs <dir-of-target.html>
const { chromium } = require("playwright");
const path = require("node:path");
const dir = path.resolve(process.argv[2] ?? ".");
const shots = [
  ["model-dark-1080p", "model,dark", 1920, 1080, 1],
  ["model-light-1080p", "model,light", 1920, 1080, 1],
  ["model-dark-125pct", "model,dark", 1536, 864, 1.25],
  ["model-dark-150pct", "model,dark", 1280, 720, 1.5],
  ["model-dark-900px", "model,dark,narrow", 900, 640, 1],
  ["model-dark-1080p-docked", "model,dark,docked", 1920, 1080, 1],
  ["model-dark-900px-docked", "model,dark,narrow,docked", 900, 640, 1],
  ["results-dark-1080p", "results,dark", 1920, 1080, 1],
  ["results-light-1080p", "results,light", 1920, 1080, 1],
  ["results-dark-900px", "results,dark,narrow", 900, 640, 1],
  ["results-illustrative-dark-1080p", "results,dark,illustrative", 1920, 1080, 1],
];
(async () => {
const browser = await chromium.launch();
for (const [name, hash, width, height, scale] of shots) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: scale });
  await page.goto(`file://${dir}/target.html#${hash}`);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(150);
  await page.screenshot({ path: `${dir}/renders/${name}.png` });
  await page.close();
}
await browser.close();
})();
