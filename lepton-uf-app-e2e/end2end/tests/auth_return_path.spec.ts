import {
  test,
  expect,
  authDialog,
  seedTestData,
  waitForAppReady,
  fillAndSubmitSignIn,
  assertSignedInLanding,
} from "./fixtures";
import { totpCode } from "./helpers/totp";

// A gated deep link with a query: the return path must keep both, exactly.
const DEEP_LINK = "/gate/auth-required?tab=history&x=1";
const PASSWORD = "CorrectHorseBattery1!";

async function expectExactUrl(
  page: import("@playwright/test").Page,
  pathAndQuery: string,
): Promise<void> {
  await expect
    .poll(
      () => {
        const url = new URL(page.url());
        return `${url.pathname}${url.search}`;
      },
      { timeout: 60_000 },
    )
    .toBe(pathAndQuery);
}

async function openGateDialog(page: import("@playwright/test").Page) {
  await waitForAppReady(page);
  await expect(page.getByText("Sign in required")).toBeVisible({
    timeout: 60_000,
  });
  await page.getByRole("button", { name: "Sign In", exact: true }).click();
  const root = authDialog(page);
  await expect(root).toBeVisible({ timeout: 30_000 });
  return root;
}

async function submitCredentials(
  root: import("@playwright/test").Locator,
  email: string,
): Promise<void> {
  await root
    .getByTestId("signin-email")
    .locator('input[name="email"]')
    .fill(email);
  await root
    .getByTestId("signin-password")
    .locator('input[name="password"]')
    .fill(PASSWORD);
  await root.getByTestId("signin-submit").getByRole("button").click();
}

async function expectBackOnDeepLink(page: import("@playwright/test").Page) {
  await assertSignedInLanding(
    page,
    /\/gate\/auth-required\?/,
    "gate-auth-required-content",
  );
  await expectExactUrl(page, DEEP_LINK);
  await expect(page.getByText("Sign in required")).toHaveCount(0);
}

test.describe("pw-auth-return-path", () => {
  test("pw-auth-return-path-deep-link-gate-happy", async ({
    page,
    request,
  }) => {
    const email = `return-gate-${Date.now()}@example.com`;
    await seedTestData(request, "auth_basic_user", { email, password: PASSWORD });

    await page.goto(DEEP_LINK);
    const root = await openGateDialog(page);
    await submitCredentials(root, email);

    await expectBackOnDeepLink(page);
  });

  test("pw-auth-return-path-mfa-happy", async ({ page, request }) => {
    const email = `return-mfa-${Date.now()}@example.com`;
    const seeded = await seedTestData(request, "auth_user_with_totp", {
      email,
      password: PASSWORD,
    });
    if (!seeded.totp_secret) {
      throw new Error("auth_user_with_totp seed missing totp_secret");
    }

    await page.goto(DEEP_LINK);
    const root = await openGateDialog(page);
    await submitCredentials(root, email);
    await expect(root.getByTestId("signin-mfa-step")).toBeVisible({
      timeout: 30_000,
    });
    await root
      .getByTestId("signin-mfa-totp")
      .locator('input[name="code"]')
      .fill(totpCode(seeded.totp_secret));
    await root.getByTestId("signin-mfa-submit").getByRole("button").click();

    await expectBackOnDeepLink(page);
  });

  test("pw-auth-return-path-expired-session-happy", async ({
    page,
    request,
  }) => {
    const email = `return-expired-${Date.now()}@example.com`;
    await seedTestData(request, "auth_basic_user", { email, password: PASSWORD });

    await page.goto(DEEP_LINK);
    await submitCredentials(await openGateDialog(page), email);
    await expectBackOnDeepLink(page);

    // Session ends under the open page; the next load shows the gate again.
    await page.context().clearCookies();
    await page.reload();
    await submitCredentials(await openGateDialog(page), email);

    await expectBackOnDeepLink(page);
  });

  test("pw-auth-return-path-signin-route-happy", async ({ page, request }) => {
    const email = `return-route-${Date.now()}@example.com`;
    await seedTestData(request, "auth_basic_user", { email, password: PASSWORD });

    await page.goto(`/auth/signin?referer=${encodeURIComponent(DEEP_LINK)}`);
    await fillAndSubmitSignIn(page, email, PASSWORD);

    await expectBackOnDeepLink(page);
  });
});
