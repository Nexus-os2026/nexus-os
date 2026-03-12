"""WebSocket server bridging the VoiceEngine with the Tauri frontend.

Start with:
    python -m nexus_voice.voice_server

The server listens on 127.0.0.1:9876 by default and accepts JSON messages:
    {"action": "start_listening"}
    {"action": "stop_listening"}
    {"action": "transcribe", "audio_base64": "<base64-encoded PCM>"}
    {"action": "status"}
"""

from __future__ import annotations

import asyncio
import base64
import json
import logging
import sys

try:
    import websockets
    import websockets.asyncio.server as ws_server
except ImportError:
    websockets = None  # type: ignore[assignment]
    ws_server = None  # type: ignore[assignment]

from .voice_engine import VoiceEngine

logger = logging.getLogger("nexus_voice")


class VoiceServer:
    """WebSocket server that exposes VoiceEngine to the Tauri desktop app."""

    def __init__(self, host: str = "127.0.0.1", port: int = 9876) -> None:
        self.host = host
        self.port = port
        self.engine = VoiceEngine()
        self._server = None

    async def handle_connection(self, websocket) -> None:  # noqa: ANN001
        """Handle incoming WebSocket messages."""
        logger.info("Client connected from %s", websocket.remote_address)
        try:
            async for raw in websocket:
                try:
                    msg = json.loads(raw)
                except json.JSONDecodeError:
                    await websocket.send(
                        json.dumps({"error": "invalid JSON"})
                    )
                    continue

                action = msg.get("action", "")
                response = self._dispatch(action, msg)
                await websocket.send(json.dumps(response))
        except Exception as exc:
            logger.warning("Connection error: %s", exc)

    def _dispatch(self, action: str, msg: dict) -> dict:
        if action == "start_listening":
            self.engine.start_listening()
            return {"ok": True, "status": "listening"}

        if action == "stop_listening":
            self.engine.stop_listening()
            return {"ok": True, "status": "stopped"}

        if action == "transcribe":
            audio_b64 = msg.get("audio_base64", "")
            try:
                audio_bytes = base64.b64decode(audio_b64)
            except Exception:
                return {"error": "invalid base64 audio data"}
            text = self.engine.transcribe(audio_bytes)
            return {"ok": True, "text": text}

        if action == "detect_wake_word":
            audio_b64 = msg.get("audio_base64", "")
            try:
                audio_bytes = base64.b64decode(audio_b64)
            except Exception:
                return {"error": "invalid base64 audio data"}
            detected = self.engine.detect_wake_word(audio_bytes)
            return {"ok": True, "detected": detected}

        if action == "status":
            return {"ok": True, **self.engine.get_status()}

        return {"error": f"unknown action: {action}"}

    async def start(self) -> None:
        """Start the WebSocket server."""
        if websockets is None:
            logger.error(
                "websockets package not installed — "
                "run: pip install websockets>=12.0"
            )
            sys.exit(1)

        logger.info("Nexus Voice server starting on %s:%d", self.host, self.port)
        self._server = await ws_server.serve(
            self.handle_connection,
            self.host,
            self.port,
        )
        logger.info("Nexus Voice server ready")
        await self._server.serve_forever()


def main() -> None:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(name)s] %(levelname)s — %(message)s",
    )
    server = VoiceServer()
    asyncio.run(server.start())


if __name__ == "__main__":
    main()
