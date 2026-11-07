import { test, expect, seedAuth, waitForHydrated, expandShellNav } from "./fixtures";

// Seeded by `seed_spectra_rows`: two lines in checkout.rs, one in worker.rs, and legacy.rs
// two days back (outside the default 24h window, inside 7d).
const CHECKOUT_12 = "e2e/checkout.rs:12";
const CHECKOUT_48 = "e2e/checkout.rs:48";
const WORKER_7 = "e2e/worker.rs:7";
const LEGACY_3 = "e2e/legacy.rs:3";

test.describe("pw-spectra-callsite-graph", () => {
  test("pw-spectra-callsite-graph-sites-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("callsite-graph-page")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("callsite-graph")).toBeVisible({ timeout: 60_000 });
    for (const site of [CHECKOUT_12, CHECKOUT_48, WORKER_7]) {
      await expect(page.getByTestId(`callsite-site-${site}`)).toBeVisible();
    }
    await expect(
      page.getByTestId(`callsite-table-${seeded.fixtures.event_table}`),
    ).toBeVisible();
    await expect(page.getByTestId("callsite-graph-edges").locator("path")).toHaveCount(3, {
      timeout: 30_000,
    });
  });

  test("pw-spectra-callsite-graph-edge-list-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const row = page.getByTestId(`callsite-edge-${CHECKOUT_12}`);
    await expect(row).toBeVisible({ timeout: 60_000 });
    await expect(row).toContainText(seeded.fixtures.event_table);
    await expect(row).toContainText("rows, last");
  });

  test("pw-spectra-callsite-graph-table-link-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page
      .getByTestId(`callsite-table-${seeded.fixtures.event_table}`)
      .getByRole("link")
      .click();
    await expect(page.getByTestId("spectra-event-explore-panel")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-callsite-graph-window-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId(`callsite-site-${WORKER_7}`)).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId(`callsite-site-${LEGACY_3}`)).toHaveCount(0);
    await page.getByTestId("spectra-time-range-7d").click();
    await expect(page.getByTestId(`callsite-site-${LEGACY_3}`)).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-callsite-graph-untracked-notice-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    // The smoke rows seeded without a generated logger have no `_callsite`.
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

  test("pw-spectra-callsite-graph-no-table-access-sad", async ({ page }) => {
    await seedAuth(page, "admin_noperms");
    await page.goto("/spectra/graph", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("callsite-graph-empty")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId(`callsite-site-${WORKER_7}`)).toHaveCount(0);
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
