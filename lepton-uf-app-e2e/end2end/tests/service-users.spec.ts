/**
 * Service users pages (`/user/service-users`, `/user/service-users/new`, `/user/service-users/:id`,
 * `/user/service-users/:id/token`).
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

/**
 * Signed-in service user creator with an authenticator but no group-granting powers.
 * Minting a token needs a fresh code, so every minting test uses one of these.
 */
async function asCreatorWithTotp(page: Page, request: Request, auth: Auth, label: string) {
  const email = uniqueEmail(label);
  const { totp_secret } = await auth.signInWithTotp({
    email,
    password: PASSWORD,
    referer: "/user/account-settings",
  });
  await serviceUserLab(request, { action: "join_creators", email });
  return { email, codes: new FreshCodes(totp_secret) };
}

/** Steps 2 and 3 with a token-only choice; answers the mint step-up and returns the secret. */
async function tokenOnlyToReveal(page: Page, codes: FreshCodes): Promise<string> {
  await page.getByTestId("service-user-access-magnon-read").getByRole("checkbox").check();
  await page.getByTestId("service-user-access-continue").getByRole("button").click();
  await expect(page.getByTestId("service-user-token-label")).toBeVisible({ timeout: 30_000 });
  await page.getByTestId("service-user-token-label").locator("input").fill("Claude Code on laptop");
  await page.getByTestId("service-user-token-submit").getByRole("button").click();
  await answerStepUp(page, await codes.next());
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
    // Minting asks for a fresh code, separate from the group step-up above.
    await answerStepUp(page, await codes.next());

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
    const { codes } = await asCreatorWithTotp(page, request, auth, "once");
    await openServiceUsers(page, "/user/service-users/new");
    const handle = `once-${Date.now()}`;
    await fillProfile(page, "Once", handle);
    await page.getByTestId("service-user-create-submit").getByRole("button").click();
    const first = await tokenOnlyToReveal(page, codes);

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
    await answerStepUp(page, await codes.next());
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
    const { email, codes } = await asCreatorWithTotp(page, request, auth, "tokcap");
    const [serviceUser] = await seedServiceUsers(request, email, `tokcap${Date.now()}`, 1);
    await serviceUserLab(request, { action: "seed_tokens", service_user_id: serviceUser.id, count: 5 });
    await openServiceUsers(page, `/user/service-users/${serviceUser.id}/token`);
    await page.getByTestId("service-user-token-scope-magnon-read").getByRole("checkbox").check();
    await page.getByTestId("service-user-token-label").locator("input").fill("Sixth");
    await page.getByTestId("service-user-token-submit").getByRole("button").click();
    // The fresh-code step-up clears before the limit check rejects the sixth token.
    await answerStepUp(page, await codes.next());
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
    const { codes } = await asCreatorWithTotp(page, request, auth, "keys");
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
    await tokenOnlyToReveal(page, codes);
    await expect(page.getByTestId("service-user-token-copy").getByRole("button")).toBeFocused();
  });

  test("pw-su-mint-step-up-happy", async ({ page, request, auth }) => {
    const { email, codes } = await asCreatorWithTotp(page, request, auth, "mintstepup");
    // Both the default desktop viewport and a phone-sized one.
    for (const viewport of [{ width: 1280, height: 800 }, { width: 390, height: 844 }]) {
      await page.setViewportSize(viewport);
      const [serviceUser] = await seedServiceUsers(request, email, `mint${viewport.width}-${Date.now()}`, 1);
      await openServiceUsers(page, `/user/service-users/${serviceUser.id}/token`);
      await page.getByTestId("service-user-token-scope-magnon-read").getByRole("checkbox").check();
      await page.getByTestId("service-user-token-label").locator("input").fill("Claude Code on laptop");
      await page.getByTestId("service-user-token-submit").getByRole("button").click();
      // Minting now demands a fresh authenticator code before any secret appears.
      await expect(page.getByTestId("step-up-dialog")).toBeVisible({ timeout: 30_000 });
      await expect(page.getByTestId("service-user-token-secret")).toHaveCount(0);
      await answerStepUp(page, await codes.next());
      await expect(page.getByTestId("service-user-token-secret")).toContainText("labtk_", {
        timeout: 30_000,
      });
    }
  });

  test("pw-su-mint-over-reach-sad", async ({ page, request, auth }) => {
    const { email, codes } = await asCreatorWithTotp(page, request, auth, "overreach");
    for (const viewport of [{ width: 1280, height: 800 }, { width: 390, height: 844 }]) {
      await page.setViewportSize(viewport);
      const [serviceUser] = await seedServiceUsers(request, email, `reach${viewport.width}-${Date.now()}`, 1);
      await openServiceUsers(page, `/user/service-users/${serviceUser.id}/token`);
      // This creator doesn't own Spectra readers, so it may not mint that scope.
      await page.getByTestId("service-user-token-scope-spectra-read").getByRole("checkbox").check();
      await page.getByTestId("service-user-token-label").locator("input").fill("Over reach");
      await page.getByTestId("service-user-token-submit").getByRole("button").click();
      // A valid code clears step-up; the server still refuses the access it can't grant.
      await answerStepUp(page, await codes.next());
      await expect(page.getByTestId("service-user-token-error")).toContainText("owners", {
        timeout: 30_000,
      });
      await expect(page.getByTestId("service-user-token-reveal")).toHaveCount(0);
      await expect(page.getByTestId("service-user-token-secret")).toHaveCount(0);
    }
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

const DESKTOP = { width: 1280, height: 720 };
const PHONE = { width: 390, height: 844 };
type Look = "desktop-light" | "phone" | "desktop-dark";

/** Run `check` on a desktop in light mode, on a phone, then on a desktop in dark mode. */
async function inEachLook(page: Page, check: (look: Look) => Promise<void>) {
  for (const look of ["desktop-light", "phone", "desktop-dark"] as const) {
    await page.setViewportSize(look === "phone" ? PHONE : DESKTOP);
    if (look === "desktop-dark") {
      await useDarkMode(page);
    }
    await check(look);
  }
}

/** Open a service user's detail page and wait until it has loaded. */
async function openDetail(page: Page, serviceUserId: string) {
  await page.goto(`/user/service-users/${serviceUserId}`);
  await waitForAppReady(page);
  await expect(page.getByTestId("service-user-detail-page")).toBeVisible({ timeout: 60_000 });
}

/** The page fits the phone's width and passes axe. */
async function expectCleanLook(page: Page, label: string) {
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  expect(overflow, `${label} scrolls sideways`).toBeLessThanOrEqual(0);
  await expectNoA11yViolations(page, label);
}

test.describe("pw-su-detail", () => {
  test("pw-su-detail-groups-happy", async ({ page, request, auth }) => {
    const { codes } = await asSuperUser(page, request, auth, "detail");
    await openServiceUsers(page, "/user/service-users/new");
    const handle = `detail-${Date.now()}`;
    await fillProfile(page, "Spectra agent", handle, "Reads metrics");
    await page.getByTestId("service-user-create-submit").getByRole("button").click();
    await expect(page.getByTestId("service-user-create-progress")).toContainText("Step 2 of 3", {
      timeout: 30_000,
    });
    await page.getByTestId("service-user-access-spectra-read").getByRole("checkbox").check();
    await page.getByTestId("service-user-access-continue").getByRole("button").click();
    await answerStepUp(page, await codes.next());
    await expect(page.getByTestId("service-user-create-progress")).toContainText("Step 3 of 3", {
      timeout: 30_000,
    });
    const serviceUserId = await serviceUserIdFromList(page, handle);

    await inEachLook(page, async (look) => {
      await openDetail(page, serviceUserId);
      const identity = page.getByTestId("service-user-detail-identity");
      await expect(identity).toContainText("Spectra agent");
      await expect(identity).toContainText(`@${handle}`);
      await expect(identity).toContainText("Reads metrics");
      await expect(page.getByTestId("service-user-detail-state")).toHaveText("Active");
      const groups = page.getByTestId("service-user-detail-groups");
      await expect(groups).toContainText("Groups (1)");
      await expect(groups.getByTestId("service-user-detail-group-row")).toHaveText([
        /Spectra readers/,
      ]);
      const permissions = page.getByTestId("service-user-detail-permissions");
      await expect(permissions).toContainText("Permissions (1)");
      const row = permissions.getByTestId("service-user-detail-permission-row");
      await expect(row).toContainText("lab.spectra.query");
      await expect(row).toContainText("Through Spectra readers");
      await expectCleanLook(page, `service user detail (${look})`);
    });
  });

  test("pw-su-detail-many-groups-scroll-happy", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "many");
    const [serviceUser] = await seedServiceUsers(request, email, `many${Date.now()}`, 1);
    await serviceUserLab(request, {
      action: "seed_memberships",
      service_user_id: serviceUser.id,
      groups: 200,
      permissions: 300,
    });

    await inEachLook(page, async (look) => {
      await openDetail(page, serviceUser.id);
      await expect(page.getByTestId("service-user-detail-groups")).toContainText("Groups (200)");
      await expect(page.getByTestId("service-user-detail-permissions")).toContainText(
        "Permissions (300)",
      );

      // Pause stays on the first screen, and the whole page is under two screens tall.
      const viewportHeight = page.viewportSize()?.height ?? 0;
      const pauseBox = await page.getByTestId("service-user-detail-pause").boundingBox();
      expect(pauseBox, "Pause button is laid out").not.toBeNull();
      expect(pauseBox!.y + pauseBox!.height, `Pause in reach (${look})`).toBeLessThanOrEqual(
        viewportHeight,
      );
      const pageHeight = await page
        .getByTestId("service-user-detail-page")
        .evaluate((el) => el.getBoundingClientRect().height);
      expect(pageHeight, `page height (${look})`).toBeLessThan(2 * viewportHeight);

      for (const [card, rowId, last, total] of [
        ["service-user-detail-groups", "service-user-detail-group-row", "Lab group 199", 200],
        [
          "service-user-detail-permissions",
          "service-user-detail-permission-row",
          "lab.seeded.perm_299",
          300,
        ],
      ] as const) {
        // Every row is rendered: nothing is cut off and nothing paginates.
        const rows = page.getByTestId(card).getByTestId(rowId);
        await expect(rows).toHaveCount(total);
        const scroller = page.getByTestId(`${card}-scroll`);
        const sizes = await scroller.evaluate((el) => ({
          scroll: el.scrollHeight,
          client: el.clientHeight,
        }));
        expect(sizes.scroll, `${card} scrolls inside its card (${look})`).toBeGreaterThan(
          sizes.client,
        );
        await scroller.scrollIntoViewIfNeeded();
        const lastRow = rows.last();
        await expect(lastRow).toContainText(last);
        await expect(lastRow).not.toBeInViewport();
        await scroller.evaluate((el) => {
          el.scrollTop = el.scrollHeight;
        });
        await expect(lastRow).toBeInViewport();
      }
      await expectCleanLook(page, `service user with many groups (${look})`);
    });
  });

  test("pw-su-pause-resume-happy", async ({ page, request, auth }) => {
    const { email, codes } = await asCreatorWithTotp(page, request, auth, "pause");
    const [serviceUser] = await seedServiceUsers(request, email, `pause${Date.now()}`, 1);

    await inEachLook(page, async (look) => {
      await openDetail(page, serviceUser.id);
      const state = page.getByTestId("service-user-detail-state");
      await expect(state).toHaveText("Active");

      await page.getByTestId("service-user-detail-pause").getByRole("button").click();
      await expect(page.getByTestId("service-user-detail-confirm")).toContainText(
        "tokens stop working",
      );
      await expectNoA11yViolations(page, `pause confirm (${look})`);
      await page.getByTestId("service-user-detail-confirm-submit").getByRole("button").click();
      await expect(
        page.getByRole("status").filter({ hasText: `@${serviceUser.handle} is paused` }),
      ).toBeVisible({ timeout: 30_000 });
      await expect(state).toHaveText("Paused", { timeout: 30_000 });
      await expect(page.getByRole("button", { name: "New token" })).toBeDisabled();
      expect(
        (await serviceUserLab(request, { action: "status", service_user_id: serviceUser.id }))
          .state,
      ).toBe("suspended");
      await expectCleanLook(page, `paused service user (${look})`);

      await page.getByTestId("service-user-detail-resume").getByRole("button").click();
      await expect(page.getByTestId("service-user-detail-confirm")).toContainText(
        "tokens work again",
      );
      await page.getByTestId("service-user-detail-confirm-submit").getByRole("button").click();
      await answerStepUp(page, await codes.next());
      await expect(
        page.getByRole("status").filter({ hasText: `@${serviceUser.handle} is active again` }),
      ).toBeVisible({ timeout: 30_000 });
      await expect(state).toHaveText("Active", { timeout: 30_000 });
      expect(
        (await serviceUserLab(request, { action: "status", service_user_id: serviceUser.id }))
          .state,
      ).toBe("active");
    });
  });

  test("pw-su-pause-forbidden-sad", async ({ page, request, auth }) => {
    const other = uniqueEmail("pauseowner");
    // Seed the other owner without signing in as them.
    await request.post("/api/test/seed-data", {
      data: { scenario: "auth_basic_user", email: other, password: PASSWORD },
    });
    await serviceUserLab(request, { action: "join_creators", email: other });
    const [theirs] = await seedServiceUsers(request, other, `theirs${Date.now()}`, 1);

    const me = await asCreator(page, request, auth, "pauseintruder");
    await inEachLook(page, async (look) => {
      await openDetail(page, theirs.id);
      const nf = page.getByTestId("service-user-detail-not-found");
      await expect(nf).toContainText("Service user not found");
      await expect(nf).toContainText("isn't yours to see");
      await expect(page.getByTestId("service-user-detail-pause")).toHaveCount(0);
      await expect(page.getByTestId("service-user-detail-groups")).toHaveCount(0);
      await expectCleanLook(page, `someone else's service user (${look})`);
    });

    // Learn the pause call from pausing a service user of our own, then aim it at theirs.
    const [mine] = await seedServiceUsers(request, me, `mine${Date.now()}`, 1);
    await openDetail(page, mine.id);
    await page.getByTestId("service-user-detail-pause").getByRole("button").click();
    const pauseCall = page.waitForRequest(
      (r) => r.method() === "POST" && (r.postData() ?? "").includes(mine.id),
    );
    await page.getByTestId("service-user-detail-confirm-submit").getByRole("button").click();
    const call = await pauseCall;
    await expect(page.getByTestId("service-user-detail-state")).toHaveText("Paused", {
      timeout: 30_000,
    });
    const refused = await page.request.post(call.url(), {
      headers: { "content-type": call.headers()["content-type"] ?? "application/x-www-form-urlencoded" },
      data: (call.postData() ?? "").replace(mine.id, theirs.id),
    });
    expect(await refused.text()).toContain("reason_class=not_found");
    expect(
      (await serviceUserLab(request, { action: "status", service_user_id: theirs.id })).state,
    ).toBe("active");
  });
});

const LOOKS = ["desktop-light", "phone", "desktop-dark"] as const;

/** Open the Delete dialog on the detail page. */
async function openDeleteDialog(page: Page) {
  await page.getByTestId("service-user-detail-delete").getByRole("button").click();
  await expect(page.getByTestId("service-user-delete-confirm")).toContainText(
    "can't be brought back",
  );
}

/** Seed `count` tokens on a service user; returns their public ids, oldest label first. */
async function seedTokens(request: Request, serviceUserId: string, count: number) {
  const out = await serviceUserLab(request, {
    action: "seed_tokens",
    service_user_id: serviceUserId,
    count,
  });
  return out.tokens as string[];
}

/** Public ids of the tokens the lab still holds for a service user. */
async function labTokens(request: Request, serviceUserId: string) {
  const out = await serviceUserLab(request, { action: "tokens", service_user_id: serviceUserId });
  return out.tokens as string[];
}

test.describe("pw-su-delete-tokens", () => {
  test("pw-su-delete-confirm-happy", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "delete");
    const seeded = await seedServiceUsers(request, email, `del${Date.now()}`, LOOKS.length);

    await inEachLook(page, async (look) => {
      const serviceUser = seeded[LOOKS.indexOf(look)];
      await openDetail(page, serviceUser.id);
      await openDeleteDialog(page);
      const submit = page.getByTestId("service-user-delete-submit").getByRole("button");
      await expect(submit).toBeDisabled();
      await page
        .getByTestId("service-user-delete-handle")
        .locator("input")
        .fill(`@${serviceUser.handle}`);
      await expect(submit).toBeEnabled();
      await expect(page.getByTestId("service-user-delete-mismatch")).toHaveCount(0);
      await expectNoA11yViolations(page, `delete confirm (${look})`);
      await submit.click();

      await expect(
        page.getByRole("status").filter({ hasText: `@${serviceUser.handle} deleted` }),
      ).toBeVisible({ timeout: 30_000 });
      await expect(page).toHaveURL(/\/user\/service-users$/, { timeout: 30_000 });
      await expect(
        page.getByTestId("service-users-count").or(page.getByTestId("service-users-empty")),
      ).toBeVisible({ timeout: 60_000 });
      await expect(page.getByTestId(`service-user-card-${serviceUser.handle}`)).toHaveCount(0);
      await expectCleanLook(page, `service users after delete (${look})`);

      await openDetail(page, serviceUser.id);
      await expect(page.getByTestId("service-user-detail-not-found")).toContainText(
        "Service user not found",
      );
    });
  });

  test("pw-su-delete-wrong-name-sad", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "delwrong");
    const [serviceUser, mine] = await seedServiceUsers(
      request,
      email,
      `delwrong${Date.now()}`,
      2,
    );
    const wrong = `${serviceUser.handle}x`;

    await inEachLook(page, async (look) => {
      await openDetail(page, serviceUser.id);
      await openDeleteDialog(page);
      await page.getByTestId("service-user-delete-handle").locator("input").fill(wrong);
      await expect(page.getByTestId("service-user-delete-mismatch")).toContainText(
        `doesn't match @${serviceUser.handle}`,
      );
      await expect(
        page.getByTestId("service-user-delete-submit").getByRole("button"),
      ).toBeDisabled();
      await expectNoA11yViolations(page, `delete wrong handle (${look})`);
      await page.getByRole("dialog").getByRole("button", { name: "Cancel" }).click();
      await expect(page.getByTestId("service-user-detail-state")).toHaveText("Active");
    });

    // The server checks the handle too: learn the delete call from deleting another
    // service user, then send it for this one with the wrong handle.
    await openDetail(page, mine.id);
    await openDeleteDialog(page);
    await page.getByTestId("service-user-delete-handle").locator("input").fill(mine.handle);
    const deleteCall = page.waitForRequest(
      (r) => r.method() === "POST" && (r.postData() ?? "").includes(mine.id),
    );
    await page.getByTestId("service-user-delete-submit").getByRole("button").click();
    const call = await deleteCall;
    await expect(page).toHaveURL(/\/user\/service-users$/, { timeout: 30_000 });
    const refused = await page.request.post(call.url(), {
      headers: {
        "content-type": call.headers()["content-type"] ?? "application/x-www-form-urlencoded",
      },
      data: (call.postData() ?? "").replace(mine.id, serviceUser.id).replace(mine.handle, wrong),
    });
    expect(await refused.text()).toContain("reason_class=confirm");
    expect(
      (await serviceUserLab(request, { action: "status", service_user_id: serviceUser.id }))
        .state,
    ).toBe("active");
  });

  test("pw-su-token-revoke-happy", async ({ page, request, auth }) => {
    const email = await asCreator(page, request, auth, "revoke");
    const [serviceUser] = await seedServiceUsers(request, email, `revoke${Date.now()}`, 1);

    await inEachLook(page, async (look) => {
      const [used, unused] = await seedTokens(request, serviceUser.id, 2);
      await openDetail(page, serviceUser.id);
      const card = page.getByTestId("service-user-detail-tokens");
      await expect(card).toContainText("Tokens (2)");
      await expect(card.getByTestId("service-user-detail-token-row")).toHaveCount(2);
      const usedRow = card.locator(`[data-token-id="${used}"]`);
      const unusedRow = card.locator(`[data-token-id="${unused}"]`);
      await expect(usedRow).toContainText("Seeded token 0");
      await expect(usedRow).toContainText("Last used");
      await expect(unusedRow).toContainText("Never used");
      await expectCleanLook(page, `service user tokens (${look})`);

      await usedRow.getByTestId("service-user-detail-token-revoke").getByRole("button").click();
      const dialog = page.getByRole("dialog");
      await expect(dialog.getByTestId("service-user-token-revoke-confirm")).toContainText(
        "stops working",
      );
      await expectNoA11yViolations(page, `revoke confirm (${look})`);
      await dialog.getByTestId("service-user-token-revoke-submit").getByRole("button").click();

      await expect(
        page.getByRole("status").filter({ hasText: "Token “Seeded token 0” revoked" }),
      ).toBeVisible({ timeout: 30_000 });
      await expect(card).toContainText("Tokens (1)", { timeout: 30_000 });
      await expect(usedRow).toHaveCount(0);
      await expect(unusedRow).toBeVisible();
      expect(await labTokens(request, serviceUser.id)).toEqual([unused]);
    });
  });

  test("pw-su-token-revoke-forbidden-sad", async ({ page, request, auth }) => {
    const other = uniqueEmail("revokeowner");
    // Seed the other owner without signing in as them.
    await request.post("/api/test/seed-data", {
      data: { scenario: "auth_basic_user", email: other, password: PASSWORD },
    });
    await serviceUserLab(request, { action: "join_creators", email: other });
    const [theirs] = await seedServiceUsers(request, other, `theirtok${Date.now()}`, 1);
    const [theirToken] = await seedTokens(request, theirs.id, 1);

    const me = await asCreator(page, request, auth, "revokeintruder");
    await inEachLook(page, async (look) => {
      await openDetail(page, theirs.id);
      await expect(page.getByTestId("service-user-detail-not-found")).toContainText(
        "isn't yours to see",
      );
      await expect(page.getByTestId("service-user-detail-tokens")).toHaveCount(0);
      await expect(page.getByTestId("service-user-detail-token-revoke")).toHaveCount(0);
      await expectCleanLook(page, `someone else's tokens (${look})`);
    });

    // Learn the revoke call from revoking a token of our own, then aim it at theirs.
    const [mine] = await seedServiceUsers(request, me, `mytok${Date.now()}`, 1);
    const [myToken] = await seedTokens(request, mine.id, 1);
    await openDetail(page, mine.id);
    await page.getByTestId("service-user-detail-token-revoke").getByRole("button").click();
    const revokeCall = page.waitForRequest(
      (r) => r.method() === "POST" && (r.postData() ?? "").includes(myToken),
    );
    await page
      .getByRole("dialog")
      .getByTestId("service-user-token-revoke-submit")
      .getByRole("button")
      .click();
    const call = await revokeCall;
    await expect(page.getByTestId("service-user-detail-tokens")).toContainText("Tokens (0)", {
      timeout: 30_000,
    });
    const replay = (data: string) =>
      page.request.post(call.url(), {
        headers: {
          "content-type": call.headers()["content-type"] ?? "application/x-www-form-urlencoded",
        },
        data,
      });
    const body = call.postData() ?? "";

    // Their service user: hidden, so not found.
    const hidden = await replay(body.replace(mine.id, theirs.id).replace(myToken, theirToken));
    expect(await hidden.text()).toContain("reason_class=not_found");
    // Our service user with their token: refused.
    const crossed = await replay(body.replace(myToken, theirToken));
    expect(await crossed.text()).toContain("reason_class=forbidden");
    expect(await labTokens(request, theirs.id)).toEqual([theirToken]);
  });
});
