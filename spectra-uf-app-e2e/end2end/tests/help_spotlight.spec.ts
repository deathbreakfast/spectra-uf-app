import { test, expect, seedAuth, waitForHydrated } from "./fixtures";
import type { Page } from "@playwright/test";

async function completeVisibleTour(page: Page) {
  const footer = page.locator('[data-testid="spotlight-footer"]:visible');
  const next = footer.getByTestId("spotlight-tour-next");
  await expect(footer).toBeVisible({ timeout: 60_000 });
  for (let i = 0; i < 24; i++) {
    if ((await footer.count()) === 0) {
      break;
    }
    // Spotlight panels can sit partially off-screen; DOM click avoids Playwright
    // viewport hit-testing failures that still occur with { force: true }.
    await next.evaluate((el: HTMLElement) => el.click());
    try {
      await expect(footer).toHaveCount(0, { timeout: 2_000 });
      break;
    } catch {
      /* more steps */
    }
  }
  await expect(footer).toHaveCount(0, { timeout: 30_000 });
}

test.describe("help-spotlight", () => {
  test("help-spotlight-skips-when-seeded", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-home-page")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId("help-step-spectra-intro")).toHaveCount(0);
    await expect(page.locator('[data-testid="spotlight-footer"]:visible')).toHaveCount(
      0,
    );
  });

  test("help-spotlight-skips-auth-gate", async ({ page }) => {
    await seedAuth(page, "anonymous", { help_tour: true });
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("auth-required-empty-state")).toBeAttached({
      timeout: 60_000,
    });
    await expect(page.getByTestId("help-step-spectra-intro")).toHaveCount(0);
    await expect(page.locator('[data-testid="spotlight-footer"]:visible')).toHaveCount(
      0,
    );
  });

  test("help-spotlight-home-green", async ({ page }) => {
    await seedAuth(page, "admin", { help_tour: true });
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("help-step-spectra-intro")).toBeVisible({
      timeout: 60_000,
    });
    await completeVisibleTour(page);
    await expect(page.getByTestId("help-step-spectra-intro")).toHaveCount(0);

    await page.reload({ waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("help-step-spectra-intro")).toHaveCount(0);
  });

  test("help-spotlight-schema-index-green", async ({ page }) => {
    await seedAuth(page, "admin", { help_tour: true });
    await page.goto("/spectra/schema", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("help-step-spectra-schema-search")).toBeVisible({
      timeout: 60_000,
    });
    await completeVisibleTour(page);
  });

  test("help-spotlight-schema-detail-green", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", { help_tour: true });
    const name = seeded.fixtures.event_table;
    await page.goto(`/spectra/schema/${encodeURIComponent(name)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("help-step-spectra-detail-meta")).toBeVisible({
      timeout: 60_000,
    });
    await completeVisibleTour(page);
  });

  test("help-spotlight-event-explore-green", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", { help_tour: true });
    const name = seeded.fixtures.event_table;
    await page.goto(`/spectra/schema/${encodeURIComponent(name)}/explore`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("help-step-spectra-event-intro")).toBeVisible({
      timeout: 60_000,
    });
    await completeVisibleTour(page);
  });

  test("help-spotlight-metric-explore-green", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", { help_tour: true });
    const name = seeded.fixtures.metric_name;
    await page.goto(`/spectra/metric/${encodeURIComponent(name)}/explore`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("help-step-spectra-metric-intro")).toBeVisible({
      timeout: 60_000,
    });
    await completeVisibleTour(page);
  });

  test("help-spotlight-dashboards-list-green", async ({ page }) => {
    await seedAuth(page, "admin", { skipData: true, help_tour: true });
    await page.goto("/spectra/dashboards", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("help-step-spectra-dashboards-tabs")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.locator("#spectra-dashboards-tabs")).toHaveCount(1);
    await expect(page.locator("#spectra-dashboards-new")).toHaveCount(1);

    const next = page
      .locator('[data-testid="spotlight-footer"]:visible')
      .getByTestId("spotlight-tour-next");
    await next.evaluate((el: HTMLElement) => el.click());
    await expect(page.getByTestId("help-step-spectra-dashboards-new")).toContainText(
      "A new dashboard is private until you share it.",
    );
    await next.evaluate((el: HTMLElement) => el.click());
    await expect(page.getByTestId("help-step-spectra-dashboards-nav")).toContainText(
      "Dashboards holds saved views, and Callsites shows which code writes each one.",
    );
    await completeVisibleTour(page);
  });

  test("help-spotlight-dashboard-detail-green", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", { help_tour: true });
    await page.goto(`/spectra/dashboards/${seeded.fixtures.checkout_dashboard_id}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("help-step-spectra-dashboard-toolbar")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId("help-step-spectra-dashboard-toolbar")).toContainText(
      "$host: a variable",
    );
    await expect(page.locator("#spectra-dashboard-toolbar")).toHaveCount(1);
    await expect(page.locator("#spectra-dashboard-panels")).toHaveCount(1);
    await completeVisibleTour(page);
  });

  test("help-spotlight-dashboard-edit-green", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", {
      skipData: true,
      freshDashboard: true,
      help_tour: true,
    });
    await page.goto(`/spectra/dashboards/${seeded.fixtures.fresh_dashboard_id}/edit`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("help-step-spectra-dashboard-grid")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.locator("#spectra-dashboard-grid")).toHaveCount(1);
    await expect(page.locator("#spectra-dashboard-add-panel")).toHaveCount(1);
    await completeVisibleTour(page);
  });
});
