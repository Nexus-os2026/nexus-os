interface AvatarProps {
  agentName: string;
  role: "coding" | "social" | "design" | "general";
  state: "running" | "paused" | "stopped" | "idle";
  size?: number;
}

function hash(input: string): number {
  let h = 0;
  for (let index = 0; index < input.length; index += 1) {
    h = (h << 5) - h + input.charCodeAt(index);
    h |= 0;
  }
  return Math.abs(h);
}

function roleColor(role: AvatarProps["role"]): string {
  if (role === "coding") {
    return "#38bdf8";
  }
  if (role === "social") {
    return "#4ade80";
  }
  if (role === "design") {
    return "#c084fc";
  }
  return "#67e8f9";
}

export function Avatar({ agentName, role, state, size = 52 }: AvatarProps): JSX.Element {
  const base = hash(`${agentName}:${role}`);
  const color = roleColor(role);
  const points = Array.from({ length: 6 }, (_, index) => {
    const angle = (Math.PI * 2 * index) / 6;
    const radius = 14 + ((base >> index) & 3) * 2;
    const x = 20 + Math.cos(angle) * radius;
    const y = 20 + Math.sin(angle) * radius;
    return `${x.toFixed(2)},${y.toFixed(2)}`;
  }).join(" ");
  const innerPoints = Array.from({ length: 5 }, (_, index) => {
    const angle = (Math.PI * 2 * index) / 5 + 0.25;
    const radius = 8 + ((base >> (index + 3)) & 1) * 2;
    const x = 20 + Math.cos(angle) * radius;
    const y = 20 + Math.sin(angle) * radius;
    return `${x.toFixed(2)},${y.toFixed(2)}`;
  }).join(" ");

  return (
    <div
      className={`agent-avatar state-${state}`}
      style={{ width: size, height: size, ["--avatar-color" as string]: color }}
      aria-hidden="true"
    >
      <svg viewBox="0 0 40 40" className="agent-avatar__svg">
        <polygon points={points} className="agent-avatar__outer" />
        <polygon points={innerPoints} className="agent-avatar__inner" />
        <circle cx="20" cy="20" r="2.6" className="agent-avatar__core" />
      </svg>
      <span className="agent-avatar__field" />
    </div>
  );
}
