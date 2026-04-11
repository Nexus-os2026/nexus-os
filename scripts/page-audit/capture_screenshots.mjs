#!/usr/bin/env node
// capture_screenshots.mjs
// Usage: node capture_screenshots.mjs <url> <slug> <outDir>
// Captures full-page screenshots at 1920x1080, 1280x800, 1024x768.
// Writes <slug>-{1920,1280,1024}.png to outDir.

import puppeteer from 'puppeteer';
import { statSync } from 'fs';
import { resolve } from 'path';

const [, , url, slug, outDir] = process.argv;

if (!url || !slug || !outDir) {
  console.error('Usage: node capture_screenshots.mjs <url> <slug> <outDir>');
  process.exit(1);
}

const VIEWPORTS = [
  { name: '1920', width: 1920, height: 1080 },
  { name: '1280', width: 1280, height: 800 },
  { name: '1024', width: 1024, height: 768 },
];

const NAV_TIMEOUT_MS = 30000;
const POST_LOAD_WAIT_MS = 800;

(async () => {
  let browser;
  try {
    browser = await puppeteer.launch({
      headless: 'new',
      args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage'],
    });
  } catch (e) {
    console.error('FATAL: failed to launch puppeteer chrome:', e.message);
    process.exit(2);
  }

  let failures = 0;
  try {
    for (const vp of VIEWPORTS) {
      const page = await browser.newPage();
      await page.setViewport({
        width: vp.width,
        height: vp.height,
        deviceScaleFactor: 1,
      });
      try {
        await page.goto(url, { waitUntil: 'networkidle2', timeout: NAV_TIMEOUT_MS });
      } catch (e) {
        console.error(`[${vp.name}] navigation: ${e.message} — capturing whatever rendered`);
      }
      await new Promise((r) => setTimeout(r, POST_LOAD_WAIT_MS));
      const outPath = resolve(outDir, `${slug}-${vp.name}.png`);
      try {
        await page.screenshot({ path: outPath, fullPage: true });
        const size = statSync(outPath).size;
        if (size === 0) {
          console.error(`[${vp.name}] screenshot wrote 0 bytes`);
          failures++;
        } else {
          console.log(`Saved ${slug}-${vp.name}.png (${(size / 1024).toFixed(0)}K)`);
        }
      } catch (e) {
        console.error(`[${vp.name}] screenshot failed: ${e.message}`);
        failures++;
      }
      await page.close();
    }
  } finally {
    await browser.close();
  }

  if (failures > 0) {
    console.error(`FAILED: ${failures} screenshot(s) did not save`);
    process.exit(3);
  }
  console.log('Done.');
})().catch((e) => {
  console.error('FATAL:', e.message);
  process.exit(2);
});
