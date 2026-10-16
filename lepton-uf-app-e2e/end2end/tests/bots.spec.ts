/**
 * Bots pages (`/user/bots`, `/user/bots/new`, `/user/bots/:id/token`).
 *
 * The lab host installs Gauge's bot gate, a "Spectra readers" group with no
 * owners, and an in-memory token store. `/api/test/bots` stands in for a super
 * user adding people to Bot creators or the Super User group.
 */
import { test, expect, waitForAppReady } from "./fixtures";
import { FreshCodes } from "./helpers/fresh_totp";
import {
  PASSWORD,
  answerStepUp,
  botLab,
  expectNoA11yViolations,
  fillProfile,
  openBots,
  seedBots,
  uniqueEmail,
  useDarkMode,
} from "./helpers/bots";

type Page = import("@playwright/test").Page;
type Request = import("@playwright/test").APIRequestContext;
type Auth = {
  signIn: (o: { email: string; password: string; referer?: string }) => Promise<void>;
  signInWithTotp: (o: {
    email: string;
    password: string;
    referer?: string;
  }) => Promise<{ totp_secret: string }>;
};

/** Signed-in Bot creator with no bots. */
async function asCreator(page: Page, request: Request, auth: Auth, label: string) {
  const email = uniqueEmail(label);
  await auth.signIn({ email, password: PASSWORD, referer: "/user/account-settings" });
  await botLab(request, { action: "join_creators", email });
  return email;
}

/** Signed-in super user with an authenticator, who may add bots to any group. */
async function asSuperUser(page: Page, request: Request, auth: Auth, label: string) {
  const email = uniqueEmail(label);
  const { totp_secret } = await auth.signInWithTotp({
    email,
    password: PASSWORD,
    referer: "/user/account-settings",
  });
  await botLab(request, { action: "super_user", email });
  return { email, codes: new FreshCodes(totp_secret) };
}

/** Steps 2 and 3 with a token-only choice; returns the revealed secret. */
async function tokenOnlyToReveal(page: Page): Promise<string> {
  await page.getByTestId("bot-access-magnon-read").getByRole("checkbox").check();
  await page.getByTestId("bot-access-continue").getByRole("button").click();
  await expect(page.getByTestId("bot-token-label")).toBeVisible({ timeout: 30_000 });
  await page.getByTestId("bot-token-label").locator("input").fill("Claude Code on laptop");
  await page.getByTestId("bot-token-submit").getByRole("button").click();
  const secret = page.getByTestId("bot-token-secret");
  await expect(secret).toContainText("labtk_", { timeout: 30_000 });
  return (await secret.innerText()).trim();
}

test.describe("pw-lepton-bots", () => {
  test("pw-lepton-bots-list-empty-happy", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "empty");
    await openBots(page);
    const empty = page.getByTestId("bots-empty");
    await expect(empty).toContainText("No bots yet");
    await expect(empty).toContainText("tools like Claude Code");
    await expect(empty.getByRole("button", { name: "Create bot" })).toBeEnabled();
    await expectNoA11yViolations(page, "bots empty state");
  });

  test("pw-lepton-bots-no-permission-sad", async ({ page, auth }) => {
    const email = uniqueEmail("noperm");
    await auth.signIn({ email, password: PASSWORD, referer: "/user/account-settings" });
    await openBots(page);
    const gate = page.getByTestId("bots-no-permission");
    await expect(gate).toContainText("You can't create bots yet");
    await expect(gate).toContainText("Bot creators");
    await expect(page.getByRole("button", { name: "Create bot" })).toHaveCount(0);
    // Going straight to the create page shows the same explanation, not a form.
    await openBots(page, "/user/bots/new");
    await expect(page.getByTestId("bots-no-permission")).toBeVisible();
    await expect(page.getByTestId("bot-display-name")).toHaveCount(0);
  });

  test("pw-lepton-bots-anonymous-sad", async ({ page }) => {
    await page.goto("/");
    await waitForAppReady(page);
    await page.goto("/user/bots");
    await waitForAppReady(page);
    await expect(page.getByTestId("bots-list")).toHaveCount(0);
    await expect(page.getByTestId("bots-empty")).toHaveCount(0);
  });

  test("pw-lepton-bots-create-full-happy", async ({ page, request, auth }) => {
    const { codes } = await asSuperUser(page, request, auth, "full");
    await openBots(page, "/user/bots/new");
    await expect(page.getByTestId("bot-create-progress")).toContainText("Step 1 of 3");
    await expect(page.getByTestId("bot-owner")).toContainText("You (");
    const handle = `full-${Date.now()}-bot`;
    await fillProfile(page, "Spectra agent", handle, "Reads metrics for Claude Code");
    await page.getByTestId("bot-create-submit").getByRole("button").click();

    // Step 2: the super user may add the bot to Spectra readers.
    await expect(page.getByTestId("bot-create-progress")).toContainText("Step 2 of 3", {
      timeout: 30_000,
    });
    await page.getByTestId("bot-access-spectra-read").getByRole("checkbox").check();
    await page.getByTestId("bot-access-continue").getByRole("button").click();
    await answerStepUp(page, await codes.next());

    // Step 3: Spectra is preselected on the token.
    await expect(page.getByTestId("bot-create-progress")).toContainText("Step 3 of 3", {
      timeout: 30_000,
    });
    await expect(
      page.getByTestId("bot-token-scope-spectra-read").getByRole("checkbox"),
    ).toBeChecked();
    await page.getByTestId("bot-token-label").locator("input").fill("Claude Code on laptop");
    await page.getByTestId("bot-token-ttl").getByRole("radio", { name: "90 days" }).check();
    await page.getByTestId("bot-token-submit").getByRole("button").click();

    const reveal = page.getByTestId("bot-token-reveal");
    await expect(reveal).toBeVisible({ timeout: 30_000 });
    await expect(reveal).toContainText("You won't see it again");
    const secret = (await page.getByTestId("bot-token-secret").innerText()).trim();
    expect(secret).toMatch(/^labtk_[0-9a-f]{32}$/);
    const setup = page.getByTestId("bot-token-setup-code").first();
    await expect(setup).toContainText("claude mcp add-json");
    await expect(setup).not.toContainText(secret);
    await expect(page.getByTestId("bot-token-summary")).toContainText(
      "can read Spectra metrics and events",
    );
    // Keyboard users land on the copy button.
    await expect(page.getByTestId("bot-token-copy").getByRole("button")).toBeFocused();
    await expectNoA11yViolations(page, "token reveal");

    // The bot really joined the group.
    const botId = await botIdFromList(page, handle);
    const check = await botLab(request, { action: "in_readers", bot_id: botId });
    expect(check.member).toBe(true);
  });

  test("pw-lepton-bots-reveal-once-happy", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "once");
    await openBots(page, "/user/bots/new");
    const handle = `once-${Date.now()}-bot`;
    await fillProfile(page, "Once", handle);
    await page.getByTestId("bot-create-submit").getByRole("button").click();
    const first = await tokenOnlyToReveal(page);

    await page.getByTestId("bot-token-done").getByRole("button").click();
    await expect(page.getByTestId("bots-page")).toBeVisible({ timeout: 30_000 });
    await expect(page.getByTestId(`bot-card-${handle}`)).toContainText("Active");
    await page.reload();
    await waitForAppReady(page);
    expect(await page.content()).not.toContain(first);

    // "New token" mints a different secret.
    await page.getByTestId(`bot-card-${handle}`).getByRole("button", { name: "New token" }).click();
    await expect(page.getByTestId("bot-token-page")).toBeVisible({ timeout: 60_000 });
    await page.getByTestId("bot-token-scope-magnon-read").getByRole("checkbox").check();
    await page.getByTestId("bot-token-label").locator("input").fill("Second");
    await page.getByTestId("bot-token-submit").getByRole("button").click();
    const second = (await page.getByTestId("bot-token-secret").innerText()).trim();
    expect(second).toMatch(/^labtk_/);
    expect(second).not.toBe(first);
  });

  test("pw-lepton-bots-handle-clash-sad", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "clash");
    const prefix = `clash${Date.now()}`;
    await seedBots(request, email, prefix, 1);
    await openBots(page, "/user/bots/new");
    await page.getByTestId("bot-display-name").locator("input").fill("Clash");
    await page.getByTestId("bot-handle").locator("input").fill(`${prefix}-0-bot`);
    const status = page.getByTestId("bot-handle-status");
    await expect(status).toContainText("is taken", { timeout: 30_000 });
    await expect(page.getByTestId("bot-create-submit").getByRole("button")).toBeDisabled();
    await page.getByTestId("bot-handle-use-suggestion").getByRole("button").click();
    await expect(status).toContainText("is free", { timeout: 30_000 });
    await page.getByTestId("bot-create-submit").getByRole("button").click();
    await expect(page.getByTestId("bot-create-progress")).toContainText("Step 2 of 3", {
      timeout: 30_000,
    });
  });

  test("pw-lepton-bots-handle-invalid-sad", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "invalid");
    await openBots(page, "/user/bots/new");
    await page.getByTestId("bot-display-name").locator("input").fill("Bad handle");
    const status = page.getByTestId("bot-handle-status");
    await page.getByTestId("bot-handle").locator("input").fill("spectra");
    await expect(status).toContainText("Handles end in bot", { timeout: 30_000 });
    await page.getByTestId("bot-handle").locator("input").fill("support-bot");
    await expect(status).toContainText("kept for the platform", { timeout: 30_000 });
    await expect(page.getByTestId("bot-create-submit").getByRole("button")).toBeDisabled();
  });

  test("pw-lepton-bots-profile-empty-name-sad", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "noname");
    await openBots(page, "/user/bots/new");
    const handle = `noname-${Date.now()}-bot`;
    await page.getByTestId("bot-handle").locator("input").fill(handle);
    await expect(page.getByTestId("bot-handle-status")).toContainText("is free", {
      timeout: 30_000,
    });
    await expect(page.getByTestId("bot-create-submit").getByRole("button")).toBeDisabled();
    await openBots(page);
    await expect(page.getByTestId("bots-empty")).toBeVisible();
  });

  test("pw-lepton-bots-access-not-grantable-sad", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "blocked");
    await openBots(page, "/user/bots/new");
    await fillProfile(page, "Blocked", `blocked-${Date.now()}-bot`);
    await page.getByTestId("bot-create-submit").getByRole("button").click();
    const blocked = page.getByTestId("bot-access-blocked-spectra-read");
    await expect(blocked).toBeVisible({ timeout: 30_000 });
    await expect(blocked.getByRole("checkbox")).toBeDisabled();
    await expect(blocked).toContainText("Only an owner of Spectra readers or a super user");
    await expect(page.getByTestId("bot-access-spectra-read")).toHaveCount(0);
    // Token-only access still works for this person.
    await expect(
      page.getByTestId("bot-access-magnon-read").getByRole("checkbox"),
    ).toBeEnabled();
    await expectNoA11yViolations(page, "access step");
  });

  test("pw-lepton-bots-quota-sad", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "quota");
    await seedBots(request, email, `quota${Date.now()}`, 10);
    await openBots(page);
    await expect(page.getByTestId("bots-count")).toContainText("10 of 10 bots");
    await expect(page.getByTestId("bots-create").getByRole("button")).toBeDisabled();
    await expect(page.getByTestId("bots-at-limit")).toContainText("most bots allowed");
    await openBots(page, "/user/bots/new");
    await expect(page.getByTestId("bots-at-limit")).toContainText("Delete one to make room");
    await expect(page.getByTestId("bot-display-name")).toHaveCount(0);
  });

  test("pw-lepton-bots-token-limit-sad", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "tokcap");
    const [bot] = await seedBots(request, email, `tokcap${Date.now()}`, 1);
    await botLab(request, { action: "seed_tokens", bot_id: bot.id, count: 5 });
    await openBots(page, `/user/bots/${bot.id}/token`);
    await page.getByTestId("bot-token-scope-magnon-read").getByRole("checkbox").check();
    await page.getByTestId("bot-token-label").locator("input").fill("Sixth");
    await page.getByTestId("bot-token-submit").getByRole("button").click();
    await expect(page.getByTestId("bot-token-error")).toContainText("5 active tokens", {
      timeout: 30_000,
    });
    await expect(page.getByTestId("bot-token-reveal")).toHaveCount(0);
  });

  test("pw-lepton-bots-foreign-bot-sad", async ({ page, request, auth }) => {
    const other = uniqueEmail("owner");
    // Seed the other owner without signing in as them.
    await request.post("/api/test/seed-data", {
      data: { scenario: "auth_basic_user", email: other, password: PASSWORD },
    });
    await botLab(request, { action: "join_creators", email: other });
    const [theirs] = await seedBots(request, other, `theirs${Date.now()}`, 1);

    await asCreator(page, request, auth, "intruder");
    await openBots(page, `/user/bots/${theirs.id}/token`);
    const nf = page.getByTestId("bot-not-found");
    await expect(nf).toContainText("Bot not found");
    await expect(nf).toContainText("isn't yours to see");
    await expect(page.getByTestId("bot-token-label")).toHaveCount(0);
  });

  test("pw-lepton-bots-suspended-bot-sad", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "paused");
    const [bot] = await seedBots(request, email, `paused${Date.now()}`, 1, true);
    await openBots(page);
    const card = page.getByTestId(`bot-card-${bot.handle}`);
    await expect(card).toContainText("Paused");
    await expect(card.getByRole("button", { name: "New token" })).toBeDisabled();
    await expect(card).toContainText("Paused bots can't get tokens");
    await openBots(page, `/user/bots/${bot.id}/token`);
    await expect(page.getByTestId("bot-token-paused")).toContainText("is paused");
  });

  test("pw-lepton-bots-dark-mode", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "dark");
    await seedBots(request, email, `dark${Date.now()}`, 2, false);
    await useDarkMode(page);
    await openBots(page);
    const scheme = await page.evaluate(() => {
      const el = document.querySelector(".orbital-theme-provider");
      return el ? getComputedStyle(el).getPropertyValue("--orb-color-scheme").trim() : "";
    });
    expect(scheme).toBe("dark");
    await expectNoA11yViolations(page, "bots list (dark)");
    await openBots(page, "/user/bots/new");
    await expectNoA11yViolations(page, "create step 1 (dark)");
  });

  test("pw-lepton-bots-mobile-390", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "mobile");
    await seedBots(request, email, `mobile${Date.now()}`, 2);
    await page.setViewportSize({ width: 390, height: 844 });
    for (const path of ["/user/bots", "/user/bots/new"]) {
      await openBots(page, path);
      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
      );
      expect(overflow, `${path} scrolls sideways`).toBeLessThanOrEqual(0);
    }
    await expectNoA11yViolations(page, "create step 1 (390px)");
  });

  test("pw-lepton-bots-keyboard-create", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "keys");
    await openBots(page, "/user/bots/new");
    const name = page.getByTestId("bot-display-name").locator("input");
    await name.focus();
    await page.keyboard.type("Keyboard bot");
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("bot-handle").locator("input")).toBeFocused();
    const handle = `keys-${Date.now()}-bot`;
    await page.keyboard.type(handle);
    await expect(page.getByTestId("bot-handle-status")).toContainText("is free", {
      timeout: 30_000,
    });
    // Tab forward to the Create button and press Enter.
    const submit = page.getByTestId("bot-create-submit").getByRole("button");
    for (let i = 0; i < 10 && !(await submit.evaluate((el) => el === document.activeElement)); i++) {
      await page.keyboard.press("Tab");
    }
    await expect(submit).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("bot-create-progress")).toContainText("Step 2 of 3", {
      timeout: 30_000,
    });
    await tokenOnlyToReveal(page);
    await expect(page.getByTestId("bot-token-copy").getByRole("button")).toBeFocused();
  });
});

/** Bot id for `handle`, read from the "New token" link on the Bots page. */
async function botIdFromList(page: Page, handle: string): Promise<string> {
  // The list page links "New token" to /user/bots/{id}/token.
  await page.goto("/user/bots");
  await waitForAppReady(page);
  const card = page.getByTestId(`bot-card-${handle}`);
  await expect(card).toBeVisible({ timeout: 60_000 });
  await card.getByRole("button", { name: "New token" }).click();
  await expect(page).toHaveURL(/\/user\/bots\/[0-9a-f-]+\/token/, { timeout: 30_000 });
  const match = /\/user\/bots\/([0-9a-f-]+)\/token/.exec(page.url());
  return match ? match[1] : "";
}
