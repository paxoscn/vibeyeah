#!/usr/bin/env python3
"""
飞书项目 — 创建需求自动化脚本 v13
全部字段自动填写。人员选择使用搜索框模式：输入人名 → 等下拉列表 → 选第一个

用法:
  source ~/playwright_env/bin/activate
  python3 ~/.hermes/skills/software-development/feishu-project-login/scripts/create_story.py

配置: 修改下方常量即可
  STORY_TITLE — 需求标题
  PERSON_NAME — 产品经理/开发/测试负责人姓名
  EXPECTED_DATE — 期望上线日期
  BUSINESS_LINE_ID — 业务线 ID
  PROJECT_NAME — 所属项目搜索关键词
"""
import asyncio
import json
from datetime import datetime
from pathlib import Path
from playwright.async_api import async_playwright

COOKIE_FILE = Path(__file__).resolve().parent.parent / "cookies.json"
CREATE_URL = "https://project.feishu.cn/chapanda_project/story/create"

# ── Configurable ──
PERSON_NAME = "莫日根"
DEV_LEAD = "刘亮"
TEST_LEAD = "刘亮"
STORY_TITLE = "测试需求0723"
EXPECTED_DATE = "2026-08-31"
BUSINESS_LINE_SEARCH = "商品中心"
PROJECT_NAME = "数字化中心-产品迭代项目"

# ── JS Templates ──
# IMPORTANT: Must use function() + arguments[0], NOT arrow functions.
# Arrow functions lack `arguments` in strict mode, causing SyntaxError.

JS_GET_CLICKABLE = """function() {
    const labelText = arguments[0];
    const items = document.querySelectorAll('[class*="CreateFormItem"]');
    for (const item of items) {
        const label = item.querySelector('label');
        if (label && label.innerText.trim() === labelText) {
            const main = item.querySelector('[class*="form-field-main"]');
            if (!main) return null;
            const cl = main.querySelector(
                '.meego-text, [class*="clickable"]' +
                ', [class*="user-display"], [class*="placeholder"]' +
                ', [class*="select"]'
            );
            if (cl) {
                const r = cl.getBoundingClientRect();
                if (r.width > 0) return {
                    cx: Math.round(r.x + 20),
                    cy: Math.round(r.y + r.height / 2),
                };
            }
            const mr = main.getBoundingClientRect();
            return {
                cx: Math.round(mr.x + 100),
                cy: Math.round(mr.y + mr.height / 2),
            };
        }
    }
    return null;
}"""

JS_FIND_LABEL_FIELD = """function() {
    const labelText = arguments[0];
    const items = document.querySelectorAll('[class*="CreateFormItem"]');
    for (const item of items) {
        const label = item.querySelector('label');
        if (label && label.innerText.trim() === labelText) {
            const main = item.querySelector('[class*="form-field-main"]');
            if (main) {
                const mr = main.getBoundingClientRect();
                return {
                    cx: Math.round(mr.x + 100),
                    cy: Math.round(mr.y + mr.height / 2),
                };
            }
        }
    }
    return null;
}"""

JS_CLICK_OPTION = """function() {
    const text = arguments[0];
    const allEls = document.querySelectorAll('*');
    const candidates = [];
    for (const el of allEls) {
        const t = (el.innerText || '').trim();
        const rect = el.getBoundingClientRect();
        if (t === text && rect.width > 30 && rect.height > 10
            && rect.height < 60 && rect.y > 50) {
            candidates.push({
                cls: (el.className || '').toString().substring(0, 80),
                cx: Math.round(rect.x + rect.width / 2),
                cy: Math.round(rect.y + rect.height / 2),
            });
        }
    }
    if (candidates.length > 0) {
        return candidates.find(c =>
            c.cls.includes('option') || c.cls.includes('item')
        ) || candidates[0];
    }
    return null;
}"""

JS_FIND_SEARCH_INPUT = """function() {
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
                });
        }
    }
    return results.length > 0 ? results[results.length - 1] : null;
}"""

JS_FIND_SEARCH_RESULT = """function() {
    const matchText = arguments[0];
    const allEls = document.querySelectorAll('*');
    for (const el of allEls) {
        const t = (el.innerText || '').trim();
        const rect = el.getBoundingClientRect();
        if (rect.width > 50 && rect.height >= 20
            && rect.height <= 60 && rect.y > 50
            && t.includes(matchText)
            && t.length < 100) {
            return {
                cx: Math.round(rect.x + rect.width / 2),
                cy: Math.round(rect.y + rect.height / 2),
            };
        }
    }
    return null;
}"""

JS_FIND_TEXTAREA = """function() {
    const tas = document.querySelectorAll('textarea');
    for (const ta of tas) {
        const r = ta.getBoundingClientRect();
        if (r.width > 100 && r.y > 50)
            return { cx: Math.round(r.x + 10), cy: Math.round(r.y + 10) };
    }
    return null;
}"""

JS_FIND_DATE_INPUT = """function() {
    const inputs = document.querySelectorAll('input');
    for (const input of inputs) {
        const rect = input.getBoundingClientRect();
        const ph = input.placeholder || '';
        if (rect.width > 100 && rect.height > 15
            && rect.height < 50 && rect.y > 50
            && (ph.includes('yyyy') || ph.includes('date'))) {
            return {
                cx: Math.round(rect.x + 10),
                cy: Math.round(rect.y + rect.height / 2),
            };
        }
    }
    return null;
}"""

JS_FIND_ROLE_LABEL = """function() {
    const roleName = arguments[0];
    const spans = document.querySelectorAll('span');
    for (const span of spans) {
        if (span.innerText.trim() === roleName) {
            const rect = span.getBoundingClientRect();
            if (rect.y > 50 && rect.height > 0 && rect.height < 40) {
                span.scrollIntoView({ block: 'center' });
                const r2 = span.getBoundingClientRect();
                return {
                    labelX: r2.x,
                    labelCy: Math.round(r2.y + r2.height / 2),
                };
            }
        }
    }
    return null;
}"""

JS_REQUERY_ROLE_LABEL = """function() {
    const roleName = arguments[0];
    const spans = document.querySelectorAll('span');
    for (const span of spans) {
        if (span.innerText.trim() === roleName) {
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
}"""

JS_INJECT_BUSINESS = """function() {
    const bizId = arguments[0];
    const items = document.querySelectorAll('[class*="CreateFormItem"]');
    for (const item of items) {
        const label = item.querySelector('label');
        if (label && label.innerText.trim() === '需求标题') {
            const fk = Object.keys(item).find(k => k.startsWith('__reactFiber$'));
            if (!fk) return 'no_fiber_key';
            let fiber = item[fk];
            for (let i = 0; i < 30; i++) {
                if (!fiber) break;
                let state = fiber.memoizedState;
                while (state) {
                    const obj = state.memoizedState;
                    if (obj && obj.current && typeof obj.current === 'object'
                        && 'business' in obj.current) {
                        obj.current.business = bizId;
                        return 'injected:' + obj.current.business;
                    }
                    state = state.next;
                }
                fiber = fiber.return;
            }
            return 'store_not_found';
        }
    }
    return 'no_title_item';
}"""


async def main():
    cookie_file = COOKIE_FILE
    if not cookie_file.exists():
        cookie_file = Path.home() / ".hermes/skills/software-development/feishu-project-login/cookies.json"
    if not cookie_file.exists():
        print("ERROR: Cookie file not found!")
        return

    print(f"Cookies: {cookie_file}")
    with open(cookie_file) as f:
        cookies = json.load(f)

    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(
            viewport={"width": 1440, "height": 900},
            user_agent="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                       "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        await context.add_cookies(cookies)
        page = await context.new_page()

        print("Loading create page...")
        await page.goto(CREATE_URL, wait_until="domcontentloaded", timeout=60000)
        await page.wait_for_timeout(15000)
        print("Page loaded")

        # Close notification banners
        for _ in range(3):
            try:
                btn = await page.query_selector('[class*="notification-notice-icon-close"]')
                if btn:
                    await btn.click()
                    await page.wait_for_timeout(500)
            except Exception:
                pass

        # ── Helpers ──
        async def close_popups():
            await page.keyboard.press("Escape")
            await page.wait_for_timeout(300)
            await page.keyboard.press("Escape")
            await page.wait_for_timeout(300)
            await page.mouse.click(350, 300)
            await page.wait_for_timeout(1500)

        async def get_clickable(label_text):
            return await page.evaluate(JS_GET_CLICKABLE, label_text)

        async def click_option(text, wait_ms=2500):
            await page.wait_for_timeout(wait_ms)
            c = await page.evaluate(JS_CLICK_OPTION, text)
            if c:
                await page.mouse.click(c['cx'], c['cy'])
                return True
            return False

        async def search_and_select(search_text, match_text, wait_search=5000):
            inp = await page.evaluate(JS_FIND_SEARCH_INPUT)
            if not inp:
                print(f"    [warn] No search input found")
                return False
            await page.mouse.click(inp['cx'], inp['cy'])
            await page.wait_for_timeout(500)
            await page.keyboard.type(search_text, delay=150)
            await page.wait_for_timeout(wait_search)
            
            # Find the semi-select-option element specifically
            target = await page.evaluate("""function() {
                const matchText = arguments[0];
                const options = document.querySelectorAll('.semi-select-option');
                for (const opt of options) {
                    const t = (opt.innerText || '').trim();
                    const rect = opt.getBoundingClientRect();
                    if (t.includes(matchText) && t.length < 100 && rect.y > 50
                        && rect.width > 50 && rect.height >= 20 && rect.height <= 60) {
                        return {
                            cx: Math.round(rect.x + rect.width / 2),
                            cy: Math.round(rect.y + rect.height / 2),
                        };
                    }
                }
                return null;
            }""", match_text)
            
            if target:
                await page.mouse.click(target['cx'], target['cy'])
                await page.wait_for_timeout(3000)
                return True
            print(f"    [warn] No result found for '{match_text}'")
            return False

        async def fill_role(role_name, person_name):
            print(f"  Role: {role_name}")
            # 1. Find role label and scroll into view
            coords = await page.evaluate(JS_FIND_ROLE_LABEL, role_name)
            if not coords:
                print(f"    [warn] Label '{role_name}' not found")
                return False
            await page.wait_for_timeout(1500)

            # 2. Re-query position after scroll
            coords2 = await page.evaluate(JS_REQUERY_ROLE_LABEL, role_name)
            if coords2:
                coords = coords2

            # 3. Click the person picker placeholder
            picker_cx = coords['labelX'] + 250
            picker_cy = coords['labelCy']
            await page.mouse.click(picker_cx, picker_cy)
            await page.wait_for_timeout(6000)  # 等6秒让picker完全展开

            # 4. Search and select with longer waits
            ok = await search_and_select(person_name, person_name, wait_search=8000)
            if ok:
                print(f"    [ok] {person_name} selected")
                # 选完后等5秒让组件内部state同步
                await page.wait_for_timeout(5000)
            return ok

        # ════════════════════════════════════
        # MAIN FLOW — 10 fields, all automated
        # ════════════════════════════════════
        await page.evaluate("window.scrollTo(0, 0)")
        await page.wait_for_timeout(2000)

        # 1. Title
        print("1. Title")
        c = await get_clickable("需求标题")
        if c:
            await page.mouse.click(c['cx'], c['cy'])
            await page.wait_for_timeout(2500)
            ta = await page.evaluate(JS_FIND_TEXTAREA)
            if ta:
                await page.mouse.click(ta['cx'], ta['cy'])
                await page.wait_for_timeout(300)
                await page.keyboard.type(STORY_TITLE, delay=30)
                await page.wait_for_timeout(500)
                await page.mouse.click(800, 300)
                await page.wait_for_timeout(2000)
                print("  [ok]")
        await close_popups()

        # 2. Category
        print("2. Category (need classification)")
        c = await get_clickable("需求分类")
        if not c:
            c = await page.evaluate(JS_FIND_LABEL_FIELD, "需求分类")
        if c:
            await page.mouse.click(c['cx'], c['cy'])
            if await click_option("产品需求"):
                print("  [ok]")
        await close_popups()

        # 3. Source
        print("3. Source")
        c = await get_clickable("需求来源")
        if not c:
            c = await page.evaluate(JS_FIND_LABEL_FIELD, "需求来源")
        if c:
            await page.mouse.click(c['cx'], c['cy'])
            if await click_option("数字化中心"):
                print("  [ok]")
        await close_popups()

        # 4. Expected date
        print("4. Expected date")
        c = await get_clickable("期望上线日期")
        if c:
            await page.mouse.click(c['cx'], c['cy'])
            await page.wait_for_timeout(1000)
            di = await page.evaluate(JS_FIND_DATE_INPUT)
            if di:
                await page.mouse.click(di['cx'], di['cy'])
                await page.wait_for_timeout(500)
                await page.keyboard.press("Control+a")
                await page.keyboard.type(EXPECTED_DATE, delay=80)
                await page.wait_for_timeout(500)
                await page.keyboard.press("Enter")
                await page.wait_for_timeout(2000)
                print("  [ok]")
        await close_popups()

        # 5. Product manager (top-level)
        print("5. Product manager")
        c = await get_clickable("产品经理")
        if c:
            await page.mouse.click(c['cx'], c['cy'])
            await page.wait_for_timeout(1000)
            if await search_and_select(PERSON_NAME, PERSON_NAME):
                print("  [ok]")
        await close_popups()

        # 6. Project (needs scrollIntoView first)
        print("6. Project")
        c = await page.evaluate("""function() {
            const items = document.querySelectorAll('[class*="CreateFormItem"]');
            for (const item of items) {
                const label = item.querySelector('label');
                if (label && label.innerText.trim() === '所属项目') {
                    label.scrollIntoView({ block: 'center' });
                    const main = item.querySelector('[class*="form-field-main"]');
                    if (main) {
                        const mr = main.getBoundingClientRect();
                        return {
                            cx: Math.round(mr.x + 100),
                            cy: Math.round(mr.y + mr.height / 2),
                        };
                    }
                }
            }
            return null;
        }""")
        if c:
            await page.wait_for_timeout(1000)
            await page.mouse.click(c['cx'], c['cy'])
            await page.wait_for_timeout(5000)
            if await search_and_select(PROJECT_NAME, "产品迭代项目"):
                print("  [ok]")
            else:
                print("  [warn] Project selection failed")
        else:
            print("  [warn] Project field not found")
        await close_popups()

        # 7. Business line (search + select)
        print("7. Business line (search)")
        # Scroll into view
        await page.evaluate("""function() {
            const items = document.querySelectorAll('[class*="CreateFormItem"]');
            for (const item of items) {
                const label = item.querySelector('label');
                if (label && label.innerText.trim() === '业务线') {
                    label.scrollIntoView({ block: 'center' });
                }
            }
        }""")
        await page.wait_for_timeout(1000)

        # Click main area to activate, then find input
        biz_main = await page.evaluate("""function() {
            const items = document.querySelectorAll('[class*="CreateFormItem"]');
            for (const item of items) {
                const label = item.querySelector('label');
                if (label && label.innerText.trim() === '业务线') {
                    const main = item.querySelector('[class*="form-field-main"]');
                    if (main) {
                        const mr = main.getBoundingClientRect();
                        return { cx: Math.round(mr.x + 100), cy: Math.round(mr.y + mr.height / 2) };
                    }
                }
            }
            return null;
        }""")
        if biz_main:
            await page.mouse.click(biz_main['cx'], biz_main['cy'])
            await page.wait_for_timeout(2000)

            # Find input that appeared
            biz_input = await page.evaluate("""function() {
                const items = document.querySelectorAll('[class*="CreateFormItem"]');
                for (const item of items) {
                    const label = item.querySelector('label');
                    if (label && label.innerText.trim() === '业务线') {
                        const input = item.querySelector('input');
                        if (input) {
                            const r = input.getBoundingClientRect();
                            if (r.width > 30) {
                                return { cx: Math.round(r.x + 10), cy: Math.round(r.y + r.height / 2) };
                            }
                        }
                    }
                }
                return null;
            }""")

            if biz_input:
                await page.mouse.click(biz_input['cx'], biz_input['cy'])
                await page.wait_for_timeout(500)
                await page.keyboard.type(BUSINESS_LINE_SEARCH, delay=150)
                await page.wait_for_timeout(5000)

                # Find the text span for the leaf node (level-3, narrow, not unavailable)
                biz_option = await page.evaluate("""function() {
                    const searchTerm = arguments[0];
                    const dropdown = document.querySelector('[class*="meego-tree-select__dropdown"]');
                    if (!dropdown) return null;
                    // Find all level-3 option labels
                    const labels = dropdown.querySelectorAll('span.meego-tree-select__option-label');
                    for (const label of labels) {
                        const li = label.closest('li');
                        if (!li) continue;
                        const cls = (li.className || '').toString();
                        if (cls.includes('unavailable')) continue;
                        if (!cls.includes('level-3')) continue;
                        const text = (label.innerText || '').trim();
                        if (text === searchTerm) {
                            const rect = label.getBoundingClientRect();
                            return {
                                text: text,
                                cx: Math.round(rect.x + rect.width / 2),
                                cy: Math.round(rect.y + rect.height / 2),
                            };
                        }
                    }
                    return null;
                }""", BUSINESS_LINE_SEARCH)

                if biz_option:
                    # Use REAL mouse click on the text span
                    await page.mouse.click(biz_option['cx'], biz_option['cy'])
                    await page.wait_for_timeout(3000)
                    print(f"  [ok] Clicked {biz_option['text']} text span at ({biz_option['cx']}, {biz_option['cy']})")
                else:
                    print(f"  [warn] {BUSINESS_LINE_SEARCH} text span not found")
            else:
                print("  [warn] Search input not found")
        else:
            print("  [warn] Business line field not found")

        # Close dropdown WITHOUT Escape (Escape might cancel selection)
        await page.mouse.click(350, 300)
        await page.wait_for_timeout(1500)

        # Store injection NOT needed — UI click already sets the correct leaf node value
        # injected = await page.evaluate(JS_INJECT_BUSINESS, BUSINESS_LINE_ID)
        # print(f"  Store inject: {injected}")

        # 8. Roles: Product Manager
        print("8. Roles - Product Manager")
        await fill_role("产品经理", PERSON_NAME)
        await close_popups()

        # 9. Roles: Dev Lead
        print("9. Roles - Dev Lead")
        await fill_role("开发负责人", DEV_LEAD)
        await close_popups()

        # 10. Roles: Test Lead
        print("10. Roles - Test Lead")
        await fill_role("测试负责人", TEST_LEAD)
        await close_popups()

        # Screenshot before submit
        await page.evaluate("window.scrollTo(0, 0)")
        await page.wait_for_timeout(2000)
        ts = datetime.now().strftime("%Y%m%d_%H%M%S")
        path = f"/home/agent/feishu_create_{ts}.png"
        await page.screenshot(path=path, timeout=60000)
        print(f"\nScreenshot before submit: {path}")
        print("All 10 fields automated. Submitting...")

        # Submit — click the Create button
        submit_clicked = False
        # Try finding button with text "Create" or "创建"
        submit_coords = await page.evaluate("""function() {
            const btns = document.querySelectorAll('button');
            for (const btn of btns) {
                const t = (btn.innerText || '').trim();
                if (t === 'Create' || t === '创建') {
                    const r = btn.getBoundingClientRect();
                    if (r.width > 0 && r.height > 0) {
                        return { cx: Math.round(r.x + r.width/2), cy: Math.round(r.y + r.height/2) };
                    }
                }
            }
            return null;
        }""")
        if submit_coords:
            await page.mouse.click(submit_coords['cx'], submit_coords['cy'])
            submit_clicked = True
            print("  Clicked Create button")
            await page.wait_for_timeout(10000)
        else:
            print("  [warn] Create button not found")

        # Screenshot after submit
        ts2 = datetime.now().strftime("%Y%m%d_%H%M%S")
        path2 = f"/home/agent/feishu_after_submit_{ts2}.png"
        await page.screenshot(path=path2, timeout=60000)
        current_url = page.url
        print(f"Screenshot after submit: {path2}")
        print(f"URL after submit: {current_url}")
        if submit_clicked:
            print("Done! Form submitted.")
        else:
            print("Done! But submit button was not found.")

        await browser.close()


asyncio.run(main())
