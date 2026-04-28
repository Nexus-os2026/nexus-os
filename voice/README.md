# NEXUS Voice (Jarvis Mode)

Local-only voice assistant pipeline for NEXUS OS.

## Modules
- `wake_word.py`: Wake word detector (`Hey NEXUS`) with background listener thread.
- `vad.py`: Speech activity segmentation.
- `stt.py`: Hardware-adaptive faster-whisper wrapper with streaming sentence chunking.
- `tts.py`: Piper-style streaming speech synthesis wrapper.
- `jarvis.py`: Orchestrates wake -> listen -> transcribe -> process -> speak loop.

## CLI entrypoints (Track C #3)

`stt.py` exposes a JSON-over-stdout CLI consumed by the Tauri
`transcribe_push_to_talk` and `voice_pipeline_health` commands.

```bash
python3 stt.py --health
# stdout (exit 0): {"status": "ok", "model": "tiny"}
# stderr (exit 1): {"status": "error", "reason": "<msg>"}

python3 stt.py transcribe path/to/audio.wav
# stdout (exit 0): {
#   "text": "...",
#   "language": "en",
#   "confidence": 0.92,
#   "latency_ms": 412.3,
#   "model": "tiny",
#   "sentence_chunks": ["..."]
# }
# stderr (exit 1): {"status": "error", "reason": "missing_file: ..."}
```

Stdout is reserved for the success payload (Rust parses it directly).
All errors go to stderr with the `{status, reason}` shape and exit 1.

## Running tests
Use:

```bash
python3 -m pytest
```

This repository includes a lightweight local `pytest` module shim for offline CI.
