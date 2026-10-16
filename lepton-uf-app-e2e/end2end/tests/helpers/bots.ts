import AxeBuilder from "@axe-core/playwright";
import { expect } from "@playwright/test";
import { waitForAppReady } from "../fixtures";

type Page = import("@playwright/test").Page;
type Request = import("@playwright/test").APIRequestContext;

export const PASSWORD = "CorrectHorseBattery1!";

/** Unique email per test so seeded state never collides across runs. */
export function uniqueEmail(label: string): string {
  return `bots-${label}-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`;
}

/** Call the lab-only `/api/test/bots` seed route. */
export async function botLab(
  request: Request,
  body: Record<string, unknown>,
): Promise<Record<string, unknown>> {
  const res = await request.post("/api/test/bots", { data: body });
  expect(res.ok(), await res.text()).toBeTruthy();
  return (await res.json()) as Record<string, unknown>;
}

/** Seed `count` bots for `email`; returns their ids and handles. */
export async function seedBots(
  request: Request,
  email: string,
  prefix: string,
  count: number,
  suspend = false,
): Promise<{ id: string; handle: string }[]> {
  const out = await botLab(request, {
    action: "seed_bots",
    email,
    prefix,
    count,
    suspend,
  });
  return out.bots as { id: string; handle: string }[];
}

/** Go to a bots page and wait until it has rendered. */
export async function openBots(page: Page, path = "/user/bots"): Promise<void> {
  await page.goto(path);
  await waitForAppReady(page);
  const pageId = path === "/user/bots" ? "bots-page" : path.endsWith("/new") ? "bot-create-page" : "bot-token-page";
  await expect(page.getByTestId(pageId)).toBeVisible({ timeout: 60_000 });
}

/** Fill step 1 and wait for the handle check to say the handle is free. */
export async function fillProfile(
  page: Page,
  name: string,
  handle: string,
  description = "",
): Promise<void> {
  await page.getByTestId("bot-display-name").locator("input").fill(name);
  await page.getByTestId("bot-handle").locator("input").fill(handle);
  if (description) {
    await page.getByTestId("bot-description").locator("textarea").fill(description);
  }
  await expect(page.getByTestId("bot-handle-status")).toContainText(
    `@${handle} is free`,
    { timeout: 30_000 },
  );
}

/** Answer the step-up dialog with an authenticator code. */
export async function answerStepUp(page: Page, code: string): Promise<void> {
  const dialog = page.getByTestId("step-up-dialog");
  await expect(dialog).toBeVisible({ timeout: 30_000 });
  await dialog.getByTestId("step-up-totp").locator("input").fill(code);
  await dialog.getByTestId("step-up-submit").getByRole("button").click();
}

/**
 * Orbital and shell nodes with known axe findings, outside these pages' own
 * markup. Each one is tracked in BACKLOG.md; remove it here when fixed.
 */
const KNOWN_UPSTREAM_EXCLUDES = [
  // App-bar search combobox: aria-label on a span (orbital_combobox_icon_aria_01).
  ".orbital-combobox__expand-icon",
  // Stepper draws the active step as an unlabelled spinner (orbital_stepper_a11y_01).
  '[data-testid="bot-create-stepper"] .orbital-spinner',
];

/** True for white text on Orbital's default brand primary button (3.55:1). */
function isKnownPrimaryContrast(id: string, html: string): boolean {
  // orbital_primary_button_contrast_01
  return id === "color-contrast" && html.includes("orbital-button--primary");
}

/** Run axe (WCAG 2 A/AA) on the page and fail on any violation. */
export async function expectNoA11yViolations(page: Page, label: string): Promise<void> {
  let builder = new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa"]);
  for (const selector of KNOWN_UPSTREAM_EXCLUDES) {
    builder = builder.exclude(selector);
  }
  const results = await builder.analyze();
  const summary = results.violations.flatMap((v) =>
    v.nodes
      .filter((n) => !isKnownPrimaryContrast(v.id, n.html))
      .map((n) => `${v.id} (${v.impact}): ${n.target.join(" ")}`),
  );
  expect(summary, `axe violations on ${label}`).toEqual([]);
}

/** Switch the signed-in user to dark mode through the Appearance page. */
export async function useDarkMode(page: Page): Promise<void> {
  // A cold goto to /user/appearance can render before the session is known
  // and show sign-in; navigate from a loaded settings page instead.
  await openBots(page);
  const expand = page.getByRole("button", { name: "Expand navigation" });
  if (await expand.isVisible().catch(() => false)) {
    await expand.click();
  }
  await page.getByTestId("nav-lepton-appearance").click();
  await expect(page).toHaveURL(/\/user\/appearance/, { timeout: 60_000 });
  await expect(page.getByTestId("appearance-page")).toBeVisible({ timeout: 60_000 });
  await page.getByRole("radio", { name: "Dark" }).check();
  await page.getByTestId("appearance-save").getByRole("button").click();
  await expect(page.getByText(/Appearance preferences saved/i)).toBeVisible({
    timeout: 30_000,
  });
}
