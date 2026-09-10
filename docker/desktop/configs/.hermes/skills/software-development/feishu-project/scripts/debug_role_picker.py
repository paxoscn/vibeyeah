#!/usr/bin/env python3
"""Debug: 查看开发负责人下拉框的 DOM 结构"""
import asyncio
import json
from pathlib import Path
from playwright.async_api import async_playwright

COOKIE_FILE = Path(__file__).resolve().parent.parent / "cookies.json"
CREATE_URL = "https://project.feishu.cn/chapanda_project/story/create"

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(
            viewport={"width": 1440, "height": 900},
            user_agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        )
        
        # Load cookies
        cookies_path = Path(COOKIE_FILE)
        if cookies_path.exists():
            with open(cookies_path) as f:
                cookies = json.load(f)
            await context.add_cookies(cookies)
        
        page = await context.new_page()
        await page.goto(CREATE_URL, wait_until="domcontentloaded")
        await page.wait_for_timeout(5000)
        
        # Wait for page to fully load
        print("Waiting for page to load...")
        await page.wait_for_timeout(10000)
        
        # Debug: check if we're logged in
        current_url = page.url
        print(f"Current URL: {current_url}")
        
        if "login" in current_url:
            print("ERROR: Not logged in, cookies may be invalid")
            await browser.close()
            return
        
        # Find 开发负责人 role label
        coords = await page.evaluate("""function() {
            const spans = document.querySelectorAll('span');
            for (const span of spans) {
                if (span.innerText.trim() === '开发负责人') {
                    span.scrollIntoView({block: 'center'});
                    const rect = span.getBoundingClientRect();
                    if (rect.y > 50 && rect.height > 0 && rect.height < 40) {
                        return {
                            labelX: rect.x,
                            labelCy: Math.round(rect.y + rect.height / 2),
                        };
                    }
                }
            }
            return null;
        }""")
        
        if not coords:
            print("ERROR: 开发负责人 label not found")
            return
        
        print(f"Found label at: {coords}")
        
        # Click the picker area (labelX + 250)
        picker_cx = coords['labelX'] + 250
        picker_cy = coords['labelCy']
        print(f"Clicking picker at: ({picker_cx}, {picker_cy})")
        await page.mouse.click(picker_cx, picker_cy)
        await page.wait_for_timeout(6000)
        
        # Find search input
        inp = await page.evaluate("""function() {
            const inputs = document.querySelectorAll('input');
            const results = [];
            for (const input of inputs) {
                const rect = input.getBoundingClientRect();
                if (rect.width > 50 && rect.height > 10
                    && rect.height < 50 && rect.y > 50 && rect.y < 850) {
                    const ph = input.placeholder || '';
                    if (ph !== 'Enter target value')
                        results.push({
                            cx: Math.round(rect.x + 10),
                            cy: Math.round(rect.y + rect.height / 2),
                            placeholder: ph,
                        });
                }
            }
            return results.length > 0 ? results[results.length - 1] : null;
        }""")
        
        if not inp:
            print("ERROR: Search input not found")
            return
        
        print(f"Found search input: {inp}")
        
        # Type search text
        await page.mouse.click(inp['cx'], inp['cy'])
        await page.wait_for_timeout(500)
        await page.keyboard.type("莫日根", delay=150)
        await page.wait_for_timeout(8000)
        
        # Analyze all elements containing "莫日根"
        results = await page.evaluate("""function() {
            const matchText = '莫日根';
            const allEls = document.querySelectorAll('*');
            const matches = [];
            for (const el of allEls) {
                const t = (el.innerText || '').trim();
                const rect = el.getBoundingClientRect();
                if (t.includes(matchText) && t.length < 100 && rect.y > 50) {
                    matches.push({
                        tag: el.tagName,
                        classes: el.className.toString().substring(0, 150),
                        text: t.substring(0, 80),
                        x: Math.round(rect.x),
                        y: Math.round(rect.y),
                        w: Math.round(rect.width),
                        h: Math.round(rect.height),
                        hasClickHandler: !!el.onclick,
                    });
                }
            }
            return matches;
        }""")
        
        print(f"\n=== Found {len(results)} elements containing '莫日根' ===")
        for i, el in enumerate(results):
            print(f"\n[{i}] {el['tag']} | {el['w']}x{el['h']} at ({el['x']},{el['y']})")
            print(f"    Classes: {el['classes']}")
            print(f"    Text: {el['text']}")
            print(f"    Has onclick: {el['hasClickHandler']}")
        
        await browser.close()

if __name__ == "__main__":
    asyncio.run(main())
