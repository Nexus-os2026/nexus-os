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
    score: float = 0.0


class WakeWordDetector:
    """Low-overhead wake word detector with optional OpenWakeWord backend."""

    def __init__(
        self,
        wake_phrase: str = "Hey NEXUS",
        model_name: str = "hey_jarvis",
        detection_threshold: float = 0.5,
    ) -> None:
        self.wake_phrase = wake_phrase.strip().lower()
        self.model_name = model_name
        self.detection_threshold = detection_threshold
        self._openwakeword_model = self._load_openwakeword_model()
        self._stop_event = Event()
        self._thread: Optional[Thread] = None

    def _load_openwakeword_model(self) -> object | None:
        try:
            from openwakeword import Model  # type: ignore

            return Model(wakeword_models=[self.model_name])
        except Exception:
            return None

    def detect(self, frame: AudioFrame) -> WakeWordDetection:
        if isinstance(frame, bytes):
            text = frame.decode("utf-8", errors="ignore").lower()
            return WakeWordDetection(
                detected=self.wake_phrase in text,
                phrase=self.wake_phrase,
                score=1.0 if self.wake_phrase in text else 0.0,
            )

        if isinstance(frame, str):
            normalized = frame.lower()
            return WakeWordDetection(
                detected=self.wake_phrase in normalized,
                phrase=self.wake_phrase,
                score=1.0 if self.wake_phrase in normalized else 0.0,
            )

        if self._openwakeword_model is not None:
            score = self._predict_score(frame)
            return WakeWordDetection(
                detected=score >= self.detection_threshold,
                phrase=self.wake_phrase,
                score=score,
            )

        return WakeWordDetection(detected=False, phrase=self.wake_phrase, score=0.0)

    def _predict_score(self, frame: Sequence[float]) -> float:
        try:
            import numpy as np  # type: ignore
        except Exception:
            return 0.0

        try:
            audio = np.asarray(frame, dtype=np.float32)
            predictions = self._openwakeword_model.predict(audio)  # type: ignore[union-attr]
            if isinstance(predictions, dict):
                if self.model_name in predictions:
                    return float(predictions[self.model_name])
                if predictions:
                    return float(next(iter(predictions.values())))
            if isinstance(predictions, (list, tuple)) and predictions:
                return float(predictions[0])
            if isinstance(predictions, (int, float)):
                return float(predictions)
        except Exception:
            return 0.0
        return 0.0

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
