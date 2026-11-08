/**
 * PersonPicker on `/dev/person-picker`: search, single and multi picks, excluded rows,
 * the empty box, the keyboard path, and source captions with kind badges. Every
 * scenario runs on a desktop in light mode, on a phone, and on a desktop in dark mode.
 */
import { test, expect, waitForAppReady } from "./fixtures";
import {
  PASSWORD,
  expectNoA11yViolations,
  serviceUserLab,
  seedServiceUsers,
  uniqueEmail,
  useDarkMode,
} from "./helpers/service-users";

type Page = import("@playwright/test").Page;
type Request = import("@playwright/test").APIRequestContext;
type Locator = import("@playwright/test").Locator;

const DESKTOP = { width: 1280, height: 720 };
const PHONE = { width: 390, height: 844 };
const LOOKS = ["desktop-light", "phone", "desktop-dark"] as const;
type Look = (typeof LOOKS)[number];

/** A lowercase word no other test's names share, so searches only find this test's rows. */
function uniqueWord(): string {
  const letters = "abcdefghijklmnopqrstuvwxyz";
  let word = "q";
  for (let i = 0; i < 7; i++) word += letters[Math.floor(Math.random() * letters.length)];
  return word;
}

function capitalized(word: string): string {
  return word[0].toUpperCase() + word.slice(1);
}

async function personPickerLab(request: Request, body: Record<string, unknown>) {
  const res = await request.post("/api/test/person-picker", { data: body });
  expect(res.ok(), await res.text()).toBeTruthy();
  return (await res.json()) as Record<string, string>;
}

/** Seed a person someone else can search for; returns their user id. */
async function seedPerson(request: Request, displayName: string): Promise<string> {
  const email = uniqueEmail("pp-person");
  await request.post("/api/test/seed-data", {
    data: { scenario: "auth_basic_user", email, password: PASSWORD },
  });
  const out = await personPickerLab(request, {
    action: "person",
    email,
    display_name: displayName,
  });
  return out.user_id;
}

async function seedTeam(request: Request, name: string, members = 4): Promise<void> {
  await personPickerLab(request, { action: "team", name, members });
}

/** Sign in a fresh viewer in `look`, then open the demo page. Returns their email. */
async function openDemo(
  page: Page,
  auth: { signIn: (o: { email: string; password: string; referer?: string }) => Promise<void> },
  look: Look,
  query = "",
): Promise<string> {
  const email = uniqueEmail("pp-viewer");
  await page.setViewportSize(look === "phone" ? PHONE : DESKTOP);
  await auth.signIn({ email, password: PASSWORD, referer: "/user/account-settings" });
  if (look === "desktop-dark") {
    await useDarkMode(page);
  }
  await page.goto(`/dev/person-picker${query}`);
  await waitForAppReady(page);
  await expect(page.getByTestId("person-picker-demo")).toBeVisible({ timeout: 60_000 });
  return email;
}

function section(page: Page, id: string): Locator {
  return page.getByTestId(id);
}

function input(page: Page, id: string): Locator {
  return section(page, id).getByRole("combobox");
}

/** The open listbox that belongs to `id`'s input. */
async function listFor(page: Page, id: string): Promise<Locator> {
  const controls = await input(page, id).getAttribute("aria-controls");
  expect(controls, "combobox names its listbox").toBeTruthy();
  return page.locator(`[id="${controls}"]`);
}

async function search(page: Page, id: string, text: string): Promise<Locator> {
  // fill() focuses without a pointer click, so an open list over the box can't eat it.
  await input(page, id).fill(text);
  return listFor(page, id);
}

function row(list: Locator, name: string): Locator {
  return list.getByTestId("person-picker-row").filter({ hasText: name });
}

function chips(page: Page, id: string): Locator {
  return section(page, id).getByTestId("person-picker-chip");
}

for (const look of LOOKS) {
  test.describe(`pw-person-picker [${look}]`, () => {
    test(`pw-person-picker-search-happy [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      await seedPerson(request, `Noor ${capitalized(word)}`);
      await openDemo(page, auth, look);

      const list = await search(page, "pp-multi", word);
      const noor = row(list, `Noor ${capitalized(word)}`);
      await expect(noor).toBeVisible({ timeout: 30_000 });
      await expect(noor).toHaveAttribute("data-kind", "user");
      await expect(noor.locator(".orbital-persona")).toBeVisible();
      await expect(noor).not.toContainText("@");
      if (look === "phone") {
        const box = await noor.boundingBox();
        expect(box?.height ?? 0).toBeGreaterThanOrEqual(44);
        const listBox = await list.boundingBox();
        const inputBox = await input(page, "pp-multi").boundingBox();
        expect(listBox?.width ?? 0).toBeGreaterThanOrEqual((inputBox?.width ?? 0) - 1);
      }
      await expectNoA11yViolations(page, `person picker list open (${look})`);
    });

    test(`pw-person-picker-empty-box-sad [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      await seedPerson(request, `Noor ${capitalized(word)}`);
      await openDemo(page, auth, look);
      const searched: string[] = [];
      page.on("request", (req) => {
        if (/search_people/i.test(req.url())) {
          searched.push(new URLSearchParams(req.postData() ?? "").get("query") ?? "");
        }
      });

      const box = input(page, "pp-multi");
      await box.click();
      const list = await listFor(page, "pp-multi");
      await expect(list.getByTestId("person-picker-status")).toContainText(
        "Type 2 or more letters to search.",
      );
      await expect(list.getByTestId("person-picker-row")).toHaveCount(0);

      await box.fill(word.slice(0, 1));
      await expect(list.getByTestId("person-picker-status")).toContainText(
        "Type 2 or more letters to search.",
      );
      await expect(list.getByTestId("person-picker-row")).toHaveCount(0);

      // A full search in another picker outlasts the debounce, so by the time it answers
      // any search for the one letter would have gone out too. None did.
      const answered = page.waitForResponse((res) => /search_people/i.test(res.url()));
      await input(page, "pp-single").fill(word);
      await answered;
      expect(searched).toEqual([word]);
      await expect(list.getByTestId("person-picker-row")).toHaveCount(0);
    });

    test(`pw-person-picker-multi-happy [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      const Word = capitalized(word);
      await seedPerson(request, `Noor ${Word}`);
      await seedTeam(request, `${Word} crew`);
      const email = await openDemo(page, auth, look);
      await serviceUserLab(request, { action: "join_creators", email });
      await seedServiceUsers(request, email, word, 1);

      let list = await search(page, "pp-multi", word);
      await row(list, `Noor ${Word}`).click();
      list = await search(page, "pp-multi", word);
      await row(list, `${Word} crew`).click();
      list = await search(page, "pp-multi", word);
      const bot = list.locator('[data-testid="person-picker-row"][data-kind="service_user"]');
      await expect(bot).toContainText("Service user · owned by you", { timeout: 30_000 });
      await bot.click();

      await expect(chips(page, "pp-multi")).toHaveCount(3);
      await expect(page.getByTestId("pp-multi-value")).toContainText(`Noor ${Word} (user)`);
      await expect(page.getByTestId("pp-multi-value")).toContainText(`${Word} crew (team)`);
      await expect(page.getByTestId("pp-multi-value")).toContainText("(service_user)");

      await section(page, "pp-multi")
        .getByRole("button", { name: `Remove ${Word} crew` })
        .click();
      await expect(chips(page, "pp-multi")).toHaveCount(2);
      await expect(page.getByTestId("pp-multi-value")).not.toContainText("crew");
    });

    test(`pw-person-picker-single-replaces-happy [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      const Word = capitalized(word);
      await seedPerson(request, `Noor ${Word}`);
      await seedPerson(request, `Riya ${Word}`);
      await openDemo(page, auth, look);

      let list = await search(page, "pp-single", word);
      await row(list, `Noor ${Word}`).click();
      await expect(chips(page, "pp-single")).toHaveCount(1);
      list = await search(page, "pp-single", word);
      await row(list, `Riya ${Word}`).click();
      await expect(chips(page, "pp-single")).toHaveCount(1);
      await expect(page.getByTestId("pp-single-value")).toHaveText(`Riya ${Word} (user)`);
    });

    test(`pw-person-picker-excluded-disabled-sad [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      const Word = capitalized(word);
      const member = await seedPerson(request, `Noor ${Word}`);
      await seedPerson(request, `Riya ${Word}`);
      await openDemo(page, auth, look, `?exclude=${member}`);

      const list = await search(page, "pp-multi", word);
      const noor = row(list, `Noor ${Word}`);
      await expect(noor).toHaveAttribute("data-disabled", "true", { timeout: 30_000 });
      await expect(list.getByRole("option", { name: `Noor ${Word}` })).toHaveAttribute(
        "aria-disabled",
        "true",
      );
      await expect(row(list, `Riya ${Word}`)).toHaveAttribute("data-disabled", "false");
      await noor.click({ force: true });
      await expect(chips(page, "pp-multi")).toHaveCount(0);
      await expect(page.getByTestId("pp-multi-value")).toHaveText("Nothing picked");
    });

    test(`pw-person-picker-keyboard-happy [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      const Word = capitalized(word);
      await seedPerson(request, `Noor ${Word}`);
      await openDemo(page, auth, look);

      const box = input(page, "pp-multi");
      await box.focus();
      await page.keyboard.type(word);
      const list = await listFor(page, "pp-multi");
      await expect(row(list, `Noor ${Word}`)).toBeVisible({ timeout: 30_000 });
      await page.keyboard.press("ArrowDown");
      await page.keyboard.press("Enter");
      await expect(chips(page, "pp-multi")).toHaveCount(1);
      await expect(box).toHaveValue("");

      await page.keyboard.press("Escape");
      await expect(box).toHaveAttribute("aria-expanded", "false");

      await page.keyboard.press("Backspace");
      await expect(chips(page, "pp-multi")).toHaveCount(0);
      await expect(page.getByTestId("pp-multi-value")).toHaveText("Nothing picked");
    });

    test(`pw-person-picker-kinds-sad [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      const Word = capitalized(word);
      await seedPerson(request, `Noor ${Word}`);
      await seedTeam(request, `${Word} crew`);
      await openDemo(page, auth, look);

      const list = await search(page, "pp-teams", word);
      await expect(row(list, `${Word} crew`)).toBeVisible({ timeout: 30_000 });
      await expect(row(list, `Noor ${Word}`)).toHaveCount(0);
      await expect(list.locator('[data-group="user"]')).toHaveCount(0);
    });

    test(`pw-person-picker-sources-happy [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      const Word = capitalized(word);
      await seedPerson(request, `Noor ${Word}`);
      await seedTeam(request, `${Word} crew`);
      await openDemo(page, auth, look);

      const list = await search(page, "pp-sources", word);
      await expect(row(list, `${Word} crew`)).toBeVisible({ timeout: 30_000 });
      await expect(list.locator('[data-group="user"]')).toContainText("People · Lepton users");
      await expect(list.locator('[data-group="team"]')).toContainText(
        "Teams · Polaron teams you belong to",
      );
      await expect(row(list, `Noor ${Word}`).getByTestId("person-picker-badge")).toHaveText(
        "Person",
      );
      await expect(row(list, `${Word} crew`).getByTestId("person-picker-badge")).toHaveText(
        "Team",
      );
      // The caller's "From this event" group doesn't match, so it isn't listed.
      await expect(list.locator('[data-group="extra-0"]')).toHaveCount(0);
      await expectNoA11yViolations(page, `person picker sources open (${look})`);
    });

    test(`pw-person-picker-sources-empty-group-sad [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      await seedPerson(request, `Riya ${capitalized(word)}`);
      await openDemo(page, auth, look);

      const list = await search(page, "pp-sources", word);
      await expect(row(list, `Riya ${capitalized(word)}`)).toBeVisible({ timeout: 30_000 });
      const teams = list.locator('[data-group="team"]');
      await expect(teams).toContainText("Teams · Polaron teams you belong to");
      await expect(teams).toContainText("No teams match.");
      await expect(teams.getByTestId("person-picker-row")).toHaveCount(0);
    });

    test(`pw-person-picker-extra-group-happy [${look}]`, async ({ page, auth }) => {
      await openDemo(page, auth, look);

      await input(page, "pp-sources").click();
      const list = await listFor(page, "pp-sources");
      const author = list.locator('[data-testid="person-picker-row"][data-kind="extra"]');
      await expect(author).toContainText("Task › author");
      await expect(author.getByTestId("person-picker-badge")).toHaveCount(0);
      await author.click();
      await expect(chips(page, "pp-sources")).toHaveCount(1);
      await expect(page.getByTestId("pp-sources-value")).toHaveText("Author (extra)");
    });

    test(`pw-person-picker-defaults-unchanged-happy [${look}]`, async ({ page, request, auth }) => {
      const word = uniqueWord();
      const Word = capitalized(word);
      await seedPerson(request, `Noor ${Word}`);
      await seedTeam(request, `${Word} crew`);
      await openDemo(page, auth, look);

      const list = await search(page, "pp-multi", word);
      await expect(row(list, `${Word} crew`)).toBeVisible({ timeout: 30_000 });
      await expect(list.locator('[data-group="user"]')).toContainText("People");
      await expect(list).not.toContainText("Lepton users");
      await expect(list).not.toContainText("Polaron teams you belong to");
      await expect(list).not.toContainText("No teams match.");
      await expect(list.getByTestId("person-picker-badge")).toHaveCount(0);
    });
  });
}
