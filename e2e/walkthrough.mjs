// End-to-end walkthrough of the UI against the in-browser mock backend.
// Fails on any page error. Set SHOTS=<dir> to also save screenshots and
// PW_EXECUTABLE=<path> to use a system Chromium instead of Playwright's.
//
//   npx vite --port 1420 &   # or `npm run dev`
//   npm run e2e
import { chromium } from "playwright";

const BASE = process.env.E2E_URL ?? "http://localhost:1420/";
const OUT = process.env.SHOTS ?? null;
const shot = async (page, name) => OUT && page.screenshot({ path: `${OUT}/${name}.png` });

const browser = await chromium.launch(process.env.PW_EXECUTABLE ? { executablePath: process.env.PW_EXECUTABLE, args: ["--no-sandbox"] } : {});
const ctx = await browser.newContext({ viewport: { width: 1280, height: 820 }, colorScheme: process.env.DARK ? "dark" : "light" });
const page = await ctx.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => m.type() === "error" && !m.text().includes("404") && errors.push(m.text()));

try {
  await page.goto(BASE);
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.waitForSelector("text=Keep the project.");
  await shot(page, "01-welcome");

  // Add a folder and scan.
  await page.click("text=Choose project folders");
  await page.fill("input[placeholder*='type a path']", "~/Projects");
  await page.click("[role=dialog] button:has-text('Add')");
  await page.click("[role=dialog] button:has-text('Scan 1 folder')");
  await page.waitForSelector("text=Potential recovery");
  await page.waitForSelector("text=Last scan today", { timeout: 60000 });
  await shot(page, "02-overview");

  // Projects: filter, select, keyboard, drawer.
  await page.keyboard.press("2");
  await page.waitForSelector("table");
  await page.click("text=Inactive > 30 days");
  await page.click("button:has-text('Select')");
  await page.click("text=Select dormant");
  const selected = await page.locator("text=projects selected").innerText();
  if (!/\d+ projects selected/.test(selected)) throw new Error(`selection bar missing: ${selected}`);
  await page.focus("tbody tr[data-id]");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await page.waitForSelector("text=Cleanup breakdown");
  await shot(page, "03-drawer");
  await page.keyboard.press("Escape");

  // Command palette. Wait for it to unmount before sending the next
  // shortcut: while its input still has focus the shortcut is ignored by
  // design, which would otherwise make the next step racy.
  await page.keyboard.press("Control+k");
  await page.waitForSelector("[aria-label='Command palette']");
  // The input takes focus a tick after mount; typing before that would send
  // the keystrokes nowhere.
  await page.waitForFunction(() => document.activeElement?.getAttribute("aria-label") === "Search commands");
  await page.keyboard.type("full");
  await page.keyboard.press("Escape");
  await page.waitForSelector("[aria-label='Command palette']", { state: "detached" });

  // Review, untick one folder, hibernate.
  // Scope the wait to the dialog: "Estimated recovery" also appears in the
  // bulk action bar, so an unscoped wait passes before the dialog opens.
  await page.keyboard.press("h");
  await page.waitForSelector("[role=dialog]:has-text('Estimated recovery')");
  await page.click("[role=dialog] button:has-text('Review files')");
  const before = await page.locator("[role=dialog] .text-accent").first().innerText();
  await page.locator("[role=dialog] [role=checkbox][aria-checked='true']").first().click();
  await page.waitForFunction((b) => document.querySelector("[role=dialog] .text-accent")?.textContent !== b, before);
  await shot(page, "04-review");
  await page.click("[role=dialog] button:has-text('Hibernate')");
  await page.waitForSelector("text=Cleanup Complete", { timeout: 120000 });
  await shot(page, "05-complete");
  await page.click("button:has-text('View History')");
  await page.waitForSelector("text=recovered across");
  await shot(page, "06-history");

  // Wake a hibernated project.
  await page.keyboard.press("2");
  await page.click("button:has-text('Filters')");
  await page.locator("label:has-text('Only hibernated') button").click();
  await page.click("button:has-text('Wake') >> nth=0");
  await page.waitForSelector("text=Runs inside");
  await page.click("[role=dialog] button:has-text('Run Command')");
  await page.waitForSelector("text=Completed in", { timeout: 30000 });
  await shot(page, "07-wake");
  await page.click("[role=dialog] button:has-text('Done')");

  // Settings: rule preview + caches on overview.
  await page.keyboard.press("4");
  await page.fill("input[placeholder='.storybook-cache']", ".storybook-cache");
  await page.waitForSelector("text=Would match", { timeout: 10000 });
  await page.click("nav >> text=Overview");
  await page.click("button:has-text('Measure')");
  await page.waitForSelector("text=Cargo registry", { timeout: 10000 });
  await shot(page, "08-overview-caches");
} finally {
  await browser.close();
}

if (errors.length) {
  console.error("page errors:", errors);
  process.exit(1);
}
console.log("walkthrough ok");
