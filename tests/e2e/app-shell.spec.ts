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

test("bulk metadata preserves blank fields and restores dialog focus", async ({ page }) => {
  await page.getByRole("button", { name: "Library" }).click();
  await page.getByRole("button", { name: "Select" }).click();
  await page.getByRole("button", { name: /Playwright clip/ }).click();

  const metadataTrigger = page.getByRole("button", { name: "Metadata" });
  await metadataTrigger.click();
  const dialog = page.getByRole("dialog", { name: /Edit 1 selected item/ });
  await expect(dialog).toBeVisible();
  await expect(page.getByText("Only entered values are applied.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Close bulk metadata editor" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(metadataTrigger).toBeFocused();

  await metadataTrigger.click();
  await page.getByLabel("Creator or channel").fill("Curated creator");
  await page.getByLabel("Add tags").fill("favorite, archive");
  await page.getByRole("button", { name: "Apply metadata" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByText(/Updated metadata for 1 selected item/)).toBeVisible();
  await expect.poll(async () => page.evaluate(() => (window as typeof window & { __PLAYWRIGHT_BULK_METADATA_REQUEST__?: unknown }).__PLAYWRIGHT_BULK_METADATA_REQUEST__)).toEqual({
    itemIds: ["00000000-0000-4000-8000-000000000002"],
    creator: "Curated creator",
    series: null,
    episode: null,
    tags: ["favorite", "archive"],
  });
});

test("playback diagnostics explain the active renderer without exposing paths", async ({ page }) => {
  await page.getByRole("button", { name: "Watch", exact: true }).click();
  const diagnostics = page.locator("summary").filter({ hasText: "Playback diagnostics" });
  await expect(diagnostics).toBeVisible();
  await diagnostics.click();
  await expect(page.getByText("Direct WebView video", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("MP4 · H.264 video · AAC audio", { exact: true })).toBeVisible();
  await expect(page.getByText("H.264 and AAC in MP4 are expected to play directly.", { exact: true })).toBeVisible();
  await expect(page.getByText("/tmp/media/playwright-clip.mp4", { exact: true })).toHaveCount(0);
});

test("Library activity inbox links attention counts to existing workspaces", async ({ page }) => {
  const inbox = page.locator("details").filter({ hasText: "Library activity" });
  await expect(inbox).toContainText("1 attention signal");
  await inbox.locator("summary").click();
  const curation = inbox.getByRole("button", { name: /Curation queue/ });
  await expect(curation).toContainText("1");
  await curation.click();
  await expect(page.getByRole("heading", { name: "Review your library quickly" })).toBeVisible();
});
