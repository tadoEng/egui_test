# Apache ECharts offline asset

- Version: **6.0.0**.
- Original runtime: <https://github.com/apache/echarts/blob/6.0.0/dist/echarts.min.js>.
- SHA-256: `baa8dfe7e1d9336b98e8986ba7e20ea15e7cdbea1ef42a59d59478632fa45a1d`.
- `.gitattributes` disables text conversion for this runtime so Windows Git
  checkouts retain the verified distribution bytes.
- On 2026-10-26, the vendored bytes were compared with the official Apache tag's
  raw distribution and matched this hash. No runtime CDN request is used.
- Upstream `LICENSE` and `NOTICE` are retained beside the runtime. ECharts and
  included third-party notices remain subject to those files.

The demo includes the runtime in the executable at build time and injects it
into an authored synthetic HTML template. It does not fetch scripts, fonts,
models or chart data from a remote service.
