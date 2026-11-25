import type { Page } from "@playwright/test";
import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

const BUILTIN = "photon-throughput";

async function openDashboard(page: Page, id: string) {
  await page.goto(`/spectra/dashboards/${id}`, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
  await expect(page.getByTestId("spectra-dashboard-page")).toBeVisible({ timeout: 60_000 });
}

async function openEdit(page: Page, id: string) {
  await page.goto(`/spectra/dashboards/${id}/edit`, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
  await expect(page.getByTestId("spectra-edit-bar")).toBeVisible({ timeout: 60_000 });
}

/**
 * Duplicate the seeded Checkout health dashboard so each test edits its own copy and the
 * shared seed keeps its layout for dashboards.spec.ts.
 */
async function freshCopy(page: Page): Promise<{ id: string; version: number }> {
  const seed = await seedAuth(page, "admin");
  await openDashboard(page, seed.fixtures.checkout_dashboard_id);
  await expect(page.getByTestId("spectra-dashboard-title")).toHaveText("Checkout health", {
    timeout: 60_000,
  });
  await page.getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Duplicate" }).click();
  await page.getByTestId("spectra-duplicate-submit").getByRole("button").click();
  await expect(page.getByTestId("spectra-dashboard-title")).toHaveText(
    "Checkout health (copy)",
    { timeout: 60_000 },
  );
  const id = new URL(page.url()).pathname.split("/").pop()!;
  const header = (await page.getByTestId("spectra-dashboard-header").textContent()) ?? "";
  const version = Number(/version (\d+)/.exec(header)![1]);
  return { id, version };
}

function cell(page: Page, panelId: string) {
  return page.getByTestId(`spectra-edit-panel-${panelId}`);
}

function handle(page: Page, panelId: string) {
  return cell(page, panelId).getByTestId("spectra-edit-panel-handle").getByRole("button");
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

function bar(page: Page) {
  return page.getByTestId("spectra-edit-bar");
}

test.describe("pw-spectra-dashboards-edit", () => {
  test.describe.configure({ timeout: 180_000 });

  test("pw-spectra-dashboards-edit-keyboard-move-save-happy", async ({ page }) => {
    const { id, version } = await freshCopy(page);
    await page.getByTestId("spectra-dashboard-edit").getByRole("button").click();
    await expect(page).toHaveURL(new RegExp(`/spectra/dashboards/${id}/edit$`), {
      timeout: 60_000,
    });
    await expect(bar(page)).toBeVisible({ timeout: 60_000 });
    const crumbs = page.getByTestId("spectra-breadcrumbs");
    await expect(crumbs).toContainText("Checkout health (copy)");
    await expect(crumbs).toContainText("Edit");
    await expect(page.getByTestId("spectra-edit-version")).toHaveAttribute(
      "data-version",
      String(version),
    );

    await expect(cell(page, "events")).toHaveAttribute("data-col", "0");
    // Moving and saving change the draft in place; the page itself is never rebuilt.
    await page
      .getByTestId("spectra-dashboard-edit-page")
      .evaluate((el) => el.setAttribute("data-pw-mark", "1"));
    await handle(page, "events").press("ArrowRight");
    await expect(cell(page, "events")).toHaveAttribute("data-col", "1");
    await expect(handle(page, "events")).toBeFocused();
    // The moved panel now covers part of its neighbour, which drops below it.
    await expect(cell(page, "by-severity")).toHaveAttribute("data-row", "4");
    await handle(page, "events").press("Shift+ArrowRight");
    await expect(cell(page, "events")).toHaveAttribute("data-span", "5");
    await expect(page.getByTestId("spectra-edit-unsaved")).toHaveText("Unsaved changes");

    await saveAndExpectVersion(page, version + 1);
    await expect(page.getByTestId("spectra-edit-unsaved")).toHaveCount(0);
    await expect(page.getByTestId("spectra-edit-saved")).toBeVisible();
    await expect(page.getByTestId("spectra-dashboard-edit-page")).toHaveAttribute(
      "data-pw-mark",
      "1",
    );

    await page.getByTestId("spectra-edit-done").getByRole("button").click();
    await expect(page).toHaveURL(new RegExp(`/spectra/dashboards/${id}$`), { timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-header")).toContainText(
      `version ${version + 1}`,
      { timeout: 60_000 },
    );

    await openEdit(page, id);
    await expect(cell(page, "events")).toHaveAttribute("data-col", "1");
    await expect(cell(page, "events")).toHaveAttribute("data-span", "5");
  });

  test("pw-spectra-dashboards-edit-conflict-keeps-changes-sad", async ({ page, browser }) => {
    const { id, version } = await freshCopy(page);
    await openEdit(page, id);
    await handle(page, "events").press("ArrowDown");
    await expect(cell(page, "events")).toHaveAttribute("data-row", "1");

    // A second session saves first.
    const other = await browser.newContext({ baseURL: test.info().project.use.baseURL });
    try {
      const second = await other.newPage();
      await seedAuth(second, "admin");
      await openEdit(second, id);
      await handle(second, "notes").press("ArrowDown");
      await expect(cell(second, "notes")).toHaveAttribute("data-row", "1");
      await saveAndExpectVersion(second, version + 1);

      await page.getByTestId("spectra-edit-save").getByRole("button").click();
      const conflict = page.getByTestId("spectra-edit-conflict");
      await expect(conflict).toBeVisible({ timeout: 60_000 });
      await expect(conflict).toHaveAttribute("data-current", String(version + 1));
      await expect(conflict).toContainText(`saved version ${version + 1}`);
      await expect(conflict).toContainText(`while you were editing version ${version}`);
      await expect(conflict).toContainText("Your changes are still here.");
      await expect(cell(page, "events")).toHaveAttribute("data-row", "1");
      await expect(page.getByTestId("spectra-edit-unsaved")).toBeVisible();
      await expect(page.getByTestId("spectra-edit-version")).toHaveAttribute(
        "data-version",
        String(version),
      );

      await page.getByTestId("spectra-edit-conflict-compare").getByRole("button").click();
      const compare = page.getByTestId("spectra-edit-compare-dialog");
      await expect(compare.getByTestId("spectra-edit-compare-mine")).toContainText(
        "Moved one panel",
      );
      await expect(compare.getByTestId("spectra-edit-compare-theirs")).toContainText(
        "Moved one panel",
        { timeout: 60_000 },
      );
      await compare.getByRole("button", { name: "Close" }).click();

      // Nothing was overwritten: the stored dashboard is the second session's save.
      await openEdit(second, id);
      await expect(second.getByTestId("spectra-edit-version")).toHaveAttribute(
        "data-version",
        String(version + 1),
      );
      await expect(cell(second, "notes")).toHaveAttribute("data-row", "1");
      await expect(cell(second, "events")).toHaveAttribute("data-row", "0");
    } finally {
      await other.close();
    }
  });

  test("pw-spectra-dashboards-edit-discard-happy", async ({ page }) => {
    const { id, version } = await freshCopy(page);
    await openEdit(page, id);
    await handle(page, "events").press("ArrowDown");
    await expect(cell(page, "events")).toHaveAttribute("data-row", "1");

    await page.getByTestId("spectra-edit-discard").getByRole("button").click();
    const dialog = page.getByTestId("spectra-edit-discard-dialog");
    await expect(dialog).toContainText("Discard your changes?");
    await expect(dialog).toContainText("You moved one panel.");
    await expect(dialog).toContainText(`goes back to version ${version}`);
    await dialog.getByTestId("spectra-edit-keep-editing").getByRole("button").click();
    await expect(cell(page, "events")).toHaveAttribute("data-row", "1");

    await page.getByTestId("spectra-edit-discard").getByRole("button").click();
    await dialog.getByTestId("spectra-edit-confirm").getByRole("button").click();
    await expect(page).toHaveURL(new RegExp(`/spectra/dashboards/${id}$`), { timeout: 60_000 });
    await expect(page.getByTestId("spectra-dashboard-header")).toContainText(
      `version ${version}`,
      { timeout: 60_000 },
    );

    await openEdit(page, id);
    await expect(cell(page, "events")).toHaveAttribute("data-row", "0");
  });

  test("pw-spectra-dashboards-edit-leave-asks-first-happy", async ({ page }) => {
    const { id } = await freshCopy(page);
    await openEdit(page, id);
    await handle(page, "events").press("ArrowDown");
    const crumbs = page.getByTestId("spectra-breadcrumbs");

    await crumbs.getByRole("link", { name: "Dashboards" }).click();
    const dialog = page.getByTestId("spectra-edit-leave-dialog");
    await expect(dialog).toContainText("Leave without saving?");
    await dialog.getByTestId("spectra-edit-keep-editing").getByRole("button").click();
    await expect(page).toHaveURL(new RegExp(`/${id}/edit$`));
    await expect(cell(page, "events")).toHaveAttribute("data-row", "1");

    await crumbs.getByRole("link", { name: "Dashboards" }).click();
    await dialog.getByTestId("spectra-edit-confirm").getByRole("button").click();
    await expect(page).toHaveURL(/\/spectra\/dashboards$/, { timeout: 60_000 });
  });

  test("pw-spectra-dashboards-edit-text-and-variables-happy", async ({ page }) => {
    const { id, version } = await freshCopy(page);
    await openEdit(page, id);

    await page.getByTestId("spectra-edit-add-text").getByRole("button").click();
    const text = page.getByTestId("spectra-edit-text-dialog");
    const apply = text.getByTestId("spectra-edit-text-apply").getByRole("button");
    await expect(apply).toBeDisabled();
    await text.getByTestId("spectra-edit-text-title").locator("input").fill("On call?");
    await text
      .getByTestId("spectra-edit-text-markdown")
      .locator("textarea")
      .fill("Page the checkout team if **failures** climb.");
    await apply.click();
    await expect(cell(page, "panel-1")).toContainText("On call?");

    await page.getByTestId("spectra-edit-variables").getByRole("button").click();
    const vars = page.getByTestId("spectra-edit-variables-dialog");
    await expect(vars.getByTestId("spectra-edit-variable-name").locator("input")).toHaveValue(
      "host",
    );
    // The seeded event table has no host field, so its panels ignore $host.
    await expect(vars.getByTestId("spectra-edit-variables-ignored")).toContainText(
      "no host field, so $host doesn't change",
    );
    await vars.getByTestId("spectra-edit-variables-add").getByRole("button").click();
    const done = vars.getByTestId("spectra-edit-variables-done").getByRole("button");
    await expect(vars.getByTestId("spectra-edit-variables-problem")).toContainText(
      "Every variable needs a name.",
    );
    await expect(done).toBeDisabled();
    await vars.getByTestId("spectra-edit-variable-name").nth(1).locator("input").fill("severity");
    await expect(done).toBeEnabled();
    await done.click();

    await saveAndExpectVersion(page, version + 1);
    await expect(page.getByTestId("spectra-dashboard-variable-severity")).toBeVisible();
  });

  test("pw-spectra-dashboards-edit-preview-as-viewer-happy", async ({ page }) => {
    const { id } = await freshCopy(page);
    await openEdit(page, id);
    await page.getByRole("button", { name: "Preview as viewer" }).click();
    await page.getByRole("menuitem", { name: /A viewer without spectra\.query\./ }).first().click();
    const preview = page.getByTestId("spectra-edit-preview");
    await expect(preview).toContainText("This is how a viewer without spectra.query.");
    await expect(preview.getByTestId("spectra-dashboard-panel-locked").first()).toBeVisible({
      timeout: 60_000,
    });
    await preview.getByTestId("spectra-edit-preview-back").getByRole("button").click();
    await expect(page.getByTestId("spectra-edit-grid").first()).toBeVisible();
  });

  test("pw-spectra-dashboards-edit-phone-reorder-happy", async ({ page }) => {
    const { id, version } = await freshCopy(page);
    await page.setViewportSize({ width: 390, height: 844 });
    await openEdit(page, id);

    await expect(page.getByTestId("spectra-edit-phone-note")).toContainText(
      "Drag and resize need a wider screen. Here you can reorder, edit, add and remove panels.",
    );
    await expect(page.getByTestId("spectra-edit-grid")).toHaveCount(0);
    const list = page.getByTestId("spectra-edit-list-overview");
    await expect(list.getByTestId("spectra-edit-row-events")).toHaveAttribute("data-index", "0");
    await expect(list.getByTestId("spectra-edit-row-by-severity")).toHaveAttribute(
      "data-index",
      "1",
    );

    await list
      .getByTestId("spectra-edit-row-by-severity")
      .getByTestId("spectra-edit-row-up")
      .getByRole("button")
      .click();
    await expect(list.getByTestId("spectra-edit-row-by-severity")).toHaveAttribute(
      "data-index",
      "0",
    );
    await expect(list.getByTestId("spectra-edit-row-events")).toHaveAttribute("data-index", "1");

    await saveAndExpectVersion(page, version + 1);
  });

  test("pw-spectra-dashboards-edit-builtin-not-found-sad", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto(`/spectra/dashboards/${BUILTIN}/edit`, { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("spectra-dashboard-not-found")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId("spectra-edit-bar")).toHaveCount(0);
  });
});
