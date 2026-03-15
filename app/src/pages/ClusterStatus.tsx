import { useEffect, useState } from "react";
import { hasDesktopRuntime, getLiveSystemMetrics } from "../api/backend";
import "./cluster-status.css";

export default function ClusterStatus(): JSX.Element {
  const [runtimeName, setRuntimeName] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function loadRuntime() {
      if (!hasDesktopRuntime()) {
        setLoading(false);
        return;
      }
      try {
        const raw = await getLiveSystemMetrics();
        const data = JSON.parse(raw);
        setRuntimeName(data.cpu_name || "Local runtime");
      } catch {
        setRuntimeName(null);
      }
      setLoading(false);
    }
    void loadRuntime();
  }, []);

  return (
    <section className="cs-hub">
      <header className="cs-header">
        <h2 className="cs-title">CLUSTER STATUS // NODE HEALTH</h2>
        <p className="cs-subtitle">Single node mode</p>
      </header>

      {loading && <div style={{ padding: "2rem", textAlign: "center", opacity: 0.5 }}>Loading cluster status...</div>}

      {!loading && (
        <div style={{ padding: "3rem", textAlign: "center", opacity: 0.5 }}>
          <p>Single node mode</p>
          <p>
            Distributed cluster peers are not configured.
            {runtimeName ? ` Local runtime: ${runtimeName}.` : ""}
          </p>
        </div>
      )}
    </section>
  );
}
