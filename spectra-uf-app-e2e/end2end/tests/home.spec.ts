import { test, expect, seedAuth, waitForHydrated, expandShellNav } from "./fixtures";

test.describe("pw-spectra-home", () => {
  test("pw-spectra-home-happy-load", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-home-page")).toBeVisible({ timeout: 60_000 });
    await expandShellNav(page);
    await expect(page.getByTestId("nav-spectra-home")).toBeVisible({ timeout: 30_000 });
    await expect(page.getByTestId("nav-spectra-schemas")).toBeVisible();
  });

  test("pw-spectra-home-stat-cards-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-stat-schemas")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-stat-event-tables")).toBeVisible();
    await expect(page.getByTestId("spectra-stat-metrics")).toBeVisible();
    await expect(page.getByTestId("spectra-stat-schemas")).not.toHaveText(/^0$/);
  });

  test("pw-spectra-home-activity-24h-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const tile = page.getByTestId("spectra-stat-24h-events");
    await expect(tile).toBeVisible({ timeout: 60_000 });
    await expect
      .poll(
        async () =>
          Number((await tile.innerText()).replace("24h event rows", "").replace(/\D/g, "")),
        { timeout: 60_000 },
      )
      .toBeGreaterThanOrEqual(seeded.fixtures.seeded_event_count);
  });

  test("pw-spectra-home-recent-schema-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(
      page.getByTestId(`spectra-schema-card-${seeded.fixtures.event_table}`),
    ).toBeVisible({ timeout: 60_000 });
  });

  test("pw-spectra-home-nav-schemas-happy", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expandShellNav(page);
    await page.getByTestId("nav-spectra-schemas").click();
    await expect(page.getByTestId("schema-index-page")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-home-quick-open-detail-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page
      .getByTestId("spectra-quick-open-search")
      .getByRole("searchbox")
      .fill(seeded.fixtures.event_table);
    await page.getByTestId("spectra-quick-open-detail").getByRole("button").click();
    await expect(page.getByTestId("spectra-schema-detail-page")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-home-quick-open-explore-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page
      .getByTestId("spectra-quick-open-search")
      .getByRole("searchbox")
      .fill(seeded.fixtures.event_table);
    await page.getByTestId("spectra-quick-open-explore").getByRole("button").click();
    await expect(page.getByTestId("spectra-event-explore-panel")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-home-quick-open-blank-sad", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page.getByTestId("spectra-quick-open-detail").getByRole("button").click();
    await expect(page.getByTestId("spectra-home-page")).toBeVisible();
    await expect(page.getByTestId("spectra-schema-detail-page")).toHaveCount(0);
  });
});
