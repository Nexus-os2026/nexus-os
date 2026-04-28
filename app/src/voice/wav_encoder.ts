/**
 * Track C #3 commit 2: WAV encoder for the local STT capture path.
 *
 * Pure functions, no React, no DOM dependencies. The capture pipeline
 * in PushToTalk pulls Float32Array chunks at the device's native
 * sample rate (typically 48000Hz); the backend (`/voice/stt.py`)
 * expects 16kHz mono PCM s16le wrapped in a standard 44-byte WAV
 * header. This module is the bridge.
 *
 * Composition: `encodeFloat32ChunksToWav` runs the full pipeline
 * (concat → downsample → float→int16 → WAV header). Individual
 * functions are exported for unit testing and for callers that want
 * intermediate forms.
 */

const WAV_HEADER_BYTES = 44;
const PCM_FORMAT = 1; // 16-bit linear PCM
const NUM_CHANNELS = 1; // mono
const BITS_PER_SAMPLE = 16;

/**
 * Concatenate `Float32Array` chunks into a single buffer. Caller-side
 * helper kept inline to the module so no new utility surface escapes.
 */
function concatFloat32(chunks: readonly Float32Array[]): Float32Array {
  let total = 0;
  for (const chunk of chunks) total += chunk.length;
  const out = new Float32Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

/**
 * Linear-interpolation downsampler. If `fromRate <= toRate` the input
 * is returned unchanged (we never up-sample — the backend is fine with
 * higher-than-16k input but the frontend never sees that case in
 * practice).
 */
export function downsampleFloat32(
  input: Float32Array,
  fromRate: number,
  toRate: number,
): Float32Array {
  if (fromRate <= toRate) return input;
  const ratio = fromRate / toRate;
  const outLength = Math.floor(input.length / ratio);
  const out = new Float32Array(outLength);
  for (let i = 0; i < outLength; i += 1) {
    const srcPos = i * ratio;
    const idx = Math.floor(srcPos);
    const frac = srcPos - idx;
    const a = input[idx] ?? 0;
    const b = input[idx + 1] ?? a;
    out[i] = a + (b - a) * frac;
  }
  return out;
}

/**
 * Float32 (-1..1) → Int16 (s16le). Clamps out-of-range samples;
 * positive scale is `0x7FFF`, negative is `-0x8000`.
 */
export function floatTo16BitPCM(input: Float32Array): Int16Array {
  const out = new Int16Array(input.length);
  for (let i = 0; i < input.length; i += 1) {
    const sample = input[i];
    if (sample >= 1) {
      out[i] = 0x7fff;
    } else if (sample <= -1) {
      out[i] = -0x8000;
    } else if (sample >= 0) {
      out[i] = Math.floor(sample * 0x7fff);
    } else {
      out[i] = Math.ceil(sample * 0x8000);
    }
  }
  return out;
}

/**
 * Wrap an `Int16Array` PCM buffer in a 44-byte WAV header. Mono,
 * 16-bit, sample rate from caller.
 */
export function encodeWav(samples: Int16Array, sampleRate: number): Uint8Array {
  const dataBytes = samples.length * 2;
  const buffer = new ArrayBuffer(WAV_HEADER_BYTES + dataBytes);
  const view = new DataView(buffer);

  // RIFF header.
  writeAscii(view, 0, "RIFF");
  view.setUint32(4, 36 + dataBytes, true); // ChunkSize
  writeAscii(view, 8, "WAVE");

  // fmt sub-chunk.
  writeAscii(view, 12, "fmt ");
  view.setUint32(16, 16, true); // Subchunk1Size (16 for PCM)
  view.setUint16(20, PCM_FORMAT, true); // AudioFormat
  view.setUint16(22, NUM_CHANNELS, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * NUM_CHANNELS * (BITS_PER_SAMPLE / 8), true); // ByteRate
  view.setUint16(32, NUM_CHANNELS * (BITS_PER_SAMPLE / 8), true); // BlockAlign
  view.setUint16(34, BITS_PER_SAMPLE, true);

  // data sub-chunk.
  writeAscii(view, 36, "data");
  view.setUint32(40, dataBytes, true);

  // PCM payload, little-endian.
  for (let i = 0; i < samples.length; i += 1) {
    view.setInt16(WAV_HEADER_BYTES + i * 2, samples[i], true);
  }

  return new Uint8Array(buffer);
}

/**
 * The full capture-to-wire pipeline: concat → downsample → s16 → WAV.
 * `chunks` are typically the raw `Float32Array` buffers handed to
 * ScriptProcessor's `onaudioprocess`.
 */
export function encodeFloat32ChunksToWav(
  chunks: readonly Float32Array[],
  sourceSampleRate: number,
  targetSampleRate: number,
): Uint8Array {
  const concat = concatFloat32(chunks);
  const downsampled = downsampleFloat32(concat, sourceSampleRate, targetSampleRate);
  const int16 = floatTo16BitPCM(downsampled);
  return encodeWav(int16, targetSampleRate);
}

function writeAscii(view: DataView, offset: number, text: string): void {
  for (let i = 0; i < text.length; i += 1) {
    view.setUint8(offset + i, text.charCodeAt(i));
  }
}
