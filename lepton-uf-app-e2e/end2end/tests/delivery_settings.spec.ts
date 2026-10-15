import {
  test,
  expect,
  clickTestId,
  signupNewUser,
  waitForAppReady,
} from "./fixtures";
import { FreshCodes } from "./helpers/fresh_totp";

type Page = import("@playwright/test").Page;
type Request = import("@playwright/test").APIRequestContext;

type MockCall = { provider: string; authorization: string; to: string };

async function mockCalls(request: Request): Promise<MockCall[]> {
  const res = await request.get("/api/test/mock-providers");
  expect(res.ok(), await res.text()).toBeTruthy();
  return (await res.json()) as MockCall[];
}

async function resetMocks(request: Request): Promise<void> {
  const res = await request.post("/api/test/mock-providers/reset");
  expect(res.ok()).toBeTruthy();
}

/** Seed a TOTP user, sign in, make them a delivery admin, open the page. */
async function openAsDeliveryAdmin(
  page: Page,
  request: Request,
  auth: {
    signInWithTotp: (o: {
      email: string;
      password: string;
      referer?: string;
    }) => Promise<{ totp_secret: string }>;
  },
  label: string,
): Promise<{ email: string; secret: string; codes: FreshCodes }> {
  const email = `delivery-${label}-${Date.now()}@example.com`;
  const password = "CorrectHorseBattery1!";
  const { totp_secret } = await auth.signInWithTotp({
    email,
    password,
    referer: "/user/account-settings",
  });
  const added = await request.post("/api/test/delivery-admin", {
    data: { email },
  });
  expect(added.status(), "admin group add").toBe(204);
  await page.goto("/user/delivery");
  await waitForAppReady(page);
  await expect(page.getByTestId("delivery-settings-container")).toBeVisible({
    timeout: 60_000,
  });
  await expect(page.getByTestId("delivery-email-card")).toBeVisible({
    timeout: 60_000,
  });
  return { email, secret: totp_secret, codes: new FreshCodes(totp_secret) };
}

async function chooseSendGrid(page: Page, key: string, from: string, to: string) {
  const card = page.getByTestId("delivery-email-card");
  await card.getByRole("radio", { name: "SendGrid" }).check();
  await page
    .getByTestId("delivery-email-sendgrid-key")
    .locator("input")
    .fill(key);
  await page.getByTestId("delivery-email-from").locator("input").fill(from);
  await page.getByTestId("delivery-email-recipient").locator("input").fill(to);
}

/** Name of the email driver radio that is currently selected. */
async function selectedEmailDriver(page: Page): Promise<string> {
  const card = page.getByTestId("delivery-email-card");
  for (const name of ["Off", "SMTP", "SendGrid", "Direct to recipient"]) {
    if (await card.getByRole("radio", { name }).isChecked()) {
      return name;
    }
  }
  throw new Error("no email driver radio is selected");
}

async function confirmStepUp(page: Page, code: string) {
  const dialog = page.getByTestId("step-up-dialog");
  await expect(dialog).toBeVisible({ timeout: 30_000 });
  await dialog.getByTestId("step-up-totp").locator("input").fill(code);
  await dialog.getByTestId("step-up-submit").getByRole("button").click();
}

test.describe("pw-lepton-delivery-settings", () => {
  test("pw-lepton-delivery-settings-happy", async ({ page, request, auth }) => {
    await resetMocks(request);
    const { codes } = await openAsDeliveryAdmin(page, request, auth, "happy");

    // Secrets are write-only: nothing on the page can hold a stored value.
    await expect(page.getByTestId("delivery-sms-card")).toBeVisible();
    await expect(page.locator("body")).not.toContainText("lab-initial-twilio-secret");

    await chooseSendGrid(page, "SG.e2e-key-one", "cell@example.test", "ops@example.test");
    await clickTestId(page, "delivery-email-save");
    await confirmStepUp(page, await codes.next());
    await expect(page.getByTestId("delivery-email-outcome")).toContainText(
      "Email settings saved",
      { timeout: 60_000 },
    );

    // The save sent one test message with the key we typed.
    const afterSave = await mockCalls(request);
    const sendgrid = afterSave.filter((c) => c.provider === "sendgrid");
    expect(sendgrid.length).toBe(1);
    expect(sendgrid[0].authorization).toBe("Bearer SG.e2e-key-one");
    expect(sendgrid[0].to).toBe("ops@example.test");

    // After reload the key shows as set, and its value appears nowhere.
    await page.reload();
    await waitForAppReady(page);
    await expect(page.getByTestId("delivery-email-card")).toContainText("Set, changed");
    await expect(page.locator("body")).not.toContainText("SG.e2e-key-one");

    // The change is live without a restart: a new sign-up's verification email
    // goes through SendGrid, so the mock sees it.
    await page.context().clearCookies();
    const newcomer = `delivery-newcomer-${Date.now()}@example.com`;
    await signupNewUser(page, newcomer);
    const afterSignup = await mockCalls(request);
    expect(
      afterSignup.some((c) => c.provider === "sendgrid" && c.to === newcomer),
      "sign-up verification email used the saved SendGrid settings",
    ).toBeTruthy();
  });

  test("pw-lepton-delivery-settings-denied-sad", async ({ page, request, auth }) => {
    const email = `delivery-denied-${Date.now()}@example.com`;
    await auth.signIn({ email, password: "CorrectHorseBattery1!", referer: "/user/account-settings" });
    await page.goto("/user/delivery");
    await waitForAppReady(page);
    await expect(page.getByTestId("delivery-denied")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("delivery-denied")).toContainText(
      "lepton.delivery.admins",
    );
    // No form, and no value, is rendered for a user without the permission.
    await expect(page.getByTestId("delivery-email-card")).toHaveCount(0);
    await expect(page.getByTestId("delivery-sms-card")).toHaveCount(0);
  });

  test("pw-lepton-delivery-settings-test-send-failed-sad", async ({
    page,
    request,
    auth,
  }) => {
    await resetMocks(request);
    const failing = await request.post("/api/test/mock-providers/fail/sendgrid/401");
    expect(failing.status()).toBe(204);
    const { codes } = await openAsDeliveryAdmin(page, request, auth, "testfail");
    const before = await selectedEmailDriver(page);

    await chooseSendGrid(page, "SG.rejected", "cell@example.test", "ops@example.test");
    await clickTestId(page, "delivery-email-test");
    await expect(page.getByTestId("delivery-email-outcome")).toContainText(
      "rejected the credentials",
      { timeout: 60_000 },
    );

    // Saving runs the same test first, so it fails too and stores nothing.
    await clickTestId(page, "delivery-email-save");
    await confirmStepUp(page, await codes.next());
    await expect(page.getByTestId("delivery-email-outcome")).toContainText(
      "Nothing was saved",
      { timeout: 60_000 },
    );

    await page.reload();
    await waitForAppReady(page);
    // The failed save changed nothing: same driver as before the attempt.
    expect(await selectedEmailDriver(page)).toBe(before);
    await resetMocks(request);
  });

  test("pw-lepton-delivery-settings-stepup-sad", async ({ page, request, auth }) => {
    await resetMocks(request);
    await openAsDeliveryAdmin(page, request, auth, "stepup");
    const before = await selectedEmailDriver(page);

    await chooseSendGrid(page, "SG.never-saved", "cell@example.test", "ops@example.test");
    await clickTestId(page, "delivery-email-save");
    await confirmStepUp(page, "000000");
    await expect(
      page.getByTestId("step-up-dialog").getByTestId("step-up-error"),
    ).toBeVisible({ timeout: 30_000 });

    // The wrong code reached no provider and saved nothing.
    const calls = await mockCalls(request);
    expect(calls.filter((c) => c.provider === "sendgrid")).toHaveLength(0);
    await page.reload();
    await waitForAppReady(page);
    expect(await selectedEmailDriver(page)).toBe(before);
  });
});
