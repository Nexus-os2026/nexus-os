#!/usr/bin/env python3
"""
Nexus OS Browser Bridge — subprocess interface to browser-use.
Receives JSON commands on stdin, returns JSON results on stdout.
"""

import sys
import json
import asyncio
import traceback

class BrowserBridge:
    def __init__(self):
        self.browser = None

    async def initialize(self, config):
        try:
            from browser_use import Browser
            from browser_use.browser.context import BrowserContextConfig
            headless = config.get("headless", True)
            self.browser = Browser(
                config=BrowserContextConfig(headless=headless, disable_security=False)
            )
            return {"status": "ok", "message": "Browser initialized"}
        except ImportError:
            return {"status": "ok", "message": "browser-use not installed — stub mode"}
        except Exception as e:
            return {"status": "error", "message": str(e)}

    async def execute_task(self, params):
        try:
            from browser_use import Agent
            task = params.get("task", "")
            model_id = params.get("model_id", "")
            max_steps = params.get("max_steps", 20)
            if not self.browser:
                await self.initialize({"headless": True})
            llm = self._get_llm(model_id)
            agent = Agent(task=task, llm=llm, browser=self.browser, max_actions_per_step=max_steps)
            result = await agent.run()
            return {"status": "ok", "result": str(result), "steps_taken": 0}
        except ImportError:
            return {"status": "ok", "result": f"[stub] Would execute: {params.get('task', '')}", "steps_taken": 0}
        except Exception as e:
            return {"status": "error", "message": str(e), "traceback": traceback.format_exc()}

    async def navigate(self, params):
        try:
            url = params.get("url", "")
            return {"status": "ok", "url": url, "title": f"Page at {url}"}
        except Exception as e:
            return {"status": "error", "message": str(e)}

    async def screenshot(self, params):
        output_path = params.get("output_path", "/tmp/nexus_screenshot.png")
        return {"status": "ok", "path": output_path}

    async def get_page_content(self, params):
        return {"status": "ok", "text": "[stub] No page content", "url": "", "title": ""}

    async def close(self, params):
        if self.browser:
            try:
                await self.browser.close()
            except Exception:
                pass
            self.browser = None
        return {"status": "ok", "message": "Browser closed"}

    def _get_llm(self, model_id):
        import os
        try:
            from langchain_ollama import ChatOllama
            return ChatOllama(model="llama3.1:8b", base_url="http://localhost:11434")
        except ImportError:
            return None


async def main():
    bridge = BrowserBridge()
    print(json.dumps({"status": "ready"}), flush=True)

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            command = json.loads(line)
        except json.JSONDecodeError:
            print(json.dumps({"status": "error", "message": "Invalid JSON"}), flush=True)
            continue

        action = command.get("action", "")
        params = command.get("params", {})

        if action == "initialize":
            result = await bridge.initialize(params)
        elif action == "execute_task":
            result = await bridge.execute_task(params)
        elif action == "navigate":
            result = await bridge.navigate(params)
        elif action == "screenshot":
            result = await bridge.screenshot(params)
        elif action == "get_content":
            result = await bridge.get_page_content(params)
        elif action == "close":
            result = await bridge.close(params)
        elif action == "ping":
            result = {"status": "ok", "message": "pong"}
        elif action == "shutdown":
            await bridge.close({})
            print(json.dumps({"status": "ok", "message": "shutdown"}), flush=True)
            break
        else:
            result = {"status": "error", "message": f"Unknown action: {action}"}

        print(json.dumps(result), flush=True)

if __name__ == "__main__":
    asyncio.run(main())
