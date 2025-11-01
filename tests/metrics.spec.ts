import { test, expect } from '@playwright/test';

test('Metrics Dashboard should load and display data', async ({ page }) => {
  await page.goto('http://localhost:5173');

  // Wait for the main dashboard to be visible
  await expect(page.locator('h1')).toHaveText('HAI-Net Portal');

  // Click the button/link to navigate to the metrics page
  await page.click('a[href="/metrics"]');

  // Wait for the metrics dashboard to load
  await expect(page.locator('h2')).toHaveText('System Metrics Dashboard');

  // Wait for some metric data to be present.
  // This assumes a 'CPU Usage' text appears on the page.
  await expect(page.locator('text=/CPU Usage/')).toBeVisible({ timeout: 15000 });

  // Take a screenshot
  await page.screenshot({ path: 'frontend_verification.png' });
});
