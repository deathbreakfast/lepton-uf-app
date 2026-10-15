/**
 * Live provider run of the "Sign-in and delivery" page against a deployed cell.
 *
 * Skipped unless `UF_LIVE_DELIVERY=1`. Driven by operator tooling maintained
 * separately; credentials come from the operator's shell, never from this repo.
 * A provider accepting the test message is the assertion: the spec does not
 * read an inbox or a phone.
 */
import { test, expect, clickTestId, waitForAppReady, authDialog, fillAndSubmitSignIn } from "./fixtures";
import { FreshCodes } from "./helpers/fresh_totp";
import { totpCode } from "./helpers/totp";

type Page = import("@playwright/test").Page;

function need(name: string): string {
  const v = process.env[name];
  if (!v) {
    throw new Error(`missing env ${name} (see uf-live-cloud-lab/lepton/README.md)`);
  }
  return v;
}

async function signInOperator(page: Page): Promise<void> {
  await page.goto("/auth/signin");
  await waitForAppReady(page);
  const root = authDialog(page);
  await expect(root).toBeVisible({ timeout: 60_000 });
  await root.getByTestId("signin-email").locator('input[name="email"]').fill(need("UF_LIVE_OPERATOR_EMAIL"));
  await root.getByTestId("signin-password").locator('input[name="password"]').fill(need("UF_LIVE_OPERATOR_PASSWORD"));
  await root.getByTestId("signin-submit").getByRole("button").click();
  await expect(root.getByTestId("signin-mfa-step")).toBeVisible({ timeout: 30_000 });
  await root
    .getByTestId("signin-mfa-totp")
    .locator('input[name="code"]')
    .fill(totpCode(need("UF_LIVE_OPERATOR_TOTP_SECRET")));
  await root.getByTestId("signin-mfa-submit").getByRole("button").click();
  await expect(page).not.toHaveURL(/\/auth\/signin/, { timeout: 60_000 });
}

async function confirmStepUp(page: Page, code: string): Promise<void> {
  const dialog = page.getByTestId("step-up-dialog");
  await expect(dialog).toBeVisible({ timeout: 30_000 });
  await dialog.getByTestId("step-up-totp").locator("input").fill(code);
  await dialog.getByTestId("step-up-submit").getByRole("button").click();
}

async function saveSms(page: Page, codes: FreshCodes, keySid: string, secret: string): Promise<void> {
  const card = page.getByTestId("delivery-sms-card");
  await card.getByRole("radio", { name: "Twilio Verify" }).check();
  await page.getByTestId("delivery-sms-account-sid").locator("input").fill(need("UF_TWILIO_ACCOUNT_SID"));
  await page.getByTestId("delivery-sms-api-key-sid").locator("input").fill(keySid);
  await page.getByTestId("delivery-sms-secret").locator("input").fill(secret);
  await page.getByTestId("delivery-sms-verify-sid").locator("input").fill(need("UF_TWILIO_VERIFY_SERVICE_SID"));
  await page.getByTestId("delivery-sms-recipient").locator("input").fill(need("UF_LIVE_SEND_TO_PHONE"));
  await clickTestId(page, "delivery-sms-save");
  await confirmStepUp(page, await codes.next());
  await expect(page.getByTestId("delivery-sms-outcome")).toContainText("Text-message settings saved", {
    timeout: 90_000,
  });
}

test.describe("pw-lepton-delivery-settings-live", () => {
  test.skip(process.env.UF_LIVE_DELIVERY !== "1", "set UF_LIVE_DELIVERY=1 against a deployed cell");

  test("pw-lepton-delivery-settings-live-happy", async ({ page }) => {
    test.setTimeout(420_000);
    await signInOperator(page);
    const codes = new FreshCodes(need("UF_LIVE_OPERATOR_TOTP_SECRET"));
    await page.goto("/user/delivery");
    await waitForAppReady(page);
    await expect(page.getByTestId("delivery-email-card")).toBeVisible({ timeout: 60_000 });

    // SendGrid: the page's save sends a real test message first.
    const email = page.getByTestId("delivery-email-card");
    await email.getByRole("radio", { name: "SendGrid" }).check();
    await page.getByTestId("delivery-email-sendgrid-key").locator("input").fill(need("UF_LIVE_SENDGRID_API_KEY"));
    await page.getByTestId("delivery-email-from").locator("input").fill(need("UF_LIVE_EMAIL_FROM"));
    await page.getByTestId("delivery-email-recipient").locator("input").fill(need("UF_LIVE_SEND_TO_EMAIL"));
    await clickTestId(page, "delivery-email-save");
    await confirmStepUp(page, await codes.next());
    await expect(page.getByTestId("delivery-email-outcome")).toContainText("Email settings saved", {
      timeout: 90_000,
    });

    // Twilio Verify: save, then rotate to a second API key and secret with no restart.
    await saveSms(page, codes, need("UF_TWILIO_API_KEY"), need("UF_TWILIO_API_SECRET"));
    await saveSms(
      page,
      codes,
      need("UF_LIVE_TWILIO_ROTATED_API_KEY"),
      need("UF_LIVE_TWILIO_ROTATED_API_SECRET"),
    );
    // With the secret field empty, the test uses the stored (rotated) secret.
    await clickTestId(page, "delivery-sms-test");
    await expect(page.getByTestId("delivery-sms-outcome")).toContainText("Test code sent", {
      timeout: 90_000,
    });
  });

  test("pw-lepton-delivery-settings-live-denied-sad", async ({ page }) => {
    await page.goto("/auth/signin");
    await fillAndSubmitSignIn(page, need("UF_LIVE_PLAIN_EMAIL"), need("UF_LIVE_PLAIN_PASSWORD"));
    await page.goto("/user/delivery");
    await waitForAppReady(page);
    await expect(page.getByTestId("delivery-denied")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("delivery-email-card")).toHaveCount(0);
  });
});
