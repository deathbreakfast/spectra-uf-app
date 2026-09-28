import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

test.describe("pw-spectra-schemas", () => {
  test("pw-spectra-schema-index-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra/schema", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("schema-index-page")).toBeVisible({ timeout: 60_000 });
    await expect(
      page.getByTestId(`spectra-schema-card-${seeded.fixtures.event_table}`),
    ).toBeVisible({ timeout: 60_000 });
  });

  test("pw-spectra-schema-search-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra/schema", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page
      .getByTestId("spectra-schema-search")
      .getByRole("searchbox")
      .fill(seeded.fixtures.event_table);
    await expect(
      page.getByTestId(`spectra-schema-card-${seeded.fixtures.event_table}`),
    ).toBeVisible({ timeout: 60_000 });
  });

  test("pw-spectra-schema-search-no-match-sad", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/schema", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page
      .getByTestId("spectra-schema-search")
      .getByRole("searchbox")
      .fill("__no_such_schema__");
    await expect(page.getByTestId("spectra-schema-search-empty")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-schema-detail-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(`/spectra/schema/${encodeURIComponent(seeded.fixtures.event_table)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-schema-detail-page")).toBeVisible({
      timeout: 60_000,
    });
    await expect(
      page.getByTestId("spectra-schema-detail-page").getByText(seeded.fixtures.event_table),
    ).toBeVisible();
  });

  test("pw-spectra-schema-fields-table-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(`/spectra/schema/${encodeURIComponent(seeded.fixtures.event_table)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    const fields = page.getByTestId("spectra-schema-fields-table");
    await expect(fields).toBeVisible({ timeout: 60_000 });
    // Either real field rows or the empty-state copy when the DTO has no fields.
    const hasHeader = await fields.getByRole("columnheader", { name: /field/i }).count();
    if (hasHeader > 0) {
      await expect(fields.getByRole("columnheader", { name: /type/i })).toBeVisible();
      await expect(fields.getByRole("columnheader", { name: /classification/i })).toBeVisible();
    } else {
      await expect(fields.getByText(/No field metadata/i)).toBeVisible();
    }
  });

  test("pw-spectra-schema-detail-explore-cta-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(`/spectra/schema/${encodeURIComponent(seeded.fixtures.event_table)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await page.getByTestId("spectra-detail-open-explore").getByRole("button").click();
    await expect(page.getByTestId("spectra-event-explore-panel")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-schema-card-details-link-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra/schema", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page
      .getByTestId(`spectra-schema-card-${seeded.fixtures.event_table}-details`)
      .getByRole("link", { name: "Details" })
      .click();
    await expect(page).toHaveURL(
      new RegExp(`/spectra/schema/${seeded.fixtures.event_table}$`),
    );
    await expect(page.getByTestId("spectra-schema-detail-page")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-schema-card-explore-link-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/spectra/schema", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await page
      .getByTestId(`spectra-schema-card-${seeded.fixtures.event_table}-explore`)
      .getByRole("link", { name: "Explore" })
      .click();
    await expect(page).toHaveURL(
      new RegExp(`/spectra/schema/${seeded.fixtures.event_table}/explore$`),
    );
    await expect(page.getByTestId("spectra-event-explore-panel")).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-spectra-schema-unknown-sad", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/spectra/schema/__spectra_no_such_table__", {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByText(/Schema not found/i)).toBeVisible({
      timeout: 60_000,
    });
  });
});
