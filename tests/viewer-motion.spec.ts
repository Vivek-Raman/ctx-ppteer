import { test, expect } from 'playwright/test';

test('runs the unchanged viewer motion fixture', async ({ page }) => {
  const runtimeErrors: string[] = [];
  page.on('pageerror', (error) => runtimeErrors.push(error.stack ?? error.message));
  await page.goto('/tests/viewer-motion.html');
  await expect.poll(() => page.title(), { timeout: 60_000 }).toMatch(/^PASS:/);
  const result = await page.evaluate(
    () => (window as Window & { motionResults?: { error?: string } }).motionResults,
  );
  expect(runtimeErrors, runtimeErrors.join('\n')).toEqual([]);
  expect(result?.error ?? null).toBeNull();
});
