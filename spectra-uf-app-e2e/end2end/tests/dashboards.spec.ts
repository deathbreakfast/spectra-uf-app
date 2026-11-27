import type { Page } from "@playwright/test";
import { test, expect, seedAuth, waitForHydrated, expandShellNav } from "./fixtures";

const BUILTIN = "photon-throughput";
const EVENT_PERMISSION = "spectra.query.platform_smoke_event";

async function openList(page: Page) {
  await page.goto("/spectra/dashboards", { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
  await expect(page.getByTestId("spectra-dashboards-page")).toBeVisible({ timeout: 60_000 });
}

async function openDashboard(page: Page, id: string) {
  await page.goto(`/spectra/dashboards/${id}`, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
  await expect(page.getByTestId("spectra-dashboard-page")).toBeVisible({ timeout: 60_000 });
}

/** Tag an element so a later check can tell whether it was rebuilt. */
async function markNode(page: Page, testId: string) {
  await page.getByTestId(testId).evaluate((el) => el.setAttribute("data-pw-mark", "1"));
}

async function expectSameNode(page: Page, testId: string) {
  await expect(page.getByTestId(testId)).toHaveAttribute("data-pw-mark", "1");
}

function panel(page: Page, id: string) {
  return page.getByTestId(`spectra-dashboard-panel-${id}`);
}

test.describe("pw-spectra-dashboards", () => {
  test("pw-spectra-dashboards-list-tabs-happy", async ({ page }) => {
    const seed = await seedAuth(page, "admin");
    const checkout = seed.fixtures.checkout_dashboard_id;
    await openList(page);
    await markNode(page, "spectra-dashboards-page");

    const mine = page.getByTestId("spectra-dashboards-tab-mine");
    await expect(mine).toHaveText(/^Mine \(\d+\)$/, { timeout: 60_000 });
    await expect(page.getByTestId(`spectra-dashboard-row-${checkout}`)).toBeVisible();
    await expect(page.getByTestId(`spectra-dashboard-row-${BUILTIN}`)).toHaveCount(0);

    await page.getByTestId("spectra-dashboards-tab-shared").click();
    await expect(page.getByTestId("spectra-dashboards-tab-shared")).toHaveText(
      "Shared with me (0)",
    );
    await expect(page.getByTestId("spectra-dashboards-empty-shared")).toBeVisible();

    await page.getByTestId("spectra-dashboards-tab-all").click();
    const builtinRow = page.getByTestId(`spectra-dashboard-row-${BUILTIN}`);
    await expect(builtinRow).toBeVisible();
    await expect(page.getByTestId("spectra-dashboard-builtin-badge").first()).toBeVisible();
    await expect(page.getByTestId(`spectra-dashboard-row-${checkout}`)).toBeVisible();
    // Switching tabs keeps the page mounted.
    await expectSameNode(page, "spectra-dashboards-page");
  });

  test("pw-spectra-dashboards-list-search-happy", async ({ page }) => {
    const seed = await seedAuth(page, "admin");
    const checkout = seed.fixtures.checkout_dashboard_id;
    await openList(page);
    await page.getByTestId("spectra-dashboards-tab-all").click();
    const search = page.getByTestId("spectra-dashboards-search").locator("input");

    await search.fill("checkout HEALTH");
    await expect(page.getByTestId(`spectra-dashboard-row-${checkout}`)).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId(`spectra-dashboard-row-${BUILTIN}`)).toHaveCount(0);
    await expect(page.getByTestId("spectra-dashboards-tab-all")).toHaveText(/All I can read \(\d+\)/);

    await search.fill("no dashboard is called this");
    const empty = page.getByTestId("spectra-dashboards-empty-search");
    await expect(empty).toBeVisible();
    await empty.getByRole("button", { name: "Clear search" }).click();
    await expect(search).toHaveValue("");
    await expect(page.getByTestId(`spectra-dashboard-row-${BUILTIN}`)).toBeVisible();
  });

  test("pw-spectra-dashboards-new-dialog-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await openList(page);
    await page.getByTestId("spectra-dashboards-new").getByRole("button").click();
    const dialog = page.getByTestId("spectra-new-dashboard-dialog");
    await expect(dialog).toBeVisible();
    const create = page.getByTestId("spectra-new-dashboard-create").getByRole("button");
    await expect(create).toBeDisabled();
    await page.getByTestId("spectra-new-dashboard-name").locator("input").fill("Release watch");
    await page
      .getByTestId("spectra-new-dashboard-description")
      .locator("textarea")
      .fill("Errors after each deploy");
    await create.click();
    await expect(page).toHaveURL(/\/spectra\/dashboards\/[^/]+$/, { timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-title")).toHaveText("Release watch", {
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-dashboard-header")).toContainText("version 1");
  });

  test("pw-spectra-dashboards-checkout-panels-happy", async ({ page }) => {
    const seed = await seedAuth(page, "admin");
    await openDashboard(page, seed.fixtures.checkout_dashboard_id);

    await expect(page.getByTestId("spectra-dashboard-title")).toHaveText("Checkout health", {
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-dashboard-header")).toContainText(/Saved .+ · version \d+/);
    await expect(page.getByTestId("spectra-breadcrumbs")).toContainText("Checkout health");
    await expect(page.getByTestId("spectra-dashboard-missing-access")).toHaveCount(0);

    await expect(panel(page, "events").getByTestId("spectra-event-headline-stats")).toBeVisible({
      timeout: 60_000,
    });
    const bar = panel(page, "by-severity");
    await expect(bar.getByTestId("spectra-dashboard-panel-caption")).toHaveText(
      "platform_smoke_event · count by severity",
    );
    await expect(bar.getByTestId("spectra-event-bar-chart")).toBeVisible({ timeout: 60_000 });
    await expect(
      panel(page, "severity-share").getByTestId("spectra-event-pie-chart"),
    ).toBeVisible({ timeout: 60_000 });
    await expect(
      panel(page, "counter").getByTestId("spectra-metric-time-series-chart"),
    ).toBeVisible({ timeout: 60_000 });
    await expect(
      panel(page, "over-time").getByTestId("spectra-event-time-series-chart"),
    ).toBeVisible({ timeout: 60_000 });

    const table = panel(page, "newest").getByTestId("spectra-dashboard-table-panel");
    await expect(table).toContainText("Playwright seed row", { timeout: 60_000 });
    await expect(table.getByTestId("spectra-dashboard-table-summary")).toHaveText(
      /^Newest \d+ of \d+ rows?\.$/,
    );

    await expect(panel(page, "notes").getByTestId("spectra-dashboard-panel-markdown")).toContainText(
      "Runbook",
    );
    // The event table has no `host` field, so the `$host` variable can't apply there.
    await expect(bar.getByTestId("spectra-dashboard-panel-variable-note")).toContainText(
      "$host not applied",
    );
  });

  test("pw-spectra-dashboards-range-in-url-happy", async ({ page }) => {
    const seed = await seedAuth(page, "admin");
    await openDashboard(page, seed.fixtures.checkout_dashboard_id);
    await expect(panel(page, "counter").getByTestId("spectra-metric-time-series-chart")).toBeVisible(
      { timeout: 60_000 },
    );
    await markNode(page, "spectra-dashboard-header");

    await page.getByTestId("spectra-dashboard-range").locator("select").selectOption("24h");
    await expect(page).toHaveURL(/[?&]range=24h/);
    await expect(panel(page, "counter").getByTestId("spectra-metric-time-series-chart")).toBeVisible(
      { timeout: 60_000 },
    );
    // A toolbar change refetches panels; the page around them stays mounted.
    await expectSameNode(page, "spectra-dashboard-header");

    await page.getByTestId("spectra-dashboard-section-toggle").first().getByRole("button").click();
    await expect(page.getByTestId("spectra-dashboard-section-overview")).toHaveAttribute(
      "data-collapsed",
      "true",
    );
    await expect(panel(page, "events")).toHaveCount(0);
  });

  test("pw-spectra-dashboards-panel-explore-happy", async ({ page }) => {
    const seed = await seedAuth(page, "admin");
    await openDashboard(page, seed.fixtures.checkout_dashboard_id);
    const counter = panel(page, "counter");
    await expect(counter.getByTestId("spectra-metric-time-series-chart")).toBeVisible({
      timeout: 60_000,
    });
    await counter.getByRole("button", { name: "Panel actions" }).click();
    await page.getByRole("menuitem", { name: "Open in explore" }).click();
    await expect(page).toHaveURL(/\/spectra\/metric\/platform_smoke_counter\/explore\?window=3600/);
  });

  test("pw-spectra-dashboards-locked-panel-sad", async ({ page }) => {
    const seed = await seedAuth(page, "admin_partial");
    await openDashboard(page, seed.fixtures.checkout_dashboard_id);

    const warning = page.getByTestId("spectra-dashboard-missing-access");
    await expect(warning).toHaveCount(1, { timeout: 60_000 });
    await expect(warning).toContainText(EVENT_PERMISSION);
    await expect(page.getByRole("link", { name: "Request access" })).toHaveCount(1);

    const locked = panel(page, "by-severity");
    await expect(locked).toHaveAttribute("data-locked", "true");
    await expect(locked.getByTestId("spectra-dashboard-panel-locked")).toContainText(
      "platform_smoke_event",
    );
    await expect(locked.getByTestId("spectra-event-bar-chart")).toHaveCount(0);
    await expect(locked.getByTestId("spectra-dashboard-panel-loading")).toHaveCount(0);
    await expect(panel(page, "newest").getByTestId("spectra-dashboard-table-panel")).toHaveCount(0);

    // The metric is still granted, so its panel loads.
    await expect(
      panel(page, "counter").getByTestId("spectra-metric-time-series-chart"),
    ).toBeVisible({ timeout: 60_000 });
    await expect(panel(page, "counter")).toHaveAttribute("data-locked", "false");
  });

  test("pw-spectra-dashboards-not-found-sad", async ({ page }) => {
    await seedAuth(page, "admin");
    await openDashboard(page, "no-such-dashboard");
    await expect(page.getByTestId("spectra-dashboard-not-found")).toContainText(
      "Dashboard not found",
      { timeout: 60_000 },
    );
    const crumbs = page.getByTestId("spectra-breadcrumbs");
    await expect(crumbs).toContainText("Not found");
    await expect(crumbs).not.toContainText("no-such-dashboard");
    await page.getByTestId("spectra-dashboard-not-found-back").getByRole("link").click();
    await expect(page).toHaveURL(/\/spectra\/dashboards$/);
  });

  test("pw-spectra-dashboards-builtin-duplicate-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await openDashboard(page, BUILTIN);
    const bar = page.getByTestId("spectra-dashboard-builtin-bar");
    await expect(bar).toContainText("Built-in dashboards can't be changed.", { timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-duplicate")).toBeVisible();
    await expect(page.getByTestId("spectra-dashboard-edit")).toHaveCount(0);
    await expect(panel(page, "p1").getByTestId("spectra-metric-time-series-chart")).toBeVisible({
      timeout: 60_000,
    });

    await page.getByTestId("spectra-dashboard-builtin-duplicate").getByRole("button").click();
    const dialog = page.getByTestId("spectra-duplicate-dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByTestId("spectra-duplicate-name").locator("input")).toHaveValue(
      "Photon throughput (copy)",
    );
    await dialog.getByTestId("spectra-duplicate-submit").getByRole("button").click();
    await expect(page).not.toHaveURL(new RegExp(`/${BUILTIN}$`), { timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-title")).toHaveText(
      "Photon throughput (copy)",
      { timeout: 60_000 },
    );
    await expect(page.getByTestId("spectra-dashboard-builtin-bar")).toHaveCount(0);
  });

  test("pw-spectra-dashboards-phone-stacks-panels-happy", async ({ page }) => {
    const seed = await seedAuth(page, "admin");
    await page.setViewportSize({ width: 390, height: 844 });
    await openDashboard(page, seed.fixtures.checkout_dashboard_id);
    const grid = page.getByTestId("spectra-dashboard-section-overview").getByTestId(
      "spectra-dashboard-grid",
    );
    await expect(grid).toHaveAttribute("data-columns", "1", { timeout: 60_000 });
    const first = await panel(page, "events").boundingBox();
    const second = await panel(page, "by-severity").boundingBox();
    expect(first && second).toBeTruthy();
    expect(Math.abs(first!.x - second!.x)).toBeLessThan(2);
    expect(second!.y).toBeGreaterThan(first!.y + first!.height - 1);
    expect(second!.width).toBeGreaterThan(300);

    await page.setViewportSize({ width: 1280, height: 900 });
    await expect(grid).toHaveAttribute("data-columns", "12");
    const wideFirst = await panel(page, "events").boundingBox();
    const wideSecond = await panel(page, "by-severity").boundingBox();
    expect(wideSecond!.x).toBeGreaterThan(wideFirst!.x + wideFirst!.width - 1);
  });

  test("pw-spectra-dashboards-edit-button-opens-edit-happy", async ({ page }) => {
    const seed = await seedAuth(page, "admin");
    const checkout = seed.fixtures.checkout_dashboard_id;
    await openDashboard(page, checkout);
    await page.getByTestId("spectra-dashboard-edit").getByRole("button").click();
    await expect(page).toHaveURL(new RegExp(`/spectra/dashboards/${checkout}/edit$`), {
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-edit-bar")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-share")).toHaveCount(0);
    // Nothing changed, so Done goes straight back without asking.
    await page.getByTestId("spectra-edit-done").getByRole("button").click();
    await expect(page).toHaveURL(new RegExp(`/spectra/dashboards/${checkout}$`), {
      timeout: 60_000,
    });
  });

  test("pw-spectra-dashboards-nav-link-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expandShellNav(page);
    await page.getByTestId("nav-spectra-dashboards").click();
    await expect(page.getByTestId("spectra-dashboards-page")).toBeVisible({ timeout: 60_000 });
  });
});
