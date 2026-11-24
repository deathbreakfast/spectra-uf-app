import type { Page } from "@playwright/test";
import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

const GROUP = "Checkout team";
const TITLE = "Release checks";

async function openDashboard(page: Page, id: string, query = "") {
  await page.goto(`/spectra/dashboards/${id}${query}`, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
  await expect(page.getByTestId("spectra-dashboard-page")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("spectra-dashboard-title")).toBeVisible({ timeout: 60_000 });
}

/** Seed admin with a new dashboard (versions 1 to 4, version 4 saved through MCP). */
async function freshDashboard(page: Page): Promise<string> {
  const seed = await seedAuth(page, "admin", { skipData: true, freshDashboard: true });
  expect(seed.fixtures.fresh_dashboard_id).toBeTruthy();
  return seed.fixtures.fresh_dashboard_id!;
}

async function moreAction(page: Page, name: string) {
  await page.getByTestId("spectra-dashboard-more").getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name }).click();
}

async function openShare(page: Page) {
  await page.getByTestId("spectra-dashboard-share").getByRole("button").click();
  const dialog = page.getByTestId("spectra-share-dialog");
  await expect(dialog).toBeVisible();
  return dialog;
}

async function shareWithGroupAsViewer(page: Page) {
  const dialog = await openShare(page);
  await dialog.getByTestId("spectra-share-viewers").locator("input").click();
  await dialog.getByTestId("spectra-share-viewers").locator("input").fill("Checkout");
  await page.getByRole("option", { name: GROUP }).click();
  await expect(dialog.getByTestId("spectra-share-viewers")).toContainText(GROUP);
  await dialog.getByTestId("spectra-share-save").getByRole("button").click();
  await expect(dialog).toBeHidden({ timeout: 30_000 });
}

async function openHistory(page: Page) {
  await moreAction(page, "Version history");
  const drawer = page.getByTestId("spectra-history-drawer");
  await expect(drawer).toBeVisible();
  return drawer;
}

test.describe("pw-spectra-dashboards-share", () => {
  test("pw-spectra-dashboards-share-group-viewer-opens-happy", async ({ page }) => {
    const id = await freshDashboard(page);
    await openDashboard(page, id);

    const dialog = await openShare(page);
    await expect(dialog.getByTestId("spectra-share-owner")).toContainText("admin");
    await expect(dialog.getByTestId("spectra-share-info")).toContainText(
      "Sharing opens the dashboard, not its tables. Someone without a table's grant sees that panel locked.",
    );
    await expect(dialog.getByTestId("spectra-share-copy-link")).toBeVisible();
    await dialog.getByTestId("spectra-share-close").getByRole("button").click();
    await expect(dialog).toBeHidden();

    await shareWithGroupAsViewer(page);

    // A member of the group opens it.
    await seedAuth(page, "teammate", { skipData: true });
    await openDashboard(page, id);
    await expect(page.getByTestId("spectra-dashboard-title")).toHaveText(TITLE);
  });

  test("pw-spectra-dashboards-share-viewer-cannot-change-sad", async ({ page }) => {
    const id = await freshDashboard(page);
    await openDashboard(page, id);
    await shareWithGroupAsViewer(page);

    await seedAuth(page, "teammate", { skipData: true });
    await openDashboard(page, id);
    const dialog = await openShare(page);
    await expect(dialog.getByTestId("spectra-share-owner")).toContainText("admin");
    await expect(dialog.getByTestId("spectra-share-your-access")).toHaveText("Can view");
    await expect(dialog.getByTestId("spectra-share-readonly")).toContainText(
      "Only the owner can change who has access.",
    );
    await expect(dialog.getByTestId("spectra-share-copy-link")).toBeVisible();
    await expect(dialog.getByTestId("spectra-share-save")).toHaveCount(0);
    await expect(dialog.getByTestId("spectra-share-viewers")).toHaveCount(0);
    // A viewer can't delete either.
    await dialog.getByTestId("spectra-share-close").getByRole("button").click();
    await expect(dialog).toBeHidden();
    await page.getByTestId("spectra-dashboard-more").getByRole("button", { name: "More actions" }).click();
    await expect(page.getByRole("menuitem", { name: "Version history" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Delete" })).toHaveCount(0);
  });

  test("pw-spectra-dashboards-share-restore-version-happy", async ({ page }) => {
    const id = await freshDashboard(page);
    await openDashboard(page, id);

    const drawer = await openHistory(page);
    await expect(drawer).toContainText(
      "Every save is kept for 180 days. Agents' saves show who they acted for.",
    );
    const newest = drawer.getByTestId("spectra-history-version-4");
    await expect(newest).toBeVisible({ timeout: 30_000 });
    await expect(newest.getByTestId("spectra-history-current")).toHaveText("Current");
    await expect(newest.getByTestId("spectra-history-via")).toHaveText("via MCP");
    await expect(newest).toContainText("Tightened the description");
    await expect(drawer.getByTestId("spectra-history-version-2").getByTestId("spectra-history-current")).toHaveCount(0);

    // View version 2 read-only.
    await drawer.getByTestId("spectra-history-version-2").getByTestId("spectra-history-view").getByRole("button").click();
    await expect(page).toHaveURL(/\?version=2$/);
    const bar = page.getByTestId("spectra-dashboard-version-bar");
    await expect(bar).toContainText("Version 2, saved by admin at", { timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-header")).toContainText("Version two of the checks.");
    await expect(page.getByTestId("spectra-dashboard-share")).toHaveCount(0);

    // Restore asks first, then version 2 comes back as version 5.
    await bar.getByTestId("spectra-dashboard-version-restore").getByRole("button").click();
    const confirm = page.getByTestId("spectra-restore-dialog");
    await expect(confirm.getByTestId("spectra-restore-text")).toHaveText(
      "Version 2 becomes version 5. Version 4, saved by admin, stays in the history.",
      { timeout: 30_000 },
    );
    await confirm.getByTestId("spectra-restore-confirm").getByRole("button").click();
    await expect(page).not.toHaveURL(/version=/, { timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-header")).toContainText("version 5", {
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-dashboard-header")).toContainText("Version two of the checks.");
    await expect(page.getByTestId("spectra-dashboard-version-bar")).toHaveCount(0);

    const after = await openHistory(page);
    const restored = after.getByTestId("spectra-history-version-5");
    await expect(restored).toBeVisible({ timeout: 30_000 });
    await expect(restored.getByTestId("spectra-history-current")).toHaveText("Current");
    await expect(restored).toContainText("Restored version 2");
    const oldNewest = after.getByTestId("spectra-history-version-4");
    await expect(oldNewest).toBeVisible();
    await expect(oldNewest.getByTestId("spectra-history-current")).toHaveCount(0);
  });

  test("pw-spectra-dashboards-share-delete-needs-name-sad", async ({ page }) => {
    const id = await freshDashboard(page);
    await openDashboard(page, id);

    await moreAction(page, "Delete");
    const dialog = page.getByTestId("spectra-delete-dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByTestId("spectra-delete-access")).toHaveText(
      "Nobody else has access. The dashboard and its version history are deleted for good.",
    );
    const submit = dialog.getByTestId("spectra-delete-submit").getByRole("button");
    const name = dialog.getByTestId("spectra-delete-name").locator("input");
    await expect(submit).toBeDisabled();
    await name.fill("Release check");
    await expect(submit).toBeDisabled();
    await name.fill("release checks");
    await expect(submit).toBeDisabled();
    await name.fill(TITLE);
    await expect(submit).toBeEnabled();
    await submit.click();
    await expect(page).toHaveURL(/\/spectra\/dashboards$/, { timeout: 60_000 });

    await page.goto(`/spectra/dashboards/${id}`, { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-dashboard-not-found")).toBeVisible({ timeout: 60_000 });
  });

  test("pw-spectra-dashboards-share-duplicate-unshared-happy", async ({ page }) => {
    const id = await freshDashboard(page);
    await openDashboard(page, id);
    await shareWithGroupAsViewer(page);

    await moreAction(page, "Duplicate");
    const dialog = page.getByTestId("spectra-duplicate-dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByTestId("spectra-duplicate-name").locator("input")).toHaveValue(
      `${TITLE} (copy)`,
    );
    await expect(dialog.getByTestId("spectra-duplicate-note")).toHaveText(
      "The copy is yours and isn't shared with anyone yet.",
    );
    await dialog.getByTestId("spectra-duplicate-submit").getByRole("button").click();
    await expect(page).not.toHaveURL(new RegExp(`/${id}$`), { timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-title")).toHaveText(`${TITLE} (copy)`, {
      timeout: 60_000,
    });

    const share = await openShare(page);
    await expect(share.getByTestId("spectra-share-owner")).toContainText("admin");
    await expect(share.getByTestId("spectra-share-save")).toBeVisible();
    await expect(share.getByTestId("spectra-share-viewers")).not.toContainText(GROUP);
    await expect(share.getByTestId("spectra-share-editors")).not.toContainText(GROUP);
  });

  test("pw-spectra-dashboards-share-phone-fits-happy", async ({ page }) => {
    const id = await freshDashboard(page);
    await page.setViewportSize({ width: 390, height: 844 });
    await openDashboard(page, id);

    const dialog = await openShare(page);
    const box = await dialog.boundingBox();
    expect(box).toBeTruthy();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(391);
    await dialog.getByTestId("spectra-share-close").getByRole("button").click();
    await expect(dialog).toBeHidden();

    const drawer = await openHistory(page);
    await expect(drawer.getByTestId("spectra-history-version-4")).toBeVisible({ timeout: 30_000 });
    const drawerBox = await drawer.boundingBox();
    expect(drawerBox!.x + drawerBox!.width).toBeLessThanOrEqual(391);
  });
});
