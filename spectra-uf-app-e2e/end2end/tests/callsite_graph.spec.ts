import type { Page } from "@playwright/test";
import { test, expect, seedAuth, waitForHydrated, expandShellNav } from "./fixtures";

// Seeded by `seed_spectra_rows`: two lines in checkout.rs, one in worker.rs, and legacy.rs
// two days back (outside the default 24h window, inside 7d).
const CHECKOUT_12 = "e2e/checkout.rs:12";
const CHECKOUT_48 = "e2e/checkout.rs:48";
const WORKER_7 = "e2e/worker.rs:7";
const LEGACY_3 = "e2e/legacy.rs:3";
// Seeded by `seed_metric_callsite`: one metric point stamped like a generated recorder.
const METRICS_5 = "e2e/metrics.rs:5";

/** An Orbital Graph node by id, inside the callsite graph. */
function node(page: Page, id: string) {
  return page.getByTestId("callsite-graph").locator(`[data-node-id="${id}"]`);
}

const tableSite = (site: string) => `site:table:${site}`;
const metricSite = (site: string) => `site:metric:${site}`;

test.describe("pw-spectra-callsite-graph", () => {
  test("pw-spectra-callsite-graph-sites-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("callsite-graph-page")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("callsite-graph")).toBeVisible({ timeout: 60_000 });
    for (const site of [CHECKOUT_12, CHECKOUT_48, WORKER_7]) {
      await expect(node(page, tableSite(site))).toBeVisible();
    }
    await expect(node(page, `table:${seeded.fixtures.event_table}`)).toBeVisible();
    await expect(node(page, metricSite(METRICS_5))).toBeVisible();
    await expect(node(page, `metric:${seeded.fixtures.metric_name}`)).toBeVisible();
    await expect(
      page.getByTestId("callsite-graph").locator('path[data-edge-from^="site:table:"]'),
    ).toHaveCount(3, { timeout: 30_000 });
    await expect(
      page.getByTestId("callsite-graph").locator('path[data-edge-from^="site:metric:"]'),
    ).toHaveCount(1);
  });

  test("pw-spectra-callsite-graph-edge-list-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const row = page.getByTestId(`callsite-edge-${CHECKOUT_12}`);
    await expect(row).toBeVisible({ timeout: 60_000 });
    await expect(row).toContainText(`writes ${seeded.fixtures.event_table}`);
    await expect(row).toContainText("rows, last");
    const metricRow = page.getByTestId(`callsite-edge-${METRICS_5}`);
    await expect(metricRow).toContainText(`records ${seeded.fixtures.metric_name}`);
    await expect(metricRow).toContainText("e2e::metrics");
  });

  test("pw-spectra-callsite-graph-table-link-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const link = page.getByTestId(`callsite-edge-${CHECKOUT_12}`).getByRole("link");
    // The explorer opens on the graph's window (24h by default), not its own last hour.
    await expect(link).toHaveAttribute("href", /\/explore\?window=24h$/, { timeout: 60_000 });
    await link.click();
    await expect(page.getByTestId("spectra-event-explore-panel")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page).toHaveURL(/window=24h/);
  });

  test("pw-spectra-callsite-graph-show-metrics-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(node(page, tableSite(CHECKOUT_12))).toBeVisible({ timeout: 60_000 });
    await page.getByTestId("callsite-show-metrics").getByRole("button").click();
    await expect(page.getByTestId("callsite-show-metrics").getByRole("button")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await expect(node(page, metricSite(METRICS_5))).toBeVisible();
    await expect(node(page, tableSite(CHECKOUT_12))).toHaveCount(0);
    await expect(page.getByTestId(`callsite-edge-${CHECKOUT_12}`)).toHaveCount(0);
    await expect(page.getByTestId(`callsite-edge-${METRICS_5}`)).toBeVisible();
    // Show only hides sets; the page never reloads the graph.
    await expect(page.getByText("Loading callsites…")).toHaveCount(0);
    await page.getByTestId("callsite-show-both").getByRole("button").click();
    await expect(node(page, tableSite(CHECKOUT_12))).toBeVisible();
  });

  test("pw-spectra-callsite-graph-window-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(node(page, tableSite(WORKER_7))).toBeVisible({
      timeout: 60_000,
    });
    await expect(node(page, tableSite(LEGACY_3))).toHaveCount(0);
    await page.getByTestId("spectra-time-range-7d").click();
    await expect(node(page, tableSite(LEGACY_3))).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-callsite-graph-untracked-notice-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    // The smoke rows seeded without a `_callsite` are counted, not drawn.
    await expect(page.getByTestId("callsite-graph-untracked")).toContainText(
      "have no callsite",
      { timeout: 60_000 },
    );
  });

  test("pw-spectra-callsite-graph-nav-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expandShellNav(page);
    await page.getByTestId("nav-spectra-graph").click();
    await expect(page.getByTestId("callsite-graph-page")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("spectra-breadcrumbs")).toContainText("Callsites");
  });

  test("pw-spectra-callsite-graph-no-grants-sad", async ({ page }) => {
    await seedAuth(page, "admin_noperms");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const empty = page.getByTestId("callsite-graph-empty");
    await expect(empty).toBeVisible({ timeout: 60_000 });
    await expect(empty).toHaveAttribute("data-reason", "no_grants");
    await expect(empty).toContainText("You can't read any tables or metrics yet");
    await expect(empty.getByTestId("callsite-request-access")).toBeVisible();
    await expect(page.getByTestId("callsite-graph")).toHaveCount(0);
  });

  test("pw-spectra-callsite-graph-anonymous-sad", async ({ page }) => {
    await seedAuth(page, "anonymous");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("auth-required-empty-state")).toBeAttached({
      timeout: 60_000,
    });
    await expect(page.getByTestId("callsite-graph-page")).toHaveCount(0);
  });
});
