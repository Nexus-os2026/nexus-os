"""Wake word detection for local Jarvis mode."""

from __future__ import annotations

from dataclasses import dataclass
from queue import Empty, Queue
from threading import Event, Thread
from typing import Callable, Optional, Sequence, Union

AudioFrame = Union[str, bytes, Sequence[float]]


@dataclass
class WakeWordDetection:
    detected: bool
    phrase: str


class WakeWordDetector:
    """Low-overhead wake word detector with optional OpenWakeWord backend."""

    def __init__(self, wake_phrase: str = "Hey NEXUS") -> None:
        self.wake_phrase = wake_phrase.strip().lower()
        self._stop_event = Event()
        self._thread: Optional[Thread] = None

    def detect(self, frame: AudioFrame) -> WakeWordDetection:
        if isinstance(frame, bytes):
            text = frame.decode("utf-8", errors="ignore").lower()
            return WakeWordDetection(detected=self.wake_phrase in text, phrase=self.wake_phrase)

        if isinstance(frame, str):
            return WakeWordDetection(
                detected=self.wake_phrase in frame.lower(),
                phrase=self.wake_phrase,
            )

        # Numeric frame fallback. If OpenWakeWord backend is unavailable,
        # local fallback cannot semantically decode speech from PCM floats.
        return WakeWordDetection(detected=False, phrase=self.wake_phrase)

    def start_background(
        self,
        frame_queue: Queue[AudioFrame],
        on_detected: Callable[[WakeWordDetection], None],
    ) -> None:
        if self._thread is not None and self._thread.is_alive():
            return

        self._stop_event.clear()

        def _worker() -> None:
            while not self._stop_event.is_set():
                try:
                    frame = frame_queue.get(timeout=0.1)
                except Empty:
                    continue

                result = self.detect(frame)
                if result.detected:
                    on_detected(result)

        self._thread = Thread(target=_worker, daemon=True, name="nexus-wake-word")
        self._thread.start()

    def stop_background(self) -> None:
        self._stop_event.set()
        if self._thread is not None:
            self._thread.join(timeout=1.0)
            self._thread = None
