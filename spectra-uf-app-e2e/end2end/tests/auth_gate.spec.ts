import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

test.describe("pw-spectra-auth-gate", () => {
  test("pw-spectra-auth-gate-sad-anonymous", async ({ page }) => {
    await seedAuth(page, "anonymous");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("auth-required-empty-state")).toBeAttached({
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-home-page")).toHaveCount(0);
  });

  test("pw-spectra-auth-gate-happy-admin", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-app-root")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("spectra-home-page")).toBeVisible({ timeout: 60_000 });
  });

  test("pw-spectra-auth-outsider-schema-index-sad", async ({ page }) => {
    await seedAuth(page, "outsider");
    await page.goto("/spectra/schema", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("permission-required-empty-state")).toBeAttached({
      timeout: 60_000,
    });
  });

  test("pw-spectra-auth-unverified-schema-index-sad", async ({ page }) => {
    await seedAuth(page, "unverified");
    await page.goto("/spectra/schema", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("permission-required-empty-state")).toBeAttached({
      timeout: 60_000,
    });
  });

  test("pw-spectra-guest-schema-index-sad", async ({ page }) => {
    await page.goto("/spectra/schema", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("auth-required-empty-state")).toBeAttached({
      timeout: 60_000,
    });
  });

  test("pw-spectra-guest-event-explore-sad", async ({ page }) => {
    await page.goto("/spectra/schema/platform_smoke_event/explore", {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("auth-required-empty-state")).toBeAttached({
      timeout: 60_000,
    });
  });

  test("pw-spectra-guest-metric-explore-sad", async ({ page }) => {
    await page.goto("/spectra/metric/platform_smoke_counter/explore", {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("auth-required-empty-state")).toBeAttached({
      timeout: 60_000,
    });
  });

  test("pw-spectra-explore-admin-noperms-sad", async ({ page }) => {
    const seeded = await seedAuth(page, "admin_noperms");
    await page.goto(
      `/spectra/schema/${encodeURIComponent(seeded.fixtures.event_table)}/explore`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-permission-denied")).toBeVisible({
      timeout: 60_000,
    });
  });
});
