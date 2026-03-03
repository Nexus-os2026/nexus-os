import { useEffect, useMemo, useRef } from "react";

interface NeuralBackgroundProps {
  activityPulse?: number;
}

interface NeuralNode {
  x: number;
  y: number;
  vx: number;
  vy: number;
  energy: number;
}

const MAX_LINK_DISTANCE = 150;
const NODE_COUNT = 34;
const FLOW_PARTICLES = 22;

function makeNode(width: number, height: number): NeuralNode {
  return {
    x: Math.random() * width,
    y: Math.random() * height,
    vx: (Math.random() - 0.5) * 0.2,
    vy: (Math.random() - 0.5) * 0.2,
    energy: 0.14 + Math.random() * 0.22
  };
}

export function NeuralBackground({ activityPulse = 0 }: NeuralBackgroundProps): JSX.Element {
  const webglCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const overlayCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const offscreenRef = useRef<HTMLCanvasElement | null>(null);

  const signature = useMemo(() => activityPulse, [activityPulse]);

  useEffect(() => {
    const webglCanvas = webglCanvasRef.current;
    const overlayCanvas = overlayCanvasRef.current;
    if (!webglCanvas || !overlayCanvas) {
      return;
    }
    const webglCanvasEl = webglCanvas;
    const overlayCanvasEl = overlayCanvas;

    const webgl = webglCanvas.getContext("webgl", {
      alpha: true,
      antialias: false,
      powerPreference: "low-power",
      preserveDrawingBuffer: false
    });
    const overlayCtx = overlayCanvasEl.getContext("2d");
    if (!overlayCtx) {
      return;
    }
    const overlayContext = overlayCtx;

    const offscreen = offscreenRef.current ?? document.createElement("canvas");
    offscreenRef.current = offscreen;
    const offscreenCtx = offscreen.getContext("2d");
    if (!offscreenCtx) {
      return;
    }
    const offscreenContext = offscreenCtx;

    let width = window.innerWidth;
    let height = window.innerHeight;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);

    const nodes = Array.from({ length: NODE_COUNT }, () => makeNode(width, height));
    const flowSeeds = Array.from({ length: FLOW_PARTICLES }, (_, index) => ({
      nodeA: index % NODE_COUNT,
      nodeB: (index * 7 + 11) % NODE_COUNT,
      progress: Math.random()
    }));

    function resize(): void {
      width = window.innerWidth;
      height = window.innerHeight;

      webglCanvasEl.width = Math.floor(width * dpr);
      webglCanvasEl.height = Math.floor(height * dpr);
      webglCanvasEl.style.width = `${width}px`;
      webglCanvasEl.style.height = `${height}px`;

      overlayCanvasEl.width = Math.floor(width * dpr);
      overlayCanvasEl.height = Math.floor(height * dpr);
      overlayCanvasEl.style.width = `${width}px`;
      overlayCanvasEl.style.height = `${height}px`;

      offscreen.width = Math.floor(width * dpr);
      offscreen.height = Math.floor(height * dpr);

      overlayContext.setTransform(dpr, 0, 0, dpr, 0, 0);
      offscreenContext.setTransform(dpr, 0, 0, dpr, 0, 0);

      if (webgl) {
        webgl.viewport(0, 0, webglCanvasEl.width, webglCanvasEl.height);
      }
    }

    resize();

    let raf = 0;
    let lastTs = 0;
    let lastSignal = signature;

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const tick = (ts: number): void => {
      raf = window.requestAnimationFrame(tick);

      if (!lastTs) {
        lastTs = ts;
      }
      const delta = ts - lastTs;
      if (delta < 32) {
        return;
      }
      lastTs = ts;

      if (webgl) {
        const glow = 0.055 + 0.02 * Math.sin(ts * 0.00027);
        webgl.clearColor(0.01 + glow * 0.12, 0.03 + glow * 0.24, 0.07 + glow * 0.32, 0.92);
        webgl.clear(webgl.COLOR_BUFFER_BIT);
      }

      if (signature !== lastSignal) {
        lastSignal = signature;
        for (let index = 0; index < 6; index += 1) {
          const node = nodes[Math.floor(Math.random() * nodes.length)];
          node.energy = 1;
        }
      }

      offscreenContext.clearRect(0, 0, width, height);

      for (const node of nodes) {
        if (!reducedMotion) {
          node.x += node.vx * delta;
          node.y += node.vy * delta;
        }
        if (node.x <= 0 || node.x >= width) {
          node.vx *= -1;
          node.x = Math.max(0, Math.min(width, node.x));
        }
        if (node.y <= 0 || node.y >= height) {
          node.vy *= -1;
          node.y = Math.max(0, Math.min(height, node.y));
        }
        node.energy *= 0.985;
        node.energy = Math.max(node.energy, 0.16);
      }

      for (let a = 0; a < nodes.length; a += 1) {
        for (let b = a + 1; b < nodes.length; b += 1) {
          const first = nodes[a];
          const second = nodes[b];
          const dx = second.x - first.x;
          const dy = second.y - first.y;
          const distance = Math.sqrt(dx * dx + dy * dy);
          if (distance > MAX_LINK_DISTANCE) {
            continue;
          }
          const strength = 1 - distance / MAX_LINK_DISTANCE;
          offscreenContext.strokeStyle = `rgba(64, 224, 255, ${strength * 0.22})`;
          offscreenContext.lineWidth = 1;
          offscreenContext.beginPath();
          offscreenContext.moveTo(first.x, first.y);
          offscreenContext.lineTo(second.x, second.y);
          offscreenContext.stroke();
        }
      }

      for (const flow of flowSeeds) {
        flow.progress += reducedMotion ? 0.002 : 0.003 + Math.random() * 0.003;
        if (flow.progress > 1) {
          flow.progress = 0;
          flow.nodeA = Math.floor(Math.random() * nodes.length);
          flow.nodeB = Math.floor(Math.random() * nodes.length);
          if (flow.nodeA === flow.nodeB) {
            flow.nodeB = (flow.nodeB + 1) % nodes.length;
          }
        }
        const start = nodes[flow.nodeA];
        const end = nodes[flow.nodeB];
        const px = start.x + (end.x - start.x) * flow.progress;
        const py = start.y + (end.y - start.y) * flow.progress;
        offscreenContext.fillStyle = "rgba(120, 244, 255, 0.7)";
        offscreenContext.beginPath();
        offscreenContext.arc(px, py, 1.2, 0, Math.PI * 2);
        offscreenContext.fill();
      }

      for (const node of nodes) {
        const radius = 1.8 + node.energy * 2.4;
        const alpha = 0.28 + node.energy * 0.55;
        offscreenContext.fillStyle = `rgba(130, 246, 255, ${alpha})`;
        offscreenContext.beginPath();
        offscreenContext.arc(node.x, node.y, radius, 0, Math.PI * 2);
        offscreenContext.fill();
      }

      overlayContext.clearRect(0, 0, width, height);
      overlayContext.drawImage(offscreen, 0, 0, width, height);
    };

    raf = window.requestAnimationFrame(tick);
    window.addEventListener("resize", resize);

    return () => {
      window.cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
    };
  }, [signature]);

  return (
    <div className="neural-background" aria-hidden="true">
      <canvas ref={webglCanvasRef} className="neural-layer neural-layer-webgl" />
      <canvas ref={overlayCanvasRef} className="neural-layer neural-layer-overlay" />
    </div>
  );
}
