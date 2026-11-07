/**
 * Shared steps for the "Sign-in and delivery" specs.
 */
import { expect } from "@playwright/test";
import { clickTestId, waitForAppReady } from "../fixtures";

type Page = import("@playwright/test").Page;

/** Type a write-only secret through its Set / Replace sheet. */
export async function setSecret(page: Page, testId: string, value: string) {
  await clickTestId(page, `${testId}-open`);
  const sheet = page.getByTestId(`${testId}-dialog`);
  await expect(sheet).toBeVisible({ timeout: 30_000 });
  await sheet.getByTestId(`${testId}-input`).locator("input").fill(value);
  await sheet.getByTestId(`${testId}-confirm`).getByRole("button").click();
  await expect(sheet).toBeHidden({ timeout: 30_000 });
  await expect(page.getByTestId(testId)).toContainText("New value ready to save");
}

/**
 * Save Dark on the Appearance page, starting from the loaded delivery page,
 * then come back. A cold goto to /user/appearance can render before the
 * session is known.
 */
export async function useDarkModeFromDelivery(page: Page): Promise<void> {
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
  await page.goto("/user/delivery");
  await waitForAppReady(page);
  await expect(page.getByTestId("delivery-email-card")).toBeVisible({ timeout: 60_000 });
  const scheme = await page.evaluate(() => {
    const el = document.querySelector(".orbital-theme-provider");
    return el ? getComputedStyle(el).getPropertyValue("--orb-color-scheme").trim() : "";
  });
  expect(scheme, "delivery page renders dark").toBe("dark");
}
