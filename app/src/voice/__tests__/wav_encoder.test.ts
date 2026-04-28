import { describe, expect, it } from "vitest";
import {
  downsampleFloat32,
  encodeFloat32ChunksToWav,
  encodeWav,
  floatTo16BitPCM,
} from "../wav_encoder";

function readAscii(view: DataView, offset: number, length: number): string {
  let s = "";
  for (let i = 0; i < length; i += 1) {
    s += String.fromCharCode(view.getUint8(offset + i));
  }
  return s;
}

describe("downsampleFloat32", () => {
  it("48kHz → 16kHz reduces sample count by 3x within 1 sample tolerance", () => {
    const input = new Float32Array(48_000);
    for (let i = 0; i < input.length; i += 1) input[i] = i / input.length;
    const out = downsampleFloat32(input, 48_000, 16_000);
    expect(Math.abs(out.length - 16_000)).toBeLessThanOrEqual(1);
  });

  it("returns the input unchanged when fromRate <= toRate", () => {
    const input = new Float32Array([0.1, 0.2, 0.3]);
    expect(downsampleFloat32(input, 16_000, 16_000)).toBe(input);
    expect(downsampleFloat32(input, 8_000, 16_000)).toBe(input);
  });

  it("preserves linear ramp shape across the downsample (endpoints match)", () => {
    const input = new Float32Array(96);
    for (let i = 0; i < input.length; i += 1) input[i] = i / 96;
    const out = downsampleFloat32(input, 48_000, 16_000);
    expect(out.length).toBe(32);
    expect(out[0]).toBeCloseTo(0, 3);
    expect(out[31]).toBeGreaterThan(0.9);
  });
});

describe("floatTo16BitPCM", () => {
  it("maps 1.0 → 32767, -1.0 → -32768, 0 → 0", () => {
    const out = floatTo16BitPCM(new Float32Array([1.0, -1.0, 0]));
    expect(out[0]).toBe(0x7fff);
    expect(out[1]).toBe(-0x8000);
    expect(out[2]).toBe(0);
  });

  it("0.5 maps to ~16384", () => {
    const out = floatTo16BitPCM(new Float32Array([0.5]));
    // 0.5 * 0x7FFF = 16383.5 → floor = 16383 (within 1 of 16384)
    expect(Math.abs(out[0] - 16_384)).toBeLessThanOrEqual(1);
  });

  it("clips out-of-range samples", () => {
    const out = floatTo16BitPCM(new Float32Array([1.5, -1.5, 2.5, -2.5]));
    expect(out[0]).toBe(0x7fff);
    expect(out[1]).toBe(-0x8000);
    expect(out[2]).toBe(0x7fff);
    expect(out[3]).toBe(-0x8000);
  });
});

describe("encodeWav", () => {
  it("writes a 44-byte header with valid RIFF/fmt/data identifiers", () => {
    const samples = new Int16Array([0, 100, -100, 0]);
    const wav = encodeWav(samples, 16_000);
    expect(wav.byteLength).toBe(44 + samples.length * 2);
    const view = new DataView(wav.buffer, wav.byteOffset, wav.byteLength);
    expect(readAscii(view, 0, 4)).toBe("RIFF");
    expect(readAscii(view, 8, 4)).toBe("WAVE");
    expect(readAscii(view, 12, 4)).toBe("fmt ");
    expect(readAscii(view, 36, 4)).toBe("data");
  });

  it("encodes the sample rate, byte rate, and bits-per-sample fields", () => {
    const samples = new Int16Array(0);
    const wav = encodeWav(samples, 16_000);
    const view = new DataView(wav.buffer, wav.byteOffset, wav.byteLength);
    expect(view.getUint16(20, true)).toBe(1); // AudioFormat = PCM
    expect(view.getUint16(22, true)).toBe(1); // NumChannels = mono
    expect(view.getUint32(24, true)).toBe(16_000); // SampleRate
    expect(view.getUint32(28, true)).toBe(32_000); // ByteRate = 16k * 1 * 2
    expect(view.getUint16(32, true)).toBe(2); // BlockAlign
    expect(view.getUint16(34, true)).toBe(16); // BitsPerSample
  });

  it("writes the PCM payload little-endian after the header", () => {
    const samples = new Int16Array([0x1234, -1, 0x7fff]);
    const wav = encodeWav(samples, 16_000);
    const view = new DataView(wav.buffer, wav.byteOffset, wav.byteLength);
    expect(view.getInt16(44, true)).toBe(0x1234);
    expect(view.getInt16(46, true)).toBe(-1);
    expect(view.getInt16(48, true)).toBe(0x7fff);
  });
});

describe("encodeFloat32ChunksToWav", () => {
  it("round-trips 1s of 440Hz sine at 48kHz → 16kHz output, header correct, payload size matches", () => {
    const sourceRate = 48_000;
    const targetRate = 16_000;
    const seconds = 1;
    // Build the chunk list by slicing a single 1s sine into 4096-frame
    // pieces — mirrors how ScriptProcessor delivers buffers.
    const total = sourceRate * seconds;
    const sine = new Float32Array(total);
    for (let i = 0; i < total; i += 1) {
      sine[i] = Math.sin((2 * Math.PI * 440 * i) / sourceRate) * 0.5;
    }
    const chunkSize = 4096;
    const chunks: Float32Array[] = [];
    for (let i = 0; i < total; i += chunkSize) {
      chunks.push(sine.slice(i, Math.min(i + chunkSize, total)));
    }

    const wav = encodeFloat32ChunksToWav(chunks, sourceRate, targetRate);
    const expectedPcmBytes = targetRate * seconds * 2; // 16k * 1s * 2 bytes/sample
    expect(wav.byteLength).toBe(44 + expectedPcmBytes);
    const view = new DataView(wav.buffer, wav.byteOffset, wav.byteLength);
    expect(readAscii(view, 0, 4)).toBe("RIFF");
    expect(view.getUint32(24, true)).toBe(targetRate);
    // First sample should be near zero (sine starts at 0).
    expect(Math.abs(view.getInt16(44, true))).toBeLessThan(2_000);
  });

  it("handles empty chunk arrays without throwing", () => {
    const wav = encodeFloat32ChunksToWav([], 48_000, 16_000);
    expect(wav.byteLength).toBe(44); // header only, no payload
  });
});
