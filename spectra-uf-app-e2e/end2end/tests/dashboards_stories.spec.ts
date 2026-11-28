import type { Page } from "@playwright/test";
import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

/**
 * One test per approved dashboards story, each run at desktop and phone width.
 */

const EVENT_TABLE = "platform_smoke_event";
const METRIC = "platform_smoke_counter";
const ORDERS_TABLE = "e2e_checkout_orders";
const GROUP = "Checkout team";

const VIEWPORTS = [
  { name: "desktop", viewport: { width: 1280, height: 900 } },
  { name: "phone", viewport: { width: 390, height: 844 } },
] as const;

async function openDashboard(page: Page, id: string) {
  await page.goto(`/spectra/dashboards/${id}`, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
  await expect(page.getByTestId("spectra-dashboard-page")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("spectra-dashboard-title")).toBeVisible({ timeout: 60_000 });
}

async function openEdit(page: Page, id: string) {
  await page.goto(`/spectra/dashboards/${id}/edit`, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
  await expect(page.getByTestId("spectra-edit-bar")).toBeVisible({ timeout: 60_000 });
}

/** Seed admin with a new dashboard of its own (versions 1 to 4, no panels). */
async function freshDashboard(page: Page, skipData = true): Promise<string> {
  const seed = await seedAuth(page, "admin", { skipData, freshDashboard: true });
  expect(seed.fixtures.fresh_dashboard_id).toBeTruthy();
  return seed.fixtures.fresh_dashboard_id!;
}

async function editVersion(page: Page): Promise<number> {
  const shown = page.getByTestId("spectra-edit-version");
  await expect(shown).toHaveAttribute("data-version", /\d+/, { timeout: 60_000 });
  return Number(await shown.getAttribute("data-version"));
}

/** Click Save and wait for `version`; a save error shows up as the received value. */
async function saveAndExpectVersion(page: Page, version: number) {
  await page.getByTestId("spectra-edit-save").getByRole("button").click();
  const error = page.getByTestId("spectra-edit-save-error");
  const shown = page.getByTestId("spectra-edit-version");
  await expect
    .poll(
      async () =>
        (await error.count()) > 0
          ? `save error: ${await error.textContent()}`
          : await shown.getAttribute("data-version"),
      { timeout: 60_000 },
    )
    .toBe(String(version));
}

function editor(page: Page) {
  return page.getByTestId("spectra-panel-editor");
}

/** Add panel › `choice`, then wait for the panel editor. */
async function addPanel(page: Page, choice: "Build a panel" | "From a query link") {
  await page.getByTestId("spectra-edit-bar").getByRole("button", { name: "Add panel" }).click();
  await page.getByRole("menuitem", { name: choice }).click();
  await expect(editor(page)).toBeVisible({ timeout: 30_000 });
}

async function pickSource(page: Page, name: string) {
  const input = editor(page).getByTestId("spectra-panel-editor-source").locator("input");
  await input.click();
  await input.fill(name);
  await page.getByRole("option", { name, exact: true }).click();
  await expect(input).toHaveValue(name);
}

async function titleAndApply(page: Page, title: string) {
  await editor(page).getByTestId("spectra-panel-editor-title").locator("input").fill(title);
  const apply = editor(page).getByTestId("spectra-panel-editor-apply").getByRole("button");
  await expect(apply).toBeEnabled({ timeout: 30_000 });
  await apply.click();
  await expect(editor(page)).toHaveCount(0);
}

/** A saved panel on the dashboard page, by its title. */
function panelTitled(page: Page, title: string) {
  return page.locator('[data-testid^="spectra-dashboard-panel-"][data-chart]').filter({
    has: page.getByTestId("spectra-dashboard-panel-title").getByText(title, { exact: true }),
  });
}

async function moreAction(page: Page, name: string) {
  await page.getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name }).click();
}

for (const { name, viewport } of VIEWPORTS) {
  test.describe(`pw-spectra-dashboards-stories-${name}`, () => {
    // Each story seeds, edits and saves; the first load compiles WASM.
    test.describe.configure({ timeout: 240_000 });
    test.use({ viewport });

    test(`pw-spectra-dashboards-story-build-${name}-happy`, async ({ page }) => {
      await seedAuth(page, "admin");
      await page.goto("/spectra/dashboards", { waitUntil: "domcontentloaded" });
      await waitForHydrated(page);
      await page.getByTestId("spectra-dashboards-new").getByRole("button").click();
      await expect(page.getByTestId("spectra-new-dashboard-dialog")).toBeVisible();
      await page
        .getByTestId("spectra-new-dashboard-name")
        .locator("input")
        .fill(`Checkout health ${name}`);
      await page.getByTestId("spectra-new-dashboard-create").getByRole("button").click();
      await expect(page).toHaveURL(/\/spectra\/dashboards\/[^/]+$/, { timeout: 60_000 });
      const id = new URL(page.url()).pathname.split("/").pop()!;

      await openEdit(page, id);
      const version = await editVersion(page);

      // A bar chart on the event table.
      await addPanel(page, "Build a panel");
      await pickSource(page, EVENT_TABLE);
      await editor(page).getByTestId("spectra-panel-editor-chart-bar").getByRole("button").click();
      await editor(page)
        .getByTestId("spectra-panel-editor-group-by")
        .locator("select")
        .selectOption("message");
      await expect(
        editor(page).getByTestId("spectra-panel-editor-preview").getByTestId("spectra-event-bar-chart"),
      ).toBeVisible({ timeout: 60_000 });
      await titleAndApply(page, "Checkouts by message");

      // A time series on the metric.
      await addPanel(page, "Build a panel");
      await editor(page)
        .getByTestId("spectra-panel-editor-source-kind")
        .getByRole("tab", { name: "Metric" })
        .click();
      await pickSource(page, METRIC);
      await editor(page)
        .getByTestId("spectra-panel-editor-chart-time-series")
        .getByRole("button")
        .click();
      await titleAndApply(page, "Smoke counter");
      await saveAndExpectVersion(page, version + 1);

      // $host. The Variables dialog offers the fields of the sources saved when the editor
      // opened, so reopen it after saving the panels.
      await openEdit(page, id);
      await page.getByTestId("spectra-edit-variables").getByRole("button").click();
      const vars = page.getByTestId("spectra-edit-variables-dialog");
      await vars.getByTestId("spectra-edit-variables-add").getByRole("button").click();
      await vars.getByTestId("spectra-edit-variable-name").last().locator("input").fill("host");
      const done = vars.getByTestId("spectra-edit-variables-done").getByRole("button");
      await expect(done).toBeEnabled();
      await done.click();

      await saveAndExpectVersion(page, version + 2);
      await openDashboard(page, id);
      await expect(page.getByTestId("spectra-dashboard-variable-host")).toBeVisible();
      await expect(
        panelTitled(page, "Checkouts by message").getByTestId("spectra-event-bar-chart"),
      ).toBeVisible({ timeout: 60_000 });
      await expect(
        panelTitled(page, "Smoke counter").getByTestId("spectra-metric-time-series-chart"),
      ).toBeVisible({ timeout: 60_000 });
    });

    test(`pw-spectra-dashboards-story-paste-link-${name}-happy`, async ({ page }) => {
      const id = await freshDashboard(page, false);
      await openEdit(page, id);
      const version = await editVersion(page);

      // The link a teammate sent from the event explorer.
      await addPanel(page, "From a query link");
      await editor(page)
        .getByTestId("spectra-panel-editor-link")
        .locator("textarea")
        .fill(
          `http://localhost:3000/spectra/schema/${EVENT_TABLE}/explore?window=6h&view=bar&measure=count&group=message`,
        );
      await expect(editor(page).getByTestId("spectra-panel-editor-link-filled")).toContainText(
        `Event table ${EVENT_TABLE}, Bar, Count, grouped by message`,
        { timeout: 60_000 },
      );
      await titleAndApply(page, "From Daniel's link");

      await saveAndExpectVersion(page, version + 1);
      await openDashboard(page, id);
      await expect(
        panelTitled(page, "From Daniel's link").getByTestId("spectra-event-bar-chart"),
      ).toBeVisible({ timeout: 60_000 });
    });

    test(`pw-spectra-dashboards-story-table-masked-${name}-happy`, async ({ page }) => {
      const id = await freshDashboard(page, false);
      await openEdit(page, id);
      const version = await editVersion(page);

      await addPanel(page, "Build a panel");
      await pickSource(page, ORDERS_TABLE);
      await editor(page).getByTestId("spectra-panel-editor-chart-table").getByRole("button").click();
      const columns = editor(page).getByTestId("spectra-panel-editor-columns");
      for (const column of ["status", "customer_email"]) {
        await columns.getByRole("button", { name: "Add column" }).click();
        // The menu can open below the fold inside the editor drawer; a DOM click avoids
        // Playwright's viewport hit test.
        await page
          .getByRole("menuitem", { name: column, exact: true })
          .evaluate((el: HTMLElement) => el.click());
        await expect(columns).toContainText(column);
      }
      await titleAndApply(page, "Latest orders");

      await saveAndExpectVersion(page, version + 1);
      await openDashboard(page, id);
      const table = panelTitled(page, "Latest orders").getByTestId("spectra-dashboard-table-panel");
      await expect(table).toBeVisible({ timeout: 60_000 });
      await expect(table.getByTestId("spectra-dashboard-masked-note")).toContainText(
        "customer_email is personal data, so it's hidden here.",
      );
      await expect(table.getByTestId("spectra-dashboard-masked-badge").first()).toBeVisible();
      await expect(table).toContainText("paid");
      await expect(table).not.toContainText("@example.com");
    });

    test(`pw-spectra-dashboards-story-shared-locked-${name}-happy`, async ({ page }) => {
      // Admin's seed writes the metric's points; teammate opens what admin shared.
      await seedAuth(page, "admin");
      const seed = await seedAuth(page, "teammate", { skipData: true });
      await openDashboard(page, seed.fixtures.shared_story_dashboard_id);
      await expect(page.getByTestId("spectra-dashboard-title")).toHaveText("Sign-in fixes");

      const warning = page.getByTestId("spectra-dashboard-missing-access");
      await expect(warning).toContainText(`spectra.query.${ORDERS_TABLE}`, { timeout: 60_000 });
      await expect(page.locator('[data-locked="true"]')).toHaveCount(1);
      const locked = page.getByTestId("spectra-dashboard-panel-orders");
      await expect(locked).toHaveAttribute("data-locked", "true");
      await expect(locked.getByTestId("spectra-dashboard-panel-locked")).toContainText(ORDERS_TABLE);
      await expect(locked.getByTestId("spectra-event-bar-chart")).toHaveCount(0);

      const counter = page.getByTestId("spectra-dashboard-panel-counter");
      await expect(counter).toHaveAttribute("data-locked", "false");
      await expect(counter.getByTestId("spectra-metric-time-series-chart")).toBeVisible({
        timeout: 60_000,
      });

      const request = page.getByTestId("spectra-dashboard-request-access").getByRole("link", {
        name: "Request access",
      });
      // The link opens Gauge's request form on that permission's record.
      await expect(request).toHaveAttribute("href", /^\/permission\/permissions\/[^?]+\?request=1$/);
    });

    test(`pw-spectra-dashboards-story-share-restore-${name}-happy`, async ({ page }) => {
      const id = await freshDashboard(page);
      await openDashboard(page, id);

      // Share with the group as viewers.
      await page.getByTestId("spectra-dashboard-share").getByRole("button").click();
      const dialog = page.getByTestId("spectra-share-dialog");
      await expect(dialog).toBeVisible();
      const viewers = dialog.getByTestId("spectra-share-viewers").locator("input");
      await viewers.click();
      await viewers.fill("Checkout");
      await page.getByRole("option", { name: GROUP }).click();
      await expect(dialog.getByTestId("spectra-share-viewers")).toContainText(GROUP);
      await dialog.getByTestId("spectra-share-save").getByRole("button").click();
      await expect(dialog).toBeHidden({ timeout: 30_000 });

      // Undo the agent's save: bring version 2 back.
      await moreAction(page, "Version history");
      const drawer = page.getByTestId("spectra-history-drawer");
      await expect(drawer.getByTestId("spectra-history-version-4")).toContainText("via MCP", {
        timeout: 30_000,
      });
      await drawer
        .getByTestId("spectra-history-version-2")
        .getByTestId("spectra-history-view")
        .getByRole("button")
        .click();
      await expect(page).toHaveURL(/\?version=2$/);
      await page
        .getByTestId("spectra-dashboard-version-bar")
        .getByTestId("spectra-dashboard-version-restore")
        .getByRole("button")
        .click();
      await page
        .getByTestId("spectra-restore-dialog")
        .getByTestId("spectra-restore-confirm")
        .getByRole("button")
        .click();
      await expect(page).not.toHaveURL(/version=/, { timeout: 60_000 });
      await expect(page.getByTestId("spectra-dashboard-header")).toContainText("version 5", {
        timeout: 60_000,
      });
      await expect(page.getByTestId("spectra-dashboard-header")).toContainText(
        "Version two of the checks.",
      );

      // A group member opens the restored dashboard.
      await seedAuth(page, "teammate", { skipData: true });
      await openDashboard(page, id);
      await expect(page.getByTestId("spectra-dashboard-header")).toContainText(
        "Version two of the checks.",
      );
    });
  });
}
