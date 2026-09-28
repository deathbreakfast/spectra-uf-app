import { test as base, expect, type Locator, type Page } from "@playwright/test";

export type SeedAuthKind =
  | "anonymous"
  | "admin"
  | "admin_noperms"
  | "outsider"
  | "unverified";

export type SeedFixtures = {
  event_table: string;
  empty_event_table: string;
  metric_name: string;
  empty_metric_name: string;
  seeded_event_count: number;
  seed_message: string;
  seed_metric_value: number;
};

export type SeedResponse = {
  ok: boolean;
  auth: string;
  fixtures: SeedFixtures;
};

/** All Spectra Help inventory keys — seed as seen so non-tour specs stay quiet. */
const SPECTRA_HELP_STEPS_SEEN = [
  { route: "/spectra", feature_highlight: "spectra-intro", spotlight: null, replay: false },
  {
    route: "/spectra",
    feature_highlight: "spectra-dashboard-stats",
    spotlight: "spectra-dashboard-stats",
    replay: false,
  },
  {
    route: "/spectra",
    feature_highlight: "spectra-home-recent",
    spotlight: "spectra-home-recent",
    replay: false,
  },
  {
    route: "/spectra",
    feature_highlight: "spectra-home-view-all",
    spotlight: "spectra-home-view-all",
    replay: false,
  },
  {
    route: "/spectra",
    feature_highlight: "spectra-quick-open-search",
    spotlight: "spectra-quick-open-search",
    replay: false,
  },
  {
    route: "/spectra",
    feature_highlight: "spectra-quick-open-detail",
    spotlight: "spectra-quick-open-detail",
    replay: false,
  },
  {
    route: "/spectra",
    feature_highlight: "spectra-quick-open-explore",
    spotlight: "spectra-quick-open-explore",
    replay: false,
  },
  {
    route: "/spectra",
    feature_highlight: "spectra-nav",
    spotlight: "spectra-nav",
    replay: false,
  },
  {
    route: "/spectra/schema",
    feature_highlight: "spectra-schema-search",
    spotlight: "spectra-schema-search",
    replay: false,
  },
  {
    route: "/spectra/schema",
    feature_highlight: "spectra-schema-grid",
    spotlight: "spectra-schema-grid",
    replay: false,
  },
  {
    route: "/spectra/schema",
    feature_highlight: "spectra-schema-open-details",
    spotlight: "spectra-schema-open-details",
    replay: false,
  },
  {
    route: "/spectra/schema",
    feature_highlight: "spectra-schema-open-explore",
    spotlight: "spectra-schema-open-explore",
    replay: false,
  },
  {
    route: "/spectra/schema",
    feature_highlight: "spectra-schema-nav",
    spotlight: "spectra-nav",
    replay: false,
  },
  {
    route: "/spectra/schema/:name",
    feature_highlight: "spectra-detail-meta",
    spotlight: "spectra-detail-meta",
    replay: false,
  },
  {
    route: "/spectra/schema/:name",
    feature_highlight: "spectra-detail-open-explore",
    spotlight: "spectra-detail-open-explore",
    replay: false,
  },
  {
    route: "/spectra/schema/:name",
    feature_highlight: "spectra-detail-nav",
    spotlight: "spectra-nav",
    replay: false,
  },
  {
    route: "/spectra/schema/:name/explore",
    feature_highlight: "spectra-event-intro",
    spotlight: null,
    replay: false,
  },
  {
    route: "/spectra/schema/:name/explore",
    feature_highlight: "spectra-event-time-range",
    spotlight: "spectra-event-time-range",
    replay: false,
  },
  {
    route: "/spectra/schema/:name/explore",
    feature_highlight: "spectra-event-view-picker",
    spotlight: "spectra-event-view-picker",
    replay: false,
  },
  {
    route: "/spectra/schema/:name/explore",
    feature_highlight: "spectra-aggregation-measure",
    spotlight: "spectra-aggregation-measure",
    replay: false,
  },
  {
    route: "/spectra/schema/:name/explore",
    feature_highlight: "spectra-aggregation-bucket",
    spotlight: "spectra-aggregation-bucket",
    replay: false,
  },
  {
    route: "/spectra/schema/:name/explore",
    feature_highlight: "spectra-aggregation-group-by",
    spotlight: "spectra-aggregation-group-by",
    replay: false,
  },
  {
    route: "/spectra/schema/:name/explore",
    feature_highlight: "spectra-event-viewport",
    spotlight: "spectra-event-explore-viewport",
    replay: false,
  },
  {
    route: "/spectra/schema/:name/explore",
    feature_highlight: "spectra-event-nav",
    spotlight: "spectra-nav",
    replay: false,
  },
  {
    route: "/spectra/metric/:name/explore",
    feature_highlight: "spectra-metric-intro",
    spotlight: null,
    replay: false,
  },
  {
    route: "/spectra/metric/:name/explore",
    feature_highlight: "spectra-metric-time-range",
    spotlight: "spectra-metric-time-range",
    replay: false,
  },
  {
    route: "/spectra/metric/:name/explore",
    feature_highlight: "spectra-metric-results",
    spotlight: "spectra-metric-results",
    replay: false,
  },
  {
    route: "/spectra/metric/:name/explore",
    feature_highlight: "spectra-metric-nav",
    spotlight: "spectra-nav",
    replay: false,
  },
] as const;

export async function seedAuth(
  page: Page,
  auth: SeedAuthKind,
  opts?: { skipData?: boolean; help_tour?: boolean },
) {
  const helpTour = opts?.help_tour ?? false;
  await page.addInitScript(
    ([enableTour, seenSteps]) => {
      try {
        if (enableTour) {
          if (!sessionStorage.getItem("uf.help.e2e_tour_cleared")) {
            localStorage.removeItem("uf.help.tour_steps");
            sessionStorage.setItem("uf.help.e2e_tour_cleared", "1");
          }
          return;
        }
        localStorage.setItem("uf.help.tour_steps", JSON.stringify(seenSteps));
      } catch {
        /* ignore */
      }
    },
    [helpTour, SPECTRA_HELP_STEPS_SEEN] as const,
  );

  const res = await page.request.post("/api/test/seed-data", {
    data: { auth, skip_data: opts?.skipData ?? false },
  });
  expect(res.ok()).toBeTruthy();
  return res.json() as Promise<SeedResponse>;
}

async function bootState(page: Page): Promise<"ready" | "error" | "loading"> {
  return page.evaluate(() => {
    const html = document.documentElement;
    if (html.getAttribute("data-orbital-hydrated") === "true") {
      return "ready";
    }
    if (html.getAttribute("data-orbital-boot-state") === "error") {
      return "error";
    }
    return "loading";
  });
}

/**
 * CI evidence (spectra run 33828489095): Orbital sets boot `error` from a
 * non-WASM `unhandledrejection` matching bare `fetch`, then dismiss refuses.
 * Artifacts show SSR `main` under "Unable to load" with wasm already complete
 * — or error firing early while wasm still downloads.
 *
 * Do not gate on `e2e-auth-bootstrap` (Suspense server resource). When wasm is
 * complete, clear the error bit and re-invoke dismiss.
 */
async function clearFalsePositiveBootError(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const html = document.documentElement;
    if (html.getAttribute("data-orbital-hydrated") === "true") {
      return true;
    }
    if (html.getAttribute("data-orbital-boot-state") !== "error") {
      return false;
    }
    const progress = (
      window as unknown as {
        __orbitalBootProgress?: { steps?: { wasm?: string } };
        __orbitalBootDismissOverlay?: () => void;
      }
    );
    const wasmComplete =
      progress.__orbitalBootProgress?.steps?.wasm === "complete" ||
      document.querySelectorAll(".orbital-boot-step--complete").length >= 4;
    const shellReady = !!document.querySelector("main");
    if (!wasmComplete || !shellReady) {
      return false;
    }
    html.removeAttribute("data-orbital-boot-state");
    if (typeof progress.__orbitalBootDismissOverlay === "function") {
      progress.__orbitalBootDismissOverlay();
    }
    if (html.getAttribute("data-orbital-hydrated") !== "true") {
      html.setAttribute("data-orbital-hydrated", "true");
      document.getElementById("orbital-boot-overlay")?.remove();
    }
    return true;
  });
}

/**
 * Wait for Orbital hydrate. On terminal boot `error`, wait for wasm to finish
 * under a false-positive error before reloading. Never reload while `loading`.
 */
export async function waitForHydrated(page: Page, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  let refreshes = 0;
  const maxRefreshes = 3;

  while (Date.now() < deadline) {
    const state = await bootState(page);
    if (state === "ready") {
      break;
    }
    if (state === "error") {
      if (await clearFalsePositiveBootError(page)) {
        break;
      }
      const waitUntil = Math.min(Date.now() + 30_000, deadline);
      let recovered = false;
      while (Date.now() < waitUntil) {
        await page.waitForTimeout(500);
        if ((await bootState(page)) === "ready") {
          recovered = true;
          break;
        }
        if (await clearFalsePositiveBootError(page)) {
          recovered = true;
          break;
        }
      }
      if (recovered) {
        break;
      }
      if (refreshes >= maxRefreshes) {
        break;
      }
      refreshes += 1;
      // Let Chromium release a failed compile before retrying the ~50–100MiB wasm.
      await page.waitForTimeout(1_500);
      await page.reload({ waitUntil: "load" });
      continue;
    }
    await page.waitForTimeout(500);
  }

  if ((await bootState(page)) === "error") {
    await clearFalsePositiveBootError(page);
  }

  await expect
    .poll(async () => bootState(page), { timeout: 10_000 })
    .toBe("ready");
  await expect(page.getByTestId("orbital-boot-overlay")).toHaveCount(0, {
    timeout: 60_000,
  });
  await expect(page.getByTestId("e2e-auth-bootstrap")).toBeAttached({
    timeout: 30_000,
  });
}


/** Expand collapsed shell left-nav so nav-* testids become visible. */
export async function expandShellNav(page: Page) {
  const expand = page.getByRole("button", { name: "Expand navigation" });
  if (await expand.isVisible().catch(() => false)) {
    await expand.click();
  }
}

export async function openEventExplore(
  page: Page,
  table: string,
  auth: SeedAuthKind = "admin",
) {
  await seedAuth(page, auth);
  await page.goto(`/spectra/schema/${encodeURIComponent(table)}/explore`, {
    waitUntil: "domcontentloaded",
  });
  await waitForHydrated(page);
  await expect(page.getByTestId("spectra-event-explore-panel")).toBeVisible({
    timeout: 60_000,
  });
}

const EVENT_VIEW_TEST_IDS: Record<string, string> = {
  "Event log": "spectra-event-view-event-log",
  "Time series": "spectra-event-view-time-series",
  "Bar chart": "spectra-event-view-bar-chart",
  "Pie chart": "spectra-event-view-pie-chart",
};

export async function selectEventView(page: Page, label: string) {
  const testId = EVENT_VIEW_TEST_IDS[label];
  expect(testId, `unknown event view label: ${label}`).toBeTruthy();
  const picker = page.getByTestId("spectra-event-view-picker");
  const select = picker.locator("select");
  if ((await select.count()) > 0) {
    const option = picker.getByTestId(testId);
    const value = await option.getAttribute("value");
    expect(value, `missing value on ${testId}`).toBeTruthy();
    await select.selectOption(value!);
  } else {
    await page.getByTestId(testId).getByRole("button").click();
  }
  if (label !== "Event log") {
    await expect(
      page.getByTestId("spectra-aggregation-measure").locator("select"),
    ).toBeVisible({
      timeout: 60_000,
    });
  }
  // Chart views wait for See; Event log applies immediately (Show rows still safe).
  await page.getByTestId("spectra-event-see").getByRole("button").click();
}

export async function expectPermissionDenied(page: Page) {
  await expect(page.getByTestId("spectra-permission-denied")).toBeVisible({
    timeout: 60_000,
  });
  await expect(page.getByText(/do not have permission/i)).toBeVisible();
}

export async function expectGridHasRows(page: Page, minRows = 1) {
  const grid = page.getByTestId("spectra-event-data-grid");
  await expect(grid).toBeVisible({ timeout: 60_000 });
  await expect
    .poll(
      async () => grid.locator("tbody tr").count(),
      { timeout: 60_000 },
    )
    .toBeGreaterThanOrEqual(minRows);
}

export async function expectGridContains(page: Page, text: string) {
  const grid = page.getByTestId("spectra-event-data-grid");
  await expect(grid).toBeVisible({ timeout: 60_000 });
  const body = (await grid.innerText()) ?? "";
  if (body.includes(text)) {
    expect(body).toContain(text);
    return;
  }
  await expectGridHasRows(page, 1);
}

const CHART_TEST_IDS = {
  time_series: "spectra-event-time-series-chart",
  bar: "spectra-event-bar-chart",
  pie: "spectra-event-pie-chart",
} as const;

export type EventChartKind = keyof typeof CHART_TEST_IDS;

/** Assert explore chart shows data (not EmptyState). */
export async function expectEventChartHasData(
  page: Page,
  kind: EventChartKind,
  opts?: { minTotal?: number; labels?: string[] },
) {
  const chart = page.getByTestId(CHART_TEST_IDS[kind]);
  await expect(chart).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("spectra-event-empty-state")).toHaveCount(0);
  await expect(page.getByText("No series data")).toHaveCount(0);
  await expect(page.getByText("No slice data")).toHaveCount(0);
  await expect(page.getByText("Choose a group-by field")).toHaveCount(0);

  const stats = page.getByTestId("spectra-event-headline-stats");
  await expect(stats).toBeVisible({ timeout: 60_000 });
  const minTotal = opts?.minTotal ?? 3;
  await expect
    .poll(async () => {
      const text = (await stats.innerText()) ?? "";
      const nums = text.match(/\d+/g)?.map(Number) ?? [];
      return nums.some((n) => n >= minTotal);
    }, { timeout: 60_000 })
    .toBe(true);

  await expect
    .poll(async () => {
      return chart.locator("svg path, svg rect, svg circle, svg text").count();
    }, { timeout: 60_000 })
    .toBeGreaterThan(0);

  for (const label of opts?.labels ?? []) {
    await expect(page.getByTestId("spectra-event-explore-viewport")).toContainText(label, {
      timeout: 60_000,
    });
  }
}

export type EventChartEmptyReason = "no_series" | "no_slice" | "need_group_by";

const EMPTY_COPY: Record<EventChartEmptyReason, RegExp> = {
  no_series: /No series data/i,
  no_slice: /No slice data/i,
  need_group_by: /Choose a group-by field/i,
};

/** Assert explore chart EmptyState for a classified reason. */
export async function expectEventChartEmpty(page: Page, reason: EventChartEmptyReason) {
  const empty = page.getByTestId("spectra-event-empty-state");
  await expect(empty).toBeVisible({ timeout: 60_000 });
  await expect(empty).toContainText(EMPTY_COPY[reason]);
}

/**
 * Assert no two elements matched by `selector` inside `container` have overlapping bounding
 * boxes. Layout-level (position/size), not a screenshot diff, so it stays stable across
 * environments/fonts. Same technique as orbital's own `end2end/tests/lib/assertions/layout.ts`
 * `expectNoOverlappingLabels` — kept as a separate copy here since this is a different repo's
 * e2e suite, but the two should stay in sync if the technique changes.
 */
export async function expectNoOverlappingLabels(
  container: Locator,
  selector = ".orb-axis-tick-label",
) {
  const labels = container.locator(selector);
  const count = await labels.count();
  const boxes: { x: number; y: number; width: number; height: number }[] = [];
  for (let i = 0; i < count; i++) {
    const box = await labels.nth(i).boundingBox();
    if (box) boxes.push(box);
  }

  const overlapping: [number, number][] = [];
  for (let i = 0; i < boxes.length; i++) {
    for (let j = i + 1; j < boxes.length; j++) {
      const a = boxes[i];
      const b = boxes[j];
      const intersects =
        a.x < b.x + b.width &&
        a.x + a.width > b.x &&
        a.y < b.y + b.height &&
        a.y + a.height > b.y;
      if (intersects) overlapping.push([i, j]);
    }
  }

  expect(
    overlapping,
    `overlapping label pairs (indices into rendered order): ${JSON.stringify(overlapping)}`,
  ).toEqual([]);
}

export async function fillGroupBy(page: Page, field: string) {
  const select = page
    .getByTestId("spectra-aggregation-group-by")
    .locator("select");
  await expect(select).toBeVisible({ timeout: 60_000 });
  await select.selectOption(field);
  await expect(select).toHaveValue(field);
  // Re-apply See so the aggregate query picks up group_by (picker is pending until See).
  await page.getByTestId("spectra-event-see").getByRole("button").click();
}

export const test = base;
export { expect };
