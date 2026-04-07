/**
 * ActionBar — action buttons that appear after build is ready.
 *
 * [Edit] [Deploy] [Export ZIP] [Share]
 *
 * - Edit: toggles visual edit mode (Phase 6A)
 * - Deploy: placeholder for Phase 7A
 * - Export ZIP: triggers existing ZIP export
 * - Share: copies preview URL to clipboard
 *
 * Hidden during build, visible on "ready" state.
 */

import { useState, useCallback } from "react";
import DeployPanel from "./DeployPanel";
import DeployHistoryPanel from "./DeployHistory";
import QualityReportCard from "./QualityReportCard";
import ImprovementDashboard from "./ImprovementDashboard";

const C = {
  surface: "#111820",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  sans: "system-ui,-apple-system,sans-serif",
};

interface ActionBarProps {
  editMode: boolean;
  onToggleEdit: () => void;
  onExport: () => void;
  onShare: () => void;
  onVariants?: () => void;
  onBackend?: () => void;
  onImport?: () => void;
  onTheme?: () => void;
  themeOpen?: boolean;
  onAuditTrail?: () => void;
  previewUrl?: string;
  hasHtml: boolean;
  projectId?: string;
  onHtmlChanged?: () => void;
}

export default function ActionBar({
  editMode,
  onToggleEdit,
  onExport,
  onShare,
  onVariants,
  onBackend,
  onImport,
  onTheme,
  themeOpen,
  onAuditTrail,
  previewUrl,
  hasHtml,
  projectId,
  onHtmlChanged,
}: ActionBarProps) {
  const [deployOpen, setDeployOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [shareToast, setShareToast] = useState(false);
  const [qualityOpen, setQualityOpen] = useState(false);
  const [improvementOpen, setImprovementOpen] = useState(false);
  const [lastDeploy, setLastDeploy] = useState<{
    provider: string; site_id: string; deploy_id: string; url: string;
  } | null>(null);

  const handleDeploy = useCallback(() => {
    setDeployOpen(true);
  }, []);

  const handleShare = useCallback(() => {
    onShare();
    setShareToast(true);
    setTimeout(() => setShareToast(false), 1500);
  }, [onShare]);

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 4,
        padding: "4px 0",
      }}
    >
      {/* Edit */}
      <ActionButton
        label={editMode ? "Editing" : "Edit"}
        active={editMode}
        onClick={onToggleEdit}
        disabled={!hasHtml}
      />

      {/* Theme */}
      {onTheme && (
        <ActionButton
          label="Theme"
          active={themeOpen}
          onClick={onTheme}
          disabled={!hasHtml}
        />
      )}

      {/* Variants */}
      {onVariants && (
        <ActionButton
          label="Variants"
          onClick={onVariants}
          disabled={!hasHtml}
        />
      )}

      {/* Backend */}
      {onBackend && (
        <ActionButton
          label="Backend"
          onClick={onBackend}
          disabled={!hasHtml}
        />
      )}

      {/* Import */}
      {onImport && (
        <ActionButton
          label="Import"
          onClick={onImport}
        />
      )}

      {/* Audit Trail */}
      {onAuditTrail && (
        <ActionButton
          label="Governance"
          onClick={onAuditTrail}
        />
      )}

      {/* Self-Improvement */}
      <ActionButton
        label="Improve"
        onClick={() => setImprovementOpen(true)}
      />
      <ImprovementDashboard
        open={improvementOpen}
        onClose={() => setImprovementOpen(false)}
      />

      {/* Deploy */}
      <ActionButton
        label={lastDeploy ? "Redeploy" : "Deploy"}
        onClick={handleDeploy}
        disabled={!hasHtml}
      />
      {deployOpen && projectId && (
        <DeployPanel
          projectId={projectId}
          onClose={() => setDeployOpen(false)}
          lastDeploy={lastDeploy}
          onHistory={() => { setDeployOpen(false); setHistoryOpen(true); }}
        />
      )}
      {historyOpen && projectId && (
        <DeployHistoryPanel
          projectId={projectId}
          onClose={() => setHistoryOpen(false)}
        />
      )}

      {/* Export ZIP */}
      <ActionButton label="ZIP" onClick={onExport} disabled={!hasHtml} />

      {/* Share */}
      <div style={{ position: "relative" }}>
        <ActionButton label="Share" onClick={handleShare} disabled={!previewUrl && !hasHtml} />
        {shareToast && (
          <div
            style={{
              position: "absolute",
              top: "100%",
              left: "50%",
              transform: "translateX(-50%)",
              marginTop: 4,
              background: C.accentDim,
              border: `1px solid rgba(0,212,170,0.25)`,
              borderRadius: 4,
              padding: "3px 8px",
              fontSize: 9,
              color: C.accent,
              whiteSpace: "nowrap",
              zIndex: 10,
            }}
          >
            Copied!
          </div>
        )}
      </div>

      {/* Quality */}
      {projectId && hasHtml && (
        <div style={{ position: "relative" }}>
          <ActionButton
            label="Quality"
            onClick={() => setQualityOpen(!qualityOpen)}
            active={qualityOpen}
          />
          {qualityOpen && (
            <div style={{
              position: "absolute",
              top: "100%",
              right: 0,
              marginTop: 6,
              zIndex: 20,
            }}>
              <QualityReportCard projectId={projectId} onHtmlChanged={onHtmlChanged} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function ActionButton({
  label,
  onClick,
  active,
  disabled,
}: {
  label: string;
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      style={{
        background: active ? C.accentDim : "transparent",
        color: active ? C.accent : disabled ? C.dim : C.muted,
        border: active
          ? "1px solid rgba(0,212,170,0.25)"
          : "1px solid transparent",
        borderRadius: 4,
        padding: "3px 9px",
        fontSize: 10,
        cursor: disabled ? "default" : "pointer",
        fontWeight: active ? 600 : 400,
        fontFamily: C.sans,
        transition: "all 0.15s ease",
      }}
    >
      {label}
    </button>
  );
}
