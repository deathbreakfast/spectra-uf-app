import { test, expect, seedAuth, waitForHydrated, expandShellNav } from "./fixtures";

const PHOTON_PUBLISHES = "spectra-dashboard-panel-photon_publishes";
const PHOTON_BACKLOG = "spectra-dashboard-panel-photon_backlog";

test.describe("pw-spectra-dashboards", () => {
  test("pw-spectra-dashboards-index-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/dashboards", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-dashboards-page")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-card-photon-throughput")).toBeVisible();
    await expect(page.getByTestId("spectra-dashboard-card-valence-db-activity")).toBeVisible();
  });

  test("pw-spectra-dashboards-open-from-index-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/dashboards", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page
      .getByTestId("spectra-dashboard-card-valence-db-activity-open")
      .getByRole("link")
      .click();
    await expect(page.getByTestId("spectra-dashboard-page")).toBeVisible({ timeout: 60_000 });
    await expect(page).toHaveURL(/\/spectra\/dashboards\/valence-db-activity$/);
  });

  test("pw-spectra-dashboards-photon-panels-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/dashboards/photon-throughput", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const panel = page.getByTestId(PHOTON_PUBLISHES);
    await expect(panel).toBeVisible({ timeout: 60_000 });
    await expect(panel.getByTestId("spectra-metric-time-series-chart")).toBeVisible({
      timeout: 60_000,
    });
    await expect(panel.getByTestId("spectra-metric-headline-stats")).toBeVisible();
  });

  test("pw-spectra-dashboards-valence-panels-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/dashboards/valence-db-activity", {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    const panel = page.getByTestId("spectra-dashboard-panel-valence_db_reads");
    await expect(panel.getByTestId("spectra-metric-time-series-chart")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-dashboards-unseeded-panel-empty-sad", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/dashboards/photon-throughput", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const panel = page.getByTestId(PHOTON_BACKLOG);
    await expect(panel.getByText("No series data")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId(PHOTON_PUBLISHES)).toBeVisible();
  });

  test("pw-spectra-dashboards-panel-explore-link-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/dashboards/photon-throughput", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page.getByTestId(`${PHOTON_PUBLISHES}-explore`).getByRole("link").click();
    await expect(page.getByTestId("spectra-metric-explore-panel")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page).toHaveURL(/\/spectra\/metric\/photon_publishes\/explore$/);
  });

  test("pw-spectra-dashboards-unknown-id-sad", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/dashboards/nope", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-dashboard-not-found")).toBeVisible({
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

  test("pw-spectra-dashboards-permission-denied-sad", async ({ page }) => {
    await seedAuth(page, "admin_noperms");
    await page.goto("/spectra/dashboards/photon-throughput", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const panel = page.getByTestId(PHOTON_PUBLISHES);
    await expect(panel.getByTestId("spectra-permission-denied")).toBeVisible({
      timeout: 60_000,
    });
  });
});
