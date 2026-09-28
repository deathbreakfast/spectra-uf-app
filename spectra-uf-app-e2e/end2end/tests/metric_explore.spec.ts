import { test, expect, seedAuth, waitForHydrated, expectPermissionDenied } from "./fixtures";

test.describe("pw-spectra-metric-explore", () => {
  test("pw-spectra-metric-chart-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(
      `/spectra/metric/${encodeURIComponent(seeded.fixtures.metric_name)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-metric-explore-panel")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-metric-time-series-chart")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-metric-headline-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(
      `/spectra/metric/${encodeURIComponent(seeded.fixtures.metric_name)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    const stats = page.getByTestId("spectra-metric-headline-stats");
    await expect(stats).toBeVisible({ timeout: 60_000 });
    await expect(stats).toContainText(String(seeded.fixtures.seed_metric_value), {
      timeout: 60_000,
    });
  });

  test("pw-spectra-metric-time-range-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(
      `/spectra/metric/${encodeURIComponent(seeded.fixtures.metric_name)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await page.getByTestId("spectra-time-range-6h").click();
    await expect(page.getByTestId("spectra-metric-time-series-chart")).toBeVisible({
      timeout: 60_000,
    });
    // Selected preset uses Primary appearance; 1h should no longer be the only active control.
    await page.getByTestId("spectra-time-range-1h").click();
    await expect(page.getByTestId("spectra-metric-time-series-chart")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-metric-refresh-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(
      `/spectra/metric/${encodeURIComponent(seeded.fixtures.metric_name)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-metric-time-series-chart")).toBeVisible({
      timeout: 60_000,
    });
    await page.getByTestId("spectra-refresh-data").getByRole("button").click();
    await expect(page.getByText(/Last refreshed/i)).toBeVisible({ timeout: 60_000 });
    await expect(page).toHaveURL(
      new RegExp(`/spectra/metric/${seeded.fixtures.metric_name}/explore`),
    );
  });

  test("pw-spectra-metric-empty-happy", async ({ page }) => {
    // Never-written empty_metric + QueryTable grant → EmptyState (not permission denied).
    const seeded = await seedAuth(page, "admin", { skipData: true });
    await page.goto(
      `/spectra/metric/${encodeURIComponent(seeded.fixtures.empty_metric_name)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-metric-explore-panel")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-metric-empty-state")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-metric-permission-denied-sad", async ({ page }) => {
    const seeded = await seedAuth(page, "admin_noperms");
    await page.goto(
      `/spectra/metric/${encodeURIComponent(seeded.fixtures.metric_name)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expectPermissionDenied(page);
  });
});
