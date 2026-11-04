import { mkdir } from 'node:fs/promises';
import { chromium, expect } from '@playwright/test';
import { createServer } from 'vite';

const server = await createServer({
  cacheDir: 'test-results/vite-cache',
  server: { host: '127.0.0.1', port: 7100, strictPort: true },
});
let browser;
const errors = [];
try {
  await server.listen();
  browser = await chromium.launch({ channel: 'msedge', headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 920 }, deviceScaleFactor: 1 });
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text());
  });
  await page.goto('http://127.0.0.1:7100', { waitUntil: 'domcontentloaded', timeout: 90000 });
  await page.locator('.loading-screen').waitFor({ state: 'detached', timeout: 90000 });
  await page.getByText('Welcome to Bevernote', { exact: true }).first().waitFor({ timeout: 90000 });
  await mkdir('test-results', { recursive: true });
  await page.screenshot({ path: 'test-results/bevernote-desktop.png', fullPage: true });

  await page.getByRole('button', { name: 'New note', exact: false }).first().click();
  await page.getByLabel('Note title').fill('My offline field note');
  await page.locator('.note-prose').click();
  await page.locator('.note-prose').pressSequentially('Fast, private, and available anywhere.');
  await page.getByLabel('Add a tag').fill('testing');
  await page.getByLabel('Add a tag').press('Enter');
  await page.getByText('Saved locally', { exact: true }).waitFor({ timeout: 10000 });
  await page.getByLabel('Search notes').fill('offline field');
  await page.getByRole('heading', { name: 'My offline field note' }).waitFor();
  await page.screenshot({ path: 'test-results/bevernote-editor.png', fullPage: true });

  await page.getByTitle('Toggle shortcut').click();
  await expect(
    page.locator('.sidebar-section').first().getByText('My offline field note', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Settings & sync' }).click();
  await expect(page.getByRole('dialog', { name: 'Settings' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Optional pCloud sync' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Connect with browser' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Sync now', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Sync now', exact: true })).toBeInViewport();
  await expect(page.getByRole('button', { name: 'Sync now', exact: true })).toBeDisabled();
  await expect(page.getByText('Connect pCloud to enable sync.', { exact: true })).toBeVisible();
  await page.screenshot({ path: 'test-results/bevernote-settings.png', fullPage: true });
  await page.getByLabel('Appearance').selectOption('dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: 'Close settings' }).click();

  // Note-content-only find and keyboard navigation.
  await page.keyboard.press('Control+f');
  await page.getByLabel('Find in note content').fill('My offline field note');
  await expect(page.getByText('No matches', { exact: true })).toBeVisible();
  await page.getByLabel('Find in note content').fill('private');
  await expect(page.locator('.note-find-match')).toHaveCount(1);
  await page.getByLabel('Next match').click();
  await expect(page.locator('.note-find-match.active')).toHaveCount(1);
  await page.getByLabel('Find in note content').press('Escape');
  await expect(page.getByLabel('Find in note content')).toHaveCount(0);
  await page.getByLabel('Keyboard shortcuts').click();
  await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeVisible();
  await page.getByRole('dialog', { name: 'Keyboard shortcuts' }).press('Escape');

  // The complete palette floats outside the editor's scroll clipping context.
  await page.locator('.note-prose').click();
  await page.locator('.note-prose').press('Control+a');
  await page.getByLabel('Text color', { exact: true }).click();
  await expect(page.getByLabel('Custom text color')).toBeVisible();
  await checkFullyInViewport(page, page.getByRole('dialog', { name: 'Text color', exact: true }));
  await page.screenshot({ path: 'test-results/bevernote-color-palette.png', fullPage: true });
  await page.getByLabel('Text color: Blue').click();
  await expect(page.locator('.note-prose span[style*="color"]').first()).toHaveCSS(
    'color',
    'rgb(37, 99, 235)',
  );
  await page.getByLabel('Highlight color', { exact: true }).click();
  await expect(page.getByLabel('Custom highlight color')).toBeVisible();
  await checkFullyInViewport(page, page.getByRole('dialog', { name: 'Highlight color', exact: true }));
  await page.screenshot({ path: 'test-results/bevernote-highlight-palette.png', fullPage: true });
  await page.getByLabel('Highlight color: Mint').click();
  await expect(page.locator('.note-prose mark').first()).toHaveCSS('background-color', 'rgb(187, 247, 208)');

  // Stack folding is independent from stack navigation.
  await page.getByLabel('Collapse stack Learning').click();
  await expect(page.locator('.stack-children')).toHaveCount(0);
  await expect(page.getByLabel('Expand stack Learning')).toHaveAttribute('aria-expanded', 'false');
  await page.getByLabel('Expand stack Learning').click();
  await expect(page.locator('.stack-children')).toBeVisible();

  // Created/updated timestamps use local values, persist, and reject invalid ranges.
  await page.getByTitle('Note information').click();
  await page.getByLabel('Created date and time').fill('2024-01-02T03:04:05');
  await page.getByLabel('Updated date and time').fill('2024-02-03T04:05:06');
  await page.getByRole('button', { name: 'Apply timestamps' }).click();
  await expect(page.getByLabel('Created date and time')).toHaveValue('2024-01-02T03:04:05');
  await expect(page.getByLabel('Updated date and time')).toHaveValue('2024-02-03T04:05:06');
  await page.screenshot({ path: 'test-results/bevernote-timestamps.png', fullPage: true });
  await page.getByLabel('Close note information').click();
  await page.getByTitle('Note information').click();
  await expect(page.getByLabel('Created date and time')).toHaveValue('2024-01-02T03:04:05');
  await expect(page.getByLabel('Updated date and time')).toHaveValue('2024-02-03T04:05:06');
  await page.getByLabel('Updated date and time').fill('2023-12-31T23:59:59');
  await page.getByRole('button', { name: 'Apply timestamps' }).click();
  await expect(page.locator('.timestamp-editor').getByRole('alert')).toContainText(
    'Updated time must be at or after created time.',
  );
  await page.getByLabel('Close note information').click();

  // Multi-note additive tags and multi-select tag filtering.
  await page.getByLabel('Search notes').fill('');
  await page.getByLabel('Select Welcome to Bevernote', { exact: true }).check();
  await page.getByLabel('Select Bring your Evernote library', { exact: true }).check();
  await expect(page.locator('.bulk-toolbar')).toContainText('2 selected');
  await page.screenshot({ path: 'test-results/bevernote-bulk-actions.png', fullPage: true });
  await page.locator('.bulk-toolbar').getByRole('button', { name: 'Tags', exact: true }).click();
  const tagDialog = page.getByRole('dialog', { name: 'Add tags', exact: true });
  await expect(tagDialog.getByRole('group', { name: 'Existing tags' })).toBeVisible();
  await tagDialog.getByLabel('Tags', { exact: true }).fill('local');
  await expect(tagDialog.getByRole('button', { name: 'Add existing tag local-first' })).toBeVisible();
  await expect(tagDialog.getByRole('button', { name: 'Add existing tag ideas' })).toHaveCount(0);
  await tagDialog.getByRole('button', { name: 'Add existing tag local-first' }).click();
  await expect(tagDialog.getByRole('group', { name: 'Tags to add' })).toContainText('#local-first');
  await tagDialog.getByLabel('Tags', { exact: true }).fill('bulk-test, second-tag');
  await tagDialog.getByLabel('Tags', { exact: true }).press('Enter');
  await expect(tagDialog.getByRole('group', { name: 'Tags to add' })).toContainText('#bulk-test');
  await expect(tagDialog.getByRole('group', { name: 'Tags to add' })).toContainText('#second-tag');
  await page.screenshot({ path: 'test-results/bevernote-tag-picker.png', fullPage: true });
  await tagDialog.getByRole('button', { name: 'Add tags', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Tags added to 2 notes.');
  await page.getByRole('button', { name: /Filters/ }).click();
  await page.getByLabel('#bulk-test', { exact: true }).check();
  await page.getByLabel('#second-tag', { exact: true }).check();
  await expect(page.locator('.note-card')).toHaveCount(2);
  await expect(page.getByRole('button', { name: /Filters/ })).toContainText('2');
  await page.screenshot({ path: 'test-results/bevernote-tag-filters.png', fullPage: true });
  await page.getByRole('button', { name: 'Clear filters', exact: true }).click();
  await page.getByLabel('Close filters').click();
  await expect(page.locator('.note-card')).toHaveCount(4);

  // Move multiple notes, then copy one note with fresh identity.
  await page.getByLabel('Select Welcome to Bevernote', { exact: true }).check();
  await page.getByLabel('Select Bring your Evernote library', { exact: true }).check();
  await page.locator('.bulk-toolbar').getByRole('button', { name: 'Move', exact: true }).click();
  const moveDialog = page.getByRole('dialog', { name: 'Move notes', exact: true });
  await moveDialog.getByLabel('Destination notebook').selectOption({ label: 'Inbox' });
  await moveDialog.getByRole('button', { name: 'Move notes', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('2 notes moved.');
  for (const title of ['Welcome to Bevernote', 'Bring your Evernote library']) {
    await expect(
      page
        .locator('.note-card')
        .filter({ has: page.getByRole('heading', { name: title, exact: true }) })
        .locator('.note-card-meta'),
    ).toContainText('Inbox');
  }
  await page.getByLabel('Select Welcome to Bevernote', { exact: true }).check();
  await page.locator('.bulk-toolbar').getByRole('button', { name: 'Copy', exact: true }).click();
  const copyDialog = page.getByRole('dialog', { name: 'Copy notes', exact: true });
  await copyDialog.getByLabel('Destination notebook').selectOption({ label: 'Reading' });
  await copyDialog.getByRole('button', { name: 'Copy notes', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('1 note copied.');
  await page.getByLabel('Search notes').fill('Welcome to Bevernote (copy)');
  await expect(page.getByRole('heading', { name: 'Welcome to Bevernote (copy)', exact: true })).toBeVisible();
  await page.getByLabel('Search notes').fill('');

  // Local PDF preview, pagination, and attachment context actions.
  await page.getByLabel('Search notes').fill('Local attachment preview');
  await page.locator('.note-card').getByText('Local attachment preview', { exact: true }).click();
  await page.getByRole('button', { name: 'Preview PDF', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Preview Bevernote preview.pdf' })).toBeVisible();
  await expect(page.locator('.pdf-canvas-wrap canvas')).toBeVisible();
  await expect(page.locator('.pdf-controls')).toContainText('1 / 2', { timeout: 30000 });
  await expect(page.locator('.pdf-canvas-wrap canvas')).toHaveAttribute('data-rendered-page', '1');
  await page.getByLabel('Next PDF page').click();
  await expect(page.locator('.pdf-controls')).toContainText('2 / 2', { timeout: 30000 });
  await expect(page.locator('.pdf-canvas-wrap canvas')).toHaveAttribute('data-rendered-page', '2');
  await expect(page.getByLabel('Loading PDF')).toHaveCount(0);
  await page.screenshot({ path: 'test-results/bevernote-pdf-preview.png', fullPage: true });
  await page.getByLabel('Close PDF preview').click();
  await page.locator('.attachment-file').click({ button: 'right' });
  await expect(page.getByRole('menu')).toBeVisible();
  await expect(page.getByRole('menuitem', { name: 'Open with system app' })).toBeDisabled();
  await expect(page.getByRole('menuitem', { name: 'Save As…' })).toBeDisabled();
  await page.getByLabel('Close attachment actions').click({ position: { x: 1, y: 1 } });

  // Create overflow in all panes and ensure wheel/touchpad scrolling is local.
  await page.evaluate(() => {
    window.prompt = () => 'Overflow stack';
  });
  for (let index = 0; index < 12; index += 1) await page.getByTitle('New stack').click();
  await checkIndependentWheel(page, '.sidebar-scroll');
  await page.getByLabel('Search notes').fill('');
  for (let index = 0; index < 18; index += 1) {
    await page.keyboard.press('Control+n');
    await page.waitForTimeout(30);
  }
  await checkIndependentWheel(page, '.note-list');
  await page.locator('.note-prose').click();
  await page.locator('.note-prose').pressSequentially('Offline scrolling test.\n\n'.repeat(55));
  await checkIndependentWheel(page, '.document-column');
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
  expect((await page.locator('.app-shell').boundingBox()).y).toBe(0);

  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByLabel('Back to notes').click();
  await page.locator('.editor-panel').evaluate(async (element) => {
    await Promise.all(element.getAnimations().map((animation) => animation.finished));
  });
  await expect(page.locator('.notes-panel')).toBeInViewport();
  await page.screenshot({ path: 'test-results/bevernote-mobile.png', fullPage: true });
  await page.getByLabel('Open navigation').click();
  await page.locator('.sidebar.is-open').waitFor();
  await page.locator('.sidebar').evaluate(async (element) => {
    await Promise.all(element.getAnimations().map((animation) => animation.finished));
  });
  await expect(page.locator('.sidebar')).toBeInViewport();
  await page.getByLabel('Move Inbox to stack').selectOption('stack-learning');
  await expect(page.locator('.stack-children').getByText('Inbox', { exact: true })).toBeVisible();
  await page.getByLabel('Move Inbox to stack').selectOption('');
  await page.screenshot({ path: 'test-results/bevernote-mobile-navigation.png', fullPage: true });
  await page.getByRole('button', { name: 'Settings & sync' }).click();
  await expect(page.getByRole('dialog', { name: 'Settings' })).toBeVisible();
  await page.screenshot({ path: 'test-results/bevernote-mobile-settings.png', fullPage: true });
  const settingsBounds = await page.getByRole('dialog', { name: 'Settings' }).boundingBox();
  expect(settingsBounds).not.toBeNull();
  expect(settingsBounds.x).toBeGreaterThanOrEqual(0);
  expect(settingsBounds.x + settingsBounds.width).toBeLessThanOrEqual(390);
  if (errors.length) throw new Error(`Browser errors:\n${errors.join('\n')}`);
  console.log('Visual and interaction checks passed.');
} finally {
  await browser?.close();
  await server.close();
}

async function checkFullyInViewport(page, locator) {
  const bounds = await locator.boundingBox();
  expect(bounds).not.toBeNull();
  const viewport = page.viewportSize();
  expect(bounds.x).toBeGreaterThanOrEqual(0);
  expect(bounds.y).toBeGreaterThanOrEqual(0);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(viewport.width);
  expect(bounds.y + bounds.height).toBeLessThanOrEqual(viewport.height);
}

async function checkIndependentWheel(page, selector) {
  const locator = page.locator(selector);
  await locator.evaluate((element) => {
    element.scrollTop = 0;
  });
  expect(await locator.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true);
  await locator.hover();
  await page.mouse.wheel(0, 420);
  await page.waitForTimeout(250);
  expect(await locator.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
  expect((await page.locator('.app-shell').boundingBox()).y).toBe(0);
}
