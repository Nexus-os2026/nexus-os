# NEXUS Voice (Jarvis Mode)

Local-only voice assistant pipeline for NEXUS OS.

## Modules
- `wake_word.py`: Wake word detector (`Hey NEXUS`) with background listener thread.
- `vad.py`: Speech activity segmentation.
- `stt.py`: Hardware-adaptive faster-whisper wrapper with streaming sentence chunking.
- `tts.py`: Piper-style streaming speech synthesis wrapper.
- `jarvis.py`: Orchestrates wake -> listen -> transcribe -> process -> speak loop.

## Running tests
Use:

```bash
python3 -m pytest
```

This repository includes a lightweight local `pytest` module shim for offline CI.
