/**
 * Service users pages (`/user/service-users`, `/user/service-users/new`, `/user/service-users/:id/token`).
 *
 * The lab host installs Gauge's service user gate, a "Spectra readers" group with no
 * owners, and an in-memory token store. `/api/test/service-users` stands in for a super
 * user adding people to Service user creators or the Super User group.
 */
import { test, expect, waitForAppReady } from "./fixtures";
import { FreshCodes } from "./helpers/fresh_totp";
import {
  PASSWORD,
  answerStepUp,
  serviceUserLab,
  expectNoA11yViolations,
  fillProfile,
  openServiceUsers,
  seedServiceUsers,
  uniqueEmail,
  useDarkMode,
} from "./helpers/service-users";

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

/** Signed-in service user creator with no service users. */
async function asCreator(page: Page, request: Request, auth: Auth, label: string) {
  const email = uniqueEmail(label);
  await auth.signIn({ email, password: PASSWORD, referer: "/user/account-settings" });
  await serviceUserLab(request, { action: "join_creators", email });
  return email;
}

/** Signed-in super user with an authenticator, who may add service users to any group. */
async function asSuperUser(page: Page, request: Request, auth: Auth, label: string) {
  const email = uniqueEmail(label);
  const { totp_secret } = await auth.signInWithTotp({
    email,
    password: PASSWORD,
    referer: "/user/account-settings",
  });
  await serviceUserLab(request, { action: "super_user", email });
  return { email, codes: new FreshCodes(totp_secret) };
}

/** Steps 2 and 3 with a token-only choice; returns the revealed secret. */
async function tokenOnlyToReveal(page: Page): Promise<string> {
  await page.getByTestId("service-user-access-magnon-read").getByRole("checkbox").check();
  await page.getByTestId("service-user-access-continue").getByRole("button").click();
  await expect(page.getByTestId("service-user-token-label")).toBeVisible({ timeout: 30_000 });
  await page.getByTestId("service-user-token-label").locator("input").fill("Claude Code on laptop");
  await page.getByTestId("service-user-token-submit").getByRole("button").click();
  const secret = page.getByTestId("service-user-token-secret");
  await expect(secret).toContainText("labtk_", { timeout: 30_000 });
  return (await secret.innerText()).trim();
}

test.describe("pw-lepton-service-users", () => {
  test("pw-lepton-service-users-list-empty-happy", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "empty");
    await openServiceUsers(page);
    const empty = page.getByTestId("service-users-empty");
    await expect(empty).toContainText("No service users yet");
    await expect(empty).toContainText("an MCP client like Claude Code");
    await expect(empty.getByRole("button", { name: "Create service user" })).toBeEnabled();
    await expectNoA11yViolations(page, "service users empty state");
  });

  test("pw-lepton-service-users-no-permission-sad", async ({ page, auth }) => {
    const email = uniqueEmail("noperm");
    await auth.signIn({ email, password: PASSWORD, referer: "/user/account-settings" });
    await openServiceUsers(page);
    const gate = page.getByTestId("service-users-no-permission");
    await expect(gate).toContainText("You can't create service users yet");
    await expect(gate).toContainText("Service user creators");
    await expect(page.getByRole("button", { name: "Create service user" })).toHaveCount(0);
    // Going straight to the create page shows the same explanation, not a form.
    await openServiceUsers(page, "/user/service-users/new");
    await expect(page.getByTestId("service-users-no-permission")).toBeVisible();
    await expect(page.getByTestId("service-user-display-name")).toHaveCount(0);
  });

  test("pw-lepton-service-users-anonymous-sad", async ({ page }) => {
    await page.goto("/");
    await waitForAppReady(page);
    await page.goto("/user/service-users");
    await waitForAppReady(page);
    await expect(page.getByTestId("service-users-list")).toHaveCount(0);
    await expect(page.getByTestId("service-users-empty")).toHaveCount(0);
  });

  test("pw-lepton-service-users-create-full-happy", async ({ page, request, auth }) => {
    const { codes } = await asSuperUser(page, request, auth, "full");
    await openServiceUsers(page, "/user/service-users/new");
    await expect(page.getByTestId("service-user-create-progress")).toContainText("Step 1 of 3");
    await expect(page.getByTestId("service-user-owner")).toContainText("You (");
    const handle = `full-${Date.now()}`;
    await fillProfile(page, "Spectra agent", handle, "Reads metrics for Claude Code");
    await page.getByTestId("service-user-create-submit").getByRole("button").click();

    // Step 2: the super user may add the service user to Spectra readers.
    await expect(page.getByTestId("service-user-create-progress")).toContainText("Step 2 of 3", {
      timeout: 30_000,
    });
    await page.getByTestId("service-user-access-spectra-read").getByRole("checkbox").check();
    await page.getByTestId("service-user-access-continue").getByRole("button").click();
    await answerStepUp(page, await codes.next());

    // Step 3: Spectra is preselected on the token.
    await expect(page.getByTestId("service-user-create-progress")).toContainText("Step 3 of 3", {
      timeout: 30_000,
    });
    await expect(
      page.getByTestId("service-user-token-scope-spectra-read").getByRole("checkbox"),
    ).toBeChecked();
    await page.getByTestId("service-user-token-label").locator("input").fill("Claude Code on laptop");
    await page.getByTestId("service-user-token-ttl").getByRole("radio", { name: "90 days" }).check();
    await page.getByTestId("service-user-token-submit").getByRole("button").click();

    const reveal = page.getByTestId("service-user-token-reveal");
    await expect(reveal).toBeVisible({ timeout: 30_000 });
    await expect(reveal).toContainText("You won't see it again");
    const secret = (await page.getByTestId("service-user-token-secret").innerText()).trim();
    expect(secret).toMatch(/^labtk_[0-9a-f]{32}$/);
    const setup = page.getByTestId("service-user-token-setup-code").first();
    await expect(setup).toContainText("claude mcp add-json");
    await expect(setup).not.toContainText(secret);
    await expect(page.getByTestId("service-user-token-summary")).toContainText(
      "can read Spectra metrics and events",
    );
    // Keyboard users land on the copy button.
    await expect(page.getByTestId("service-user-token-copy").getByRole("button")).toBeFocused();
    await expectNoA11yViolations(page, "token reveal");

    // The service user really joined the group.
    const serviceUserId = await serviceUserIdFromList(page, handle);
    const check = await serviceUserLab(request, { action: "in_readers", service_user_id: serviceUserId });
    expect(check.member).toBe(true);
    // The list marks the handle as a service user, not a person.
    await expect(
      page.getByTestId(`service-user-card-${handle}`).getByTestId("service-user-badge"),
    ).toHaveText("Service user");
  });

  test("pw-lepton-service-users-reveal-once-happy", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "once");
    await openServiceUsers(page, "/user/service-users/new");
    const handle = `once-${Date.now()}`;
    await fillProfile(page, "Once", handle);
    await page.getByTestId("service-user-create-submit").getByRole("button").click();
    const first = await tokenOnlyToReveal(page);

    await page.getByTestId("service-user-token-done").getByRole("button").click();
    await expect(page.getByTestId("service-users-page")).toBeVisible({ timeout: 30_000 });
    await expect(page.getByTestId(`service-user-card-${handle}`)).toContainText("Active");
    await page.reload();
    await waitForAppReady(page);
    expect(await page.content()).not.toContain(first);

    // "New token" mints a different secret.
    await page.getByTestId(`service-user-card-${handle}`).getByRole("button", { name: "New token" }).click();
    await expect(page.getByTestId("service-user-token-page")).toBeVisible({ timeout: 60_000 });
    await page.getByTestId("service-user-token-scope-magnon-read").getByRole("checkbox").check();
    await page.getByTestId("service-user-token-label").locator("input").fill("Second");
    await page.getByTestId("service-user-token-submit").getByRole("button").click();
    const second = (await page.getByTestId("service-user-token-secret").innerText()).trim();
    expect(second).toMatch(/^labtk_/);
    expect(second).not.toBe(first);
  });

  test("pw-lepton-service-users-handle-clash-sad", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "clash");
    const prefix = `clash${Date.now()}`;
    await seedServiceUsers(request, email, prefix, 1);
    await openServiceUsers(page, "/user/service-users/new");
    await page.getByTestId("service-user-display-name").locator("input").fill("Clash");
    await page.getByTestId("service-user-handle").locator("input").fill(`${prefix}-0`);
    const status = page.getByTestId("service-user-handle-status");
    await expect(status).toContainText("is taken", { timeout: 30_000 });
    await expect(page.getByTestId("service-user-create-submit").getByRole("button")).toBeDisabled();
    await page.getByTestId("service-user-handle-use-suggestion").getByRole("button").click();
    await expect(status).toContainText("is free", { timeout: 30_000 });
    await page.getByTestId("service-user-create-submit").getByRole("button").click();
    await expect(page.getByTestId("service-user-create-progress")).toContainText("Step 2 of 3", {
      timeout: 30_000,
    });
  });

  test("pw-lepton-service-users-handle-invalid-sad", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "invalid");
    await openServiceUsers(page, "/user/service-users/new");
    await page.getByTestId("service-user-display-name").locator("input").fill("Bad handle");
    const status = page.getByTestId("service-user-handle-status");
    await page.getByTestId("service-user-handle").locator("input").fill("se@n");
    await expect(status).toContainText("lowercase letters, numbers or single dashes", { timeout: 30_000 });
    await page.getByTestId("service-user-handle").locator("input").fill("support");
    await expect(status).toContainText("kept for the platform", { timeout: 30_000 });
    await expect(page.getByTestId("service-user-create-submit").getByRole("button")).toBeDisabled();
  });

  test("pw-lepton-service-users-profile-empty-name-sad", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "noname");
    await openServiceUsers(page, "/user/service-users/new");
    const handle = `noname-${Date.now()}`;
    await page.getByTestId("service-user-handle").locator("input").fill(handle);
    await expect(page.getByTestId("service-user-handle-status")).toContainText("is free", {
      timeout: 30_000,
    });
    await expect(page.getByTestId("service-user-create-submit").getByRole("button")).toBeDisabled();
    await openServiceUsers(page);
    await expect(page.getByTestId("service-users-empty")).toBeVisible();
  });

  test("pw-lepton-service-users-access-not-grantable-sad", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "blocked");
    await openServiceUsers(page, "/user/service-users/new");
    await fillProfile(page, "Blocked", `blocked-${Date.now()}`);
    await page.getByTestId("service-user-create-submit").getByRole("button").click();
    const blocked = page.getByTestId("service-user-access-blocked-spectra-read");
    await expect(blocked).toBeVisible({ timeout: 30_000 });
    await expect(blocked.getByRole("checkbox")).toBeDisabled();
    await expect(blocked).toContainText("Only an owner of Spectra readers or a super user");
    await expect(page.getByTestId("service-user-access-spectra-read")).toHaveCount(0);
    // Token-only access still works for this person.
    await expect(
      page.getByTestId("service-user-access-magnon-read").getByRole("checkbox"),
    ).toBeEnabled();
    await expectNoA11yViolations(page, "access step");
  });

  test("pw-lepton-service-users-quota-sad", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "quota");
    await seedServiceUsers(request, email, `quota${Date.now()}`, 10);
    await openServiceUsers(page);
    await expect(page.getByTestId("service-users-count")).toContainText("10 of 10 service users");
    await expect(page.getByTestId("service-users-create").getByRole("button")).toBeDisabled();
    await expect(page.getByTestId("service-users-at-limit")).toContainText("most service users allowed");
    await openServiceUsers(page, "/user/service-users/new");
    await expect(page.getByTestId("service-users-at-limit")).toContainText("Delete one to make room");
    await expect(page.getByTestId("service-user-display-name")).toHaveCount(0);
  });

  test("pw-lepton-service-users-token-limit-sad", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "tokcap");
    const [serviceUser] = await seedServiceUsers(request, email, `tokcap${Date.now()}`, 1);
    await serviceUserLab(request, { action: "seed_tokens", service_user_id: serviceUser.id, count: 5 });
    await openServiceUsers(page, `/user/service-users/${serviceUser.id}/token`);
    await page.getByTestId("service-user-token-scope-magnon-read").getByRole("checkbox").check();
    await page.getByTestId("service-user-token-label").locator("input").fill("Sixth");
    await page.getByTestId("service-user-token-submit").getByRole("button").click();
    await expect(page.getByTestId("service-user-token-error")).toContainText("5 active tokens", {
      timeout: 30_000,
    });
    await expect(page.getByTestId("service-user-token-reveal")).toHaveCount(0);
  });

  test("pw-lepton-service-users-foreign-service-user-sad", async ({ page, request, auth }) => {
    const other = uniqueEmail("owner");
    // Seed the other owner without signing in as them.
    await request.post("/api/test/seed-data", {
      data: { scenario: "auth_basic_user", email: other, password: PASSWORD },
    });
    await serviceUserLab(request, { action: "join_creators", email: other });
    const [theirs] = await seedServiceUsers(request, other, `theirs${Date.now()}`, 1);

    await asCreator(page, request, auth, "intruder");
    await openServiceUsers(page, `/user/service-users/${theirs.id}/token`);
    const nf = page.getByTestId("service-user-not-found");
    await expect(nf).toContainText("Service user not found");
    await expect(nf).toContainText("isn't yours to see");
    await expect(page.getByTestId("service-user-token-label")).toHaveCount(0);
  });

  test("pw-lepton-service-users-suspended-service-user-sad", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "paused");
    const [serviceUser] = await seedServiceUsers(request, email, `paused${Date.now()}`, 1, true);
    await openServiceUsers(page);
    const card = page.getByTestId(`service-user-card-${serviceUser.handle}`);
    await expect(card).toContainText("Paused");
    await expect(card.getByRole("button", { name: "New token" })).toBeDisabled();
    await expect(card).toContainText("Paused service users can't get tokens");
    await openServiceUsers(page, `/user/service-users/${serviceUser.id}/token`);
    await expect(page.getByTestId("service-user-token-paused")).toContainText("is paused");
  });

  test("pw-lepton-service-users-dark-mode", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "dark");
    await seedServiceUsers(request, email, `dark${Date.now()}`, 2, false);
    await useDarkMode(page);
    await openServiceUsers(page);
    const scheme = await page.evaluate(() => {
      const el = document.querySelector(".orbital-theme-provider");
      return el ? getComputedStyle(el).getPropertyValue("--orb-color-scheme").trim() : "";
    });
    expect(scheme).toBe("dark");
    await expectNoA11yViolations(page, "service users list (dark)");
    await openServiceUsers(page, "/user/service-users/new");
    await expectNoA11yViolations(page, "create step 1 (dark)");
  });

  test("pw-lepton-service-users-mobile-390", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "mobile");
    await seedServiceUsers(request, email, `mobile${Date.now()}`, 2);
    await page.setViewportSize({ width: 390, height: 844 });
    for (const path of ["/user/service-users", "/user/service-users/new"]) {
      await openServiceUsers(page, path);
      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
      );
      expect(overflow, `${path} scrolls sideways`).toBeLessThanOrEqual(0);
    }
    await expectNoA11yViolations(page, "create step 1 (390px)");
  });

  test("pw-lepton-service-users-keyboard-create", async ({ page, request, auth }) => {
    await asCreator(page, request, auth, "keys");
    await openServiceUsers(page, "/user/service-users/new");
    const name = page.getByTestId("service-user-display-name").locator("input");
    await name.focus();
    await page.keyboard.type("Keyboard service user");
    await page.keyboard.press("Tab");
    await expect(page.getByTestId("service-user-handle").locator("input")).toBeFocused();
    const handle = `keys-${Date.now()}`;
    await page.keyboard.type(handle);
    await expect(page.getByTestId("service-user-handle-status")).toContainText("is free", {
      timeout: 30_000,
    });
    // Tab forward to the Create button and press Enter.
    const submit = page.getByTestId("service-user-create-submit").getByRole("button");
    for (let i = 0; i < 10 && !(await submit.evaluate((el) => el === document.activeElement)); i++) {
      await page.keyboard.press("Tab");
    }
    await expect(submit).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("service-user-create-progress")).toContainText("Step 2 of 3", {
      timeout: 30_000,
    });
    await tokenOnlyToReveal(page);
    await expect(page.getByTestId("service-user-token-copy").getByRole("button")).toBeFocused();
  });
});

/** Service user id for `handle`, read from the "New token" link on the Service users page. */
async function serviceUserIdFromList(page: Page, handle: string): Promise<string> {
  // The list page links "New token" to /user/service-users/{id}/token.
  await page.goto("/user/service-users");
  await waitForAppReady(page);
  const card = page.getByTestId(`service-user-card-${handle}`);
  await expect(card).toBeVisible({ timeout: 60_000 });
  await card.getByRole("button", { name: "New token" }).click();
  await expect(page).toHaveURL(/\/user\/service-users\/[0-9a-f-]+\/token/, { timeout: 30_000 });
  const match = /\/user\/service-users\/([0-9a-f-]+)\/token/.exec(page.url());
  return match ? match[1] : "";
}
