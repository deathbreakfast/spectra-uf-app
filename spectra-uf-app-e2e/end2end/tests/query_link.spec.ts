import { test, expect, seedAuth, waitForHydrated, expectGridHasRows, selectEventView } from "./fixtures";

const WARN_ROW = "Warn severity row";
const TOOLTIP =
  "Copy a link to this view. Paste it into a dashboard panel to chart the same query.";

function eventLink(table: string, query: string): string {
  return `/spectra/schema/${encodeURIComponent(table)}/explore?${query}`;
}

test.describe("pw-spectra-query-link", () => {
  test("pw-spectra-query-link-opens-filtered-log-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(
      eventLink(
        seeded.fixtures.event_table,
        `window=1h&view=log&f=message:eq:${encodeURIComponent(WARN_ROW)}`,
      ),
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expectGridHasRows(page, 1);
    const body = page.getByTestId("spectra-event-data-grid").locator("tbody");
    await expect(body).toContainText(WARN_ROW, { timeout: 60_000 });
    await expect(body).not.toContainText(seeded.fixtures.seed_message);
    await expect(body.locator("tr")).toHaveCount(1);
    await expect(page.getByTestId("spectra-query-link-notice")).toHaveCount(0);
  });

  test("pw-spectra-query-link-view-change-updates-address-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(eventLink(seeded.fixtures.event_table, "window=1h&view=log"), {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expectGridHasRows(page, 1);
    const historyBefore = await page.evaluate(() => history.length);
    await selectEventView(page, "Bar chart");
    await expect(page).toHaveURL(/[?&]view=bar(&|$)/);
    await expect(page).toHaveURL(/[?&]window=1h(&|$)/);
    // Each change replaces the address instead of adding a history entry.
    expect(await page.evaluate(() => history.length)).toBe(historyBefore);
  });

  test("pw-spectra-query-link-copy-icon-tooltip-happy", async ({ page, context }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    const seeded = await seedAuth(page, "admin");
    await page.goto(eventLink(seeded.fixtures.event_table, "window=6h&view=log"), {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    const copy = page
      .getByTestId("spectra-copy-query-link")
      .getByRole("button", { name: "Copy link" });
    await expect(copy).toBeVisible({ timeout: 60_000 });
    await copy.hover();
    await expect(page.getByRole("tooltip")).toContainText(TOOLTIP);
    await copy.click();
    await expect
      .poll(async () => page.evaluate(() => navigator.clipboard.readText()))
      .toBe(page.url());
  });

  test("pw-spectra-query-link-unknown-field-notice-sad", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(
      eventLink(seeded.fixtures.event_table, "window=1h&view=log&f=status:eq:failed"),
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    const notice = page.getByTestId("spectra-query-link-notice");
    await expect(notice).toContainText("doesn't have: status", { timeout: 60_000 });
  });

  test("pw-spectra-query-link-bad-value-notice-sad", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(eventLink(seeded.fixtures.event_table, "window=2h"), {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-query-link-notice")).toContainText("window", {
      timeout: 60_000,
    });
    await expectGridHasRows(page, 1);
  });

  test("pw-spectra-query-link-metric-window-happy", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(
      `/spectra/metric/${encodeURIComponent(seeded.fixtures.metric_name)}/explore?window=24h`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-metric-time-series-chart")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-time-range-24h").getByRole("button")).toHaveClass(
      /primary/,
    );
    await expect(page.getByTestId("spectra-time-range-1h").getByRole("button")).not.toHaveClass(
      /primary/,
    );
    await expect(
      page.getByTestId("spectra-copy-query-link").getByRole("button", { name: "Copy link" }),
    ).toBeVisible();
    await page.getByTestId("spectra-time-range-6h").getByRole("button").click();
    await expect(page).toHaveURL(/[?&]window=6h(&|$)/);
  });
});
