import { useEffect, useRef } from "react";

interface NeuralGraphNode {
  id: string;
  group?: string;
  activity?: number;
}

interface NeuralGraphEdge {
  from: string;
  to: string;
  weight?: number;
}

interface NeuralGraphProps {
  nodes: NeuralGraphNode[];
  edges: NeuralGraphEdge[];
  width?: number;
  height?: number;
}

interface Point {
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
}

export function NeuralGraph({ nodes, edges, width = 520, height = 220 }: NeuralGraphProps): JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      return;
    }

    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = Math.floor(width * dpr);
    canvas.height = Math.floor(height * dpr);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const points = new Map<string, Point>();
    for (const node of nodes) {
      points.set(node.id, {
        x: Math.random() * width,
        y: Math.random() * height,
        z: Math.random() * 1.8 - 0.9,
        vx: (Math.random() - 0.5) * 0.18,
        vy: (Math.random() - 0.5) * 0.18,
        vz: (Math.random() - 0.5) * 0.01
      });
    }

    const indexById = new Map(nodes.map((node) => [node.id, node]));

    let raf = 0;

    const step = (): void => {
      raf = window.requestAnimationFrame(step);
      ctx.clearRect(0, 0, width, height);

      for (const point of points.values()) {
        point.x += point.vx;
        point.y += point.vy;
        point.z += point.vz;
        if (point.x <= 8 || point.x >= width - 8) {
          point.vx *= -1;
        }
        if (point.y <= 8 || point.y >= height - 8) {
          point.vy *= -1;
        }
        if (point.z <= -1 || point.z >= 1) {
          point.vz *= -1;
        }
      }

      for (const edge of edges) {
        const a = points.get(edge.from);
        const b = points.get(edge.to);
        if (!a || !b) {
          continue;
        }
        const strength = Math.max(0.12, Math.min(1, edge.weight ?? 0.5));
        ctx.strokeStyle = `rgba(92, 242, 255, ${0.12 + strength * 0.3})`;
        ctx.lineWidth = 1 + strength * 0.8;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.stroke();
      }

      for (const [id, point] of points) {
        const node = indexById.get(id);
        const role = node?.group ?? "generic";
        const activity = Math.max(0.15, Math.min(1, node?.activity ?? 0.25));
        const radius = 3.2 + (point.z + 1) * 1.8 + activity;
        const color =
          role === "coding"
            ? "76, 190, 255"
            : role === "social"
              ? "72, 250, 163"
              : role === "design"
                ? "194, 136, 255"
                : "123, 240, 255";

        ctx.fillStyle = `rgba(${color}, ${0.5 + activity * 0.4})`;
        ctx.beginPath();
        ctx.arc(point.x, point.y, radius, 0, Math.PI * 2);
        ctx.fill();
      }
    };

    raf = window.requestAnimationFrame(step);
    return () => {
      window.cancelAnimationFrame(raf);
    };
  }, [edges, height, nodes, width]);

  return <canvas ref={canvasRef} className="viz-neural-graph" aria-label="Agent relationship graph" />;
}
