#!/usr/bin/env python3
"""
飞书项目管理 Headless 登录脚本
流程：加载Cookie → 访问目标页 → 检测登录状态 → QR码切换 → 等待扫码 → 保存Cookie
"""
import asyncio
import json
import os
import sys
import time
from datetime import datetime
from pathlib import Path
from playwright.async_api import async_playwright

SKILL_DIR = Path(__file__).resolve().parent.parent
COOKIE_FILE = SKILL_DIR / "cookies.json"
TARGET_URL = "https://project.feishu.cn/chapanda_project"
SCAN_TIMEOUT = 180  # 秒


def ts():
    return datetime.now().isoformat()


def log_event(event, **kwargs):
    data = {"event": event, "ts": ts(), **kwargs}
    print(json.dumps(data, ensure_ascii=False), flush=True)


def save_cookies(cookies):
    with open(COOKIE_FILE, "w") as f:
        json.dump(cookies, f, indent=2, ensure_ascii=False)
    log_event("cookies_saved", path=str(COOKIE_FILE), count=len(cookies))


async def main():
    # 1. 加载已存储的Cookie
    cookies = []
    if COOKIE_FILE.exists():
        with open(COOKIE_FILE) as f:
            cookies = json.load(f)
        log_event("cookies_loaded", count=len(cookies))
    else:
        log_event("no_cookies_found")

    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(
            viewport={"width": 1280, "height": 900},
            user_agent=(
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
            ),
        )
        if cookies:
            await context.add_cookies(cookies)

        page = await context.new_page()

        # 2. 访问目标页
        log_event("navigating", url=TARGET_URL)
        await page.goto(TARGET_URL, wait_until="domcontentloaded", timeout=60000)
        await page.wait_for_timeout(5000)

        current_url = page.url
        log_event("page_loaded", url=current_url)

        # 3. 判断是否已登录
        if (
            "project.feishu.cn" in current_url
            and "accounts.feishu.cn" not in current_url
            and "login" not in current_url
        ):
            log_event("already_logged_in", url=current_url)
            new_cookies = await context.cookies()
            save_cookies(new_cookies)
            await browser.close()
            return

        # 4. 在登录页
        init_path = f"/home/agent/feishu_project_init_{datetime.now():%Y%m%d_%H%M%S}.png"
        await page.screenshot(path=init_path, timeout=60000)
        log_event("login_page_detected", url=current_url, screenshot=init_path)

        # 5. 点击二维码切换按钮（左半边右上角 .switch-login-mode-box）
        qr_switch = await page.query_selector(".switch-login-mode-box")
        if not qr_switch:
            qr_switch = await page.query_selector(".login-qr-switch-box")

        if qr_switch:
            log_event("clicking_qr_switch")
            await qr_switch.click()
            await page.wait_for_timeout(3000)
        else:
            log_event("qr_switch_not_found_trying_js")
            await page.evaluate("""
                () => {
                    const el = document.querySelector('.switch-login-mode-box')
                            || document.querySelector('.login-qr-switch-box')
                            || document.querySelector('.switch-login-mode-container');
                    if (el) el.click();
                }
            """)
            await page.wait_for_timeout(3000)

        # 6. 等待二维码出现并截图
        qr_path = f"/home/agent/feishu_project_qr_{datetime.now():%Y%m%d_%H%M%S}.png"

        qr_ready = False
        for _ in range(10):
            has_qr = await page.evaluate("""
                () => {
                    const canvas = document.querySelector('canvas');
                    const qrImg = document.querySelector(
                        'img[class*="qr"], img[src*="qr"], img[class*="QR"]'
                    );
                    const qrBox = document.querySelector(
                        '[class*="qr-code"], [class*="QRCode"], [class*="qrcode"]'
                    );
                    return !!(canvas || qrImg || qrBox);
                }
            """)
            if has_qr:
                qr_ready = True
                break
            await page.wait_for_timeout(1000)

        await page.screenshot(path=qr_path, timeout=60000)

        if qr_ready:
            log_event("qr_ready", path=qr_path, message="请用飞书扫描此二维码")
        else:
            log_event(
                "qr_screenshot_fallback",
                path=qr_path,
                message="请查看截图并用飞书扫描二维码",
            )

        # 7. 等待用户扫码 — 监测URL变化
        log_event("waiting_for_scan", timeout=SCAN_TIMEOUT)
        start_time = time.time()
        last_heartbeat = start_time

        while time.time() - start_time < SCAN_TIMEOUT:
            current_url = page.url

            if (
                "project.feishu.cn/chapanda_project" in current_url
                and "accounts.feishu.cn" not in current_url
            ):
                elapsed = round(time.time() - start_time)
                log_event("scan_success", url=current_url, elapsed=f"{elapsed}s")

                await page.wait_for_timeout(5000)

                new_cookies = await context.cookies()
                save_cookies(new_cookies)

                success_path = (
                    f"/home/agent/feishu_project_success_{datetime.now():%Y%m%d_%H%M%S}.png"
                )
                await page.screenshot(path=success_path, timeout=60000)
                log_event("login_success", screenshot=success_path)
                await browser.close()
                return

            if time.time() - last_heartbeat >= 30:
                elapsed = round(time.time() - start_time)
                log_event("heartbeat", elapsed=f"{elapsed}s", url=current_url)
                last_heartbeat = time.time()

            await page.wait_for_timeout(2000)

        # 超时
        timeout_path = (
            f"/home/agent/feishu_project_timeout_{datetime.now():%Y%m%d_%H%M%S}.png"
        )
        await page.screenshot(path=timeout_path, timeout=60000)
        log_event(
            "timeout",
            screenshot=timeout_path,
            message=f"二维码扫描超时（{SCAN_TIMEOUT}s）",
        )
        await browser.close()


if __name__ == "__main__":
    asyncio.run(main())
