import {
  test,
  expect,
  seedAuth,
  waitForHydrated,
  openEventExplore,
  selectEventView,
  expectPermissionDenied,
  expectGridHasRows,
  expectEventChartHasData,
  expectEventChartEmpty,
  expectNoOverlappingLabels,
  fillGroupBy,
} from "./fixtures";

test.describe("pw-spectra-event-explore", () => {
  test("pw-spectra-event-explore-layout-side-by-side-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.setViewportSize({ width: 1280, height: 800 });
    await openEventExplore(page, seeded.fixtures.event_table);
    const controls = page.getByTestId("spectra-event-explore-controls");
    const viewport = page.getByTestId("spectra-event-explore-viewport-pane");
    await expect(controls).toBeVisible({ timeout: 60_000 });
    await expect(viewport).toBeVisible();
    const c = await controls.boundingBox();
    const v = await viewport.boundingBox();
    expect(c).toBeTruthy();
    expect(v).toBeTruthy();
    // Desktop: controls left of viewport (same row), not stacked above.
    expect(c!.x + c!.width).toBeLessThanOrEqual(v!.x + 8);
    expect(Math.abs(c!.y - v!.y)).toBeLessThan(80);
  });

  test("pw-spectra-event-explore-layout-narrow-stack-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.setViewportSize({ width: 480, height: 900 });
    await openEventExplore(page, seeded.fixtures.event_table);
    const controls = page.getByTestId("spectra-event-explore-controls");
    const viewport = page.getByTestId("spectra-event-explore-viewport-pane");
    await expect(controls).toBeVisible({ timeout: 60_000 });
    await expect(viewport).toBeVisible();
    const c = await controls.boundingBox();
    const v = await viewport.boundingBox();
    expect(c).toBeTruthy();
    expect(v).toBeTruthy();
    // Narrow: viewport sits under controls.
    expect(v!.y).toBeGreaterThan(c!.y + c!.height - 24);
  });

  test("pw-spectra-event-chart-toolbar-no-thead-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Time series");
    await expectEventChartHasData(page, "time_series", { minTotal: 3 });
    const grid = page.getByTestId("spectra-event-data-grid");
    await expect(grid).toBeVisible();
    // Chart mode hides thead/scroll host (CSS until Orbital show_table_grid ships on host).
    await expect(grid.locator(".orbital-data-table__scroll-host")).toBeHidden();
    await expect(grid.getByRole("columnheader")).toHaveCount(0);
  });

  test("pw-spectra-event-log-empty-table-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", { skipData: true });
    await page.goto(
      `/spectra/schema/${encodeURIComponent(seeded.fixtures.event_table)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    const grid = page.getByTestId("spectra-event-data-grid");
    await expect(grid).toBeVisible({ timeout: 60_000 });
    await expect(grid.getByRole("columnheader", { name: /message/i })).toBeVisible();
  });

  test("pw-spectra-event-log-seeded-row-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(
      `/spectra/schema/${encodeURIComponent(seeded.fixtures.event_table)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expectGridHasRows(page, seeded.fixtures.seeded_event_count);
  });

  test("pw-spectra-event-explore-time-range-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await expectGridHasRows(page, 1);
    await expect(page.getByTestId("spectra-event-time-range")).toBeVisible();
    await page.getByTestId("spectra-refresh-data").getByRole("button").click();
    await expectGridHasRows(page, 1);
  });

  test("pw-spectra-event-refresh-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await expectGridHasRows(page, 1);
    await page.getByTestId("spectra-refresh-data").getByRole("button").click();
    await expect(page.getByText(/Last refreshed/i)).toBeVisible({ timeout: 60_000 });
    await expect(page).toHaveURL(
      new RegExp(`/spectra/schema/${seeded.fixtures.event_table}/explore`),
    );
  });

  test("pw-spectra-event-view-timeseries-data-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Time series");
    await expectEventChartHasData(page, "time_series", { minTotal: 3 });
  });

  test("pw-spectra-event-view-bar-chart-data-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Bar chart");
    await fillGroupBy(page, "severity");
    await expectEventChartHasData(page, "bar", {
      minTotal: 2,
      labels: ["info", "warn"],
    });
  });

  test("pw-spectra-event-view-pie-chart-data-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Pie chart");
    await fillGroupBy(page, "severity");
    // Pie marks use values, not band tick labels — assert data via headline + SVG marks.
    await expectEventChartHasData(page, "pie", { minTotal: 2 });
  });

  test("pw-spectra-event-aggregate-sum-data-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Time series");
    await page.getByTestId("spectra-aggregation-measure").locator("select").selectOption("sum");
    const field = page.getByTestId("spectra-aggregation-measure-field").locator("select");
    await field.selectOption("value");
    await page.getByTestId("spectra-event-see").getByRole("button").click();
    await expectEventChartHasData(page, "time_series", { minTotal: 18 });
  });

  test("pw-spectra-event-view-pie-need-groupby-sad", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Pie chart");
    await expectEventChartEmpty(page, "need_group_by");
  });

  test("pw-spectra-event-view-bar-need-groupby-sad", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Bar chart");
    await expectEventChartEmpty(page, "need_group_by");
  });

  test("pw-spectra-event-view-timeseries-empty-table-sad", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", { skipData: true });
    await page.goto(
      `/spectra/schema/${encodeURIComponent(seeded.fixtures.empty_event_table)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await selectEventView(page, "Time series");
    await expectEventChartEmpty(page, "no_series");
  });

  test("pw-spectra-event-view-pie-empty-groupby-sad", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Pie chart");
    // Leave group-by on "(select field)" — empty slices / need group-by empty state.
    await expectEventChartEmpty(page, "need_group_by");
  });

  test("pw-spectra-event-explore-permission-denied-sad", async ({ page }) => {
    const seeded = await seedAuth(page, "admin_noperms");
    await page.goto(
      `/spectra/schema/${encodeURIComponent(seeded.fixtures.event_table)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expectPermissionDenied(page);
  });

  test("pw-spectra-event-chart-stat-row-gap-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.setViewportSize({ width: 1280, height: 800 });
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Time series");
    await expectEventChartHasData(page, "time_series", { minTotal: 3 });
    const stats = page.getByTestId("spectra-event-headline-stats");
    const chart = page.getByTestId("spectra-event-time-series-chart");
    const [statsBox, chartBox] = await Promise.all([stats.boundingBox(), chart.boundingBox()]);
    expect(statsBox).toBeTruthy();
    expect(chartBox).toBeTruthy();
    // Chart sits below the stat cards with a real gap, not flush against them.
    const gap = chartBox!.y - (statsBox!.y + statsBox!.height);
    expect(gap).toBeGreaterThan(4);
  });

  test("pw-spectra-event-chart-fills-container-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Time series");
    await expectEventChartHasData(page, "time_series", { minTotal: 3 });
    const chartHost = page.getByTestId("spectra-event-time-series-chart").locator("svg.orb-chart-svg");

    await page.setViewportSize({ width: 1400, height: 900 });
    await expect
      .poll(async () => (await chartHost.boundingBox())?.width ?? 0, { timeout: 15_000 })
      .toBeGreaterThan(0);
    const wideBox = await chartHost.boundingBox();

    await page.setViewportSize({ width: 760, height: 900 });
    await expect
      .poll(async () => (await chartHost.boundingBox())?.width ?? 0, { timeout: 15_000 })
      .toBeLessThan(wideBox!.width);
  });

  test("pw-spectra-event-view-timeseries-dense-buckets-no-overlap-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await openEventExplore(page, seeded.fixtures.event_table);
    await selectEventView(page, "Time series");
    // 150s buckets over the default 1-hour range -> up to 24 buckets, the dense case that
    // used to draw every tick label horizontal/full-width and overlap into unreadable text.
    const bucketInput = page.getByTestId("spectra-aggregation-bucket").locator("input");
    await bucketInput.fill("150");
    await page.getByTestId("spectra-event-see").getByRole("button").click();
    await expectEventChartHasData(page, "time_series", { minTotal: 3 });
    const chart = page.getByTestId("spectra-event-time-series-chart");
    await expect
      .poll(async () => chart.locator(".orb-axis-tick-label").count(), { timeout: 15_000 })
      .toBeGreaterThan(4);
    await expectNoOverlappingLabels(chart);
  });
});
