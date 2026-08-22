import { expect, test } from "@playwright/test";
import { installTauriMock } from "./tauri-mock";

test.beforeEach(async ({ page }) => {
  await installTauriMock(page);
  await page.goto("/");
});

test("renders the private streaming shell without interface errors", async ({ page }) => {
  await expect(page.getByRole("navigation", { name: "Primary navigation" })).toBeVisible();
  await expect(page.getByText("Nice Downloader could not draw this workspace.")).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Your library" })).toBeVisible();
  await expect(page.getByText(/Unhandled Playwright Tauri command/)).toHaveCount(0);
});

test("global keyboard shortcuts preserve navigation and focus", async ({ page }) => {
  await page.keyboard.press("Control+n");
  await expect(page.locator("#media-url")).toBeFocused();
  await page.keyboard.press("Control+,");
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
});

test("command search opens and closes with focus restoration", async ({ page }) => {
  const trigger = page.getByRole("button", { name: /^Search/ });
  await trigger.focus();
  await page.keyboard.press("Control+k");
  await expect(page.getByRole("dialog", { name: /search media and run/i })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(trigger).toBeFocused();
});

test("Library exposes live Smart Collections without crowding the catalog", async ({ page }) => {
  await page.getByRole("button", { name: "Library" }).click();
  await expect(page.getByRole("heading", { name: "Smart Collections" })).toBeVisible();
  await expect(page.getByText("Build a shelf around what matters to you")).toBeVisible();
  await page.getByRole("button", { name: "Create from current filters" }).click();
  await expect(page.getByRole("textbox", { name: "Smart Collection name" })).toBeFocused();
});

test("watched folders expose typed automation controls", async ({ page }) => {
  await page.getByRole("button", { name: "Library" }).click();
  await page.getByRole("button", { name: /^Manage/ }).click();
  await expect(page.getByRole("heading", { name: "Watch-folder automation" })).toBeVisible();
  await page.getByRole("button", { name: /Playwright media Auto scan enabled/ }).click();
  await expect(page.getByLabel("Add tags")).toBeVisible();
  await expect(page.getByRole("button", { name: "Save automation" })).toBeVisible();
});
