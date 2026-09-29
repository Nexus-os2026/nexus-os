"""Keep the voice tests offline: no test may download a model.

Every test module imports this module before anything else. It sets, in
this process's environment, the switches the model libraries honour, and
the stt.py processes the tests start inherit them:

- HF_HUB_OFFLINE: huggingface_hub, through which faster-whisper fetches its
  Whisper models, uses only its local cache and makes no request;
- HF_HUB_DISABLE_TELEMETRY and HF_HUB_DISABLE_UPDATE_CHECK: it sends no
  telemetry and no version check.

huggingface_hub reads these when it is first imported, which here is when a
test first constructs a Whisper backend. A model that is not already cached
then fails to load, and the tests that need one skip.

silero-vad and openwakeword load models from files in their own packages,
piper runs as a command with the model path it is given, and no voice
module downloads anything itself.
"""

import os

OFFLINE_ENVIRONMENT = {
    "HF_HUB_OFFLINE": "1",
    "HF_HUB_DISABLE_TELEMETRY": "1",
    "HF_HUB_DISABLE_UPDATE_CHECK": "1",
}

os.environ.update(OFFLINE_ENVIRONMENT)
