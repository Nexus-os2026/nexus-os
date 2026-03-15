import { useState, useCallback, useMemo, useRef } from "react";
import "./media-studio.css";

/* ─── types ─── */
type View = "library" | "editor" | "generate" | "compare";
type MediaType = "image" | "video" | "svg" | "gif";
type Filter = "none" | "grayscale" | "sepia" | "blur" | "brightness" | "contrast" | "saturate" | "invert" | "hue-rotate";

interface MediaAsset {
  id: string;
  name: string;
  type: MediaType;
  size: number;
  width: number;
  height: number;
  folder: string;
  tags: string[];
  createdAt: number;
  modifiedAt: number;
  thumbnail: string; // CSS gradient placeholder
  agent?: string;
}

interface Annotation {
  id: string;
  type: "rect" | "arrow" | "text" | "circle";
  x: number;
  y: number;
  w: number;
  h: number;
  color: string;
  label?: string;
}

interface CropRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/* ─── constants ─── */
const FILTERS: { id: Filter; label: string; css: string }[] = [
  { id: "none", label: "None", css: "none" },
  { id: "grayscale", label: "Grayscale", css: "grayscale(100%)" },
  { id: "sepia", label: "Sepia", css: "sepia(100%)" },
  { id: "blur", label: "Blur", css: "blur(3px)" },
  { id: "brightness", label: "Bright", css: "brightness(1.4)" },
  { id: "contrast", label: "Contrast", css: "contrast(1.6)" },
  { id: "saturate", label: "Saturate", css: "saturate(2)" },
  { id: "invert", label: "Invert", css: "invert(100%)" },
  { id: "hue-rotate", label: "Hue Shift", css: "hue-rotate(90deg)" },
];

const FOLDERS = ["All", "Screenshots", "Generated", "Uploads", "Agent Output", "Exports"];

const EXPORT_FORMATS = ["PNG", "JPEG", "WebP"];

const AI_STYLES = ["Photorealistic", "Digital Art", "Cyberpunk", "Minimalist", "Watercolor", "3D Render", "Pixel Art", "Sketch"];

/* ─── component ─── */
export default function MediaStudio() {
  const [view, setView] = useState<View>("library");
  const [assets, setAssets] = useState<MediaAsset[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedFolder, setSelectedFolder] = useState("All");
  const [searchQuery, setSearchQuery] = useState("");
  const [sortBy, setSortBy] = useState<"date" | "name" | "size">("date");
  const [gridSize, setGridSize] = useState<"sm" | "md" | "lg">("md");
  const [fuelUsed, setFuelUsed] = useState(0);
  const [auditLog, setAuditLog] = useState<string[]>([]);

  // editor state
  const [activeFilter, setActiveFilter] = useState<Filter>("none");
  const [brightness, setBrightness] = useState(100);
  const [contrast, setContrast] = useState(100);
  const [saturation, setSaturation] = useState(100);
  const [rotation, setRotation] = useState(0);
  const [flipH, setFlipH] = useState(false);
  const [flipV, setFlipV] = useState(false);
  const [crop, setCrop] = useState<CropRect | null>(null);
  const [showCrop, setShowCrop] = useState(false);
  const [annotations, setAnnotations] = useState<Annotation[]>([]);
  const [annotTool, setAnnotTool] = useState<Annotation["type"] | null>(null);
  const [annotColor, setAnnotColor] = useState("var(--nexus-accent)");

  // generate state
  const [genPrompt, setGenPrompt] = useState("");
  const [genStyle, setGenStyle] = useState("Cyberpunk");
  const [genSize, setGenSize] = useState("1024x1024");
  const [generating, setGenerating] = useState(false);

  // compare state
  const [compareA, setCompareA] = useState<string | null>(null);
  const [compareB, setCompareB] = useState<string | null>(null);
  const [sliderPos, setSliderPos] = useState(50);
  const sliderRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  // Store real uploaded image data URLs keyed by asset id
  const [imageDataUrls, setImageDataUrls] = useState<Record<string, string>>({});

  // canvas ref for real export
  const exportCanvasRef = useRef<HTMLCanvasElement>(null);

  const selectedAsset = useMemo(() => assets.find(a => a.id === selectedId) ?? null, [assets, selectedId]);

  /* ─── filtered assets ─── */
  const filteredAssets = useMemo(() => {
    let list = selectedFolder === "All" ? [...assets] : assets.filter(a => a.folder === selectedFolder);
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(a => a.name.toLowerCase().includes(q) || a.tags.some(t => t.includes(q)));
    }
    list.sort((a, b) => {
      if (sortBy === "name") return a.name.localeCompare(b.name);
      if (sortBy === "size") return b.size - a.size;
      return b.modifiedAt - a.modifiedAt;
    });
    return list;
  }, [assets, selectedFolder, searchQuery, sortBy]);

  const logAudit = useCallback((msg: string) => setAuditLog(prev => [msg, ...prev].slice(0, 30)), []);

  /* ─── helpers ─── */
  const formatSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1048576) return `${(bytes / 1024).toFixed(0)} KB`;
    return `${(bytes / 1048576).toFixed(1)} MB`;
  };

  const formatDate = (ts: number) => new Date(ts).toLocaleDateString();

  const getFilterCSS = () => {
    const parts: string[] = [];
    const f = FILTERS.find(f => f.id === activeFilter);
    if (f && f.css !== "none") parts.push(f.css);
    if (brightness !== 100) parts.push(`brightness(${brightness / 100})`);
    if (contrast !== 100) parts.push(`contrast(${contrast / 100})`);
    if (saturation !== 100) parts.push(`saturate(${saturation / 100})`);
    return parts.length ? parts.join(" ") : "none";
  };

  const getTransformCSS = () => {
    const parts: string[] = [];
    if (rotation !== 0) parts.push(`rotate(${rotation}deg)`);
    if (flipH) parts.push("scaleX(-1)");
    if (flipV) parts.push("scaleY(-1)");
    return parts.length ? parts.join(" ") : "none";
  };

  const resetEditor = () => {
    setActiveFilter("none");
    setBrightness(100);
    setContrast(100);
    setSaturation(100);
    setRotation(0);
    setFlipH(false);
    setFlipV(false);
    setCrop(null);
    setShowCrop(false);
    setAnnotations([]);
    setAnnotTool(null);
  };

  /* ─── actions ─── */
  const openEditor = useCallback((id: string) => {
    setSelectedId(id);
    setView("editor");
    resetEditor();
    logAudit(`Opened editor: ${assets.find(a => a.id === id)?.name}`);
  }, [assets, logAudit]);

  const handleExport = useCallback((format: string) => {
    if (!selectedAsset) return;
    // Real export: render the filtered image to a canvas and trigger download
    const canvas = document.createElement("canvas");
    canvas.width = selectedAsset.width;
    canvas.height = selectedAsset.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    // Apply CSS filter to canvas context
    const filterStr = getFilterCSS();
    ctx.filter = filterStr === "none" ? "none" : filterStr;
    // Draw the real image if we have one, otherwise a gradient placeholder
    const dataUrl = imageDataUrls[selectedAsset.id];
    if (dataUrl) {
      const img = new Image();
      img.onload = () => {
        ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
        doDownload();
      };
      img.src = dataUrl;
    } else {
      // Gradient placeholder
      ctx.filter = "none";
      const grad = ctx.createLinearGradient(0, 0, canvas.width, canvas.height);
      grad.addColorStop(0, "#0f172a");
      grad.addColorStop(0.5, "var(--nexus-accent)");
      grad.addColorStop(1, "#06b6d4");
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      doDownload();
    }

    function doDownload() {
      const mimeMap: Record<string, string> = {
        PNG: "image/png", JPEG: "image/jpeg", WebP: "image/webp",
      };
      const mime = mimeMap[format] ?? "image/png";
      const ext = format.toLowerCase();
      try {
        const exportUrl = canvas.toDataURL(mime, 0.92);
        const link = document.createElement("a");
        link.download = `${selectedAsset!.name.replace(/\.[^.]+$/, "")}-export.${ext}`;
        link.href = exportUrl;
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
        logAudit(`Exported ${selectedAsset!.name} as ${format}`);
      } catch {
        logAudit(`Export failed for ${format} — try PNG`);
      }
    }
  }, [selectedAsset, imageDataUrls, logAudit]);

  const handleGenerate = useCallback(() => {
    if (!genPrompt.trim()) return;
    setGenerating(true);
    setFuelUsed(f => f + 15);
    logAudit(`AI generating: "${genPrompt.slice(0, 40)}..." [${genStyle}]`);
    setTimeout(() => {
      const asset: MediaAsset = {
        id: `ms-${Date.now()}`,
        name: `ai-generated-${Date.now().toString(36)}.png`,
        type: "image",
        size: Math.floor(300000 + Math.random() * 400000),
        width: parseInt(genSize.split("x")[0]),
        height: parseInt(genSize.split("x")[1]),
        folder: "Generated",
        tags: ["ai", "generated", genStyle.toLowerCase()],
        createdAt: Date.now(),
        modifiedAt: Date.now(),
        thumbnail: `linear-gradient(${Math.floor(Math.random() * 360)}deg, #0f172a 0%, #${Math.floor(Math.random() * 16777215).toString(16).padStart(6, "0")} 50%, var(--nexus-accent) 100%)`,
        agent: "Designer Agent",
      };
      setAssets(prev => [asset, ...prev]);
      setGenerating(false);
      logAudit(`Generated: ${asset.name}`);
    }, 2500);
  }, [genPrompt, genStyle, genSize, logAudit]);

  const addAnnotation = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (!annotTool) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 100;
    const y = ((e.clientY - rect.top) / rect.height) * 100;
    const ann: Annotation = {
      id: `ann-${Date.now()}`,
      type: annotTool,
      x, y,
      w: annotTool === "text" ? 20 : 15,
      h: annotTool === "text" ? 5 : 10,
      color: annotColor,
      label: annotTool === "text" ? "Text" : undefined,
    };
    setAnnotations(prev => [...prev, ann]);
    logAudit(`Added ${annotTool} annotation`);
  }, [annotTool, annotColor, logAudit]);

  const handleCompareSlider = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    if (!sliderRef.current) return;
    const rect = sliderRef.current.getBoundingClientRect();
    const pos = ((e.clientX - rect.left) / rect.width) * 100;
    setSliderPos(Math.max(0, Math.min(100, pos)));
  }, []);

  const handleFileUpload = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      const img = new Image();
      img.onload = () => {
        const asset: MediaAsset = {
          id: `ms-${Date.now()}`,
          name: file.name,
          type: file.type.includes("svg") ? "svg" : file.type.includes("gif") ? "gif" : "image",
          size: file.size,
          width: img.naturalWidth || 800,
          height: img.naturalHeight || 600,
          folder: "Uploads",
          tags: ["uploaded"],
          createdAt: Date.now(),
          modifiedAt: Date.now(),
          thumbnail: `linear-gradient(135deg, #0f172a 0%, #334155 50%, var(--nexus-accent) 100%)`,
        };
        setAssets(prev => [asset, ...prev]);
        setImageDataUrls(prev => ({ ...prev, [asset.id]: dataUrl }));
        logAudit(`Uploaded: ${file.name}`);
      };
      img.src = dataUrl;
    };
    reader.readAsDataURL(file);
    // Reset input
    e.target.value = "";
  }, [logAudit]);

  const deleteAsset = useCallback((id: string) => {
    const a = assets.find(a => a.id === id);
    setAssets(prev => prev.filter(a => a.id !== id));
    if (selectedId === id) setSelectedId(null);
    logAudit(`Deleted: ${a?.name}`);
  }, [assets, selectedId, logAudit]);

  /* ─── render ─── */
  return (
    <div className="ms-container">
      {/* ─── Sidebar ─── */}
      <aside className="ms-sidebar">
        <div className="ms-sidebar-header">
          <h2 className="ms-sidebar-title">Media Studio</h2>
        </div>

        {/* views */}
        <div className="ms-views">
          {([["library", "📁", "Library"], ["editor", "🖌", "Editor"], ["generate", "✦", "AI Generate"], ["compare", "⇔", "Compare"]] as const).map(([id, icon, label]) => (
            <button key={id} className={`ms-view-btn ${view === id ? "active" : ""}`} onClick={() => setView(id as View)}>
              <span className="ms-view-icon">{icon}</span>{label}
            </button>
          ))}
          <button className="ms-view-btn" disabled style={{ opacity: 0.4, cursor: "default" }}>
            <span className="ms-view-icon">📖</span>OCR — Coming Soon
          </button>
        </div>

        {/* folders (library view) */}
        {view === "library" && (
          <div className="ms-folders">
            <div className="ms-section-header">Folders</div>
            {FOLDERS.map(f => (
              <button key={f} className={`ms-folder-btn ${selectedFolder === f ? "active" : ""}`} onClick={() => setSelectedFolder(f)}>
                {f}
                <span className="ms-folder-count">{f === "All" ? assets.length : assets.filter(a => a.folder === f).length}</span>
              </button>
            ))}
          </div>
        )}

        {/* asset details */}
        {selectedAsset && view === "library" && (
          <div className="ms-detail-panel">
            <div className="ms-section-header">Details</div>
            <div className="ms-detail-thumb" style={{ background: selectedAsset.thumbnail }} />
            <div className="ms-detail-name">{selectedAsset.name}</div>
            <div className="ms-detail-meta">{selectedAsset.width} × {selectedAsset.height} · {formatSize(selectedAsset.size)}</div>
            <div className="ms-detail-meta">{selectedAsset.type.toUpperCase()} · {formatDate(selectedAsset.modifiedAt)}</div>
            {selectedAsset.agent && <div className="ms-detail-agent">⬢ {selectedAsset.agent}</div>}
            <div className="ms-detail-tags">
              {selectedAsset.tags.map(t => <span key={t} className="ms-tag">{t}</span>)}
            </div>
            <div className="ms-detail-actions">
              <button className="ms-sm-btn" onClick={() => openEditor(selectedAsset.id)}>Edit</button>
              <button className="ms-sm-btn ms-btn-danger" onClick={() => deleteAsset(selectedAsset.id)}>Delete</button>
            </div>
          </div>
        )}

        {/* audit */}
        <div className="ms-audit">
          <div className="ms-section-header">Activity</div>
          {auditLog.slice(0, 5).map((msg, i) => (
            <div key={i} className="ms-audit-entry">{msg}</div>
          ))}
        </div>
      </aside>

      {/* ─── Main Content ─── */}
      <div className="ms-main">
        {/* ═══ LIBRARY VIEW ═══ */}
        {view === "library" && (
          <div className="ms-library">
            <div className="ms-lib-toolbar">
              <input className="ms-search" placeholder="Search assets..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />
              <button className="ms-sm-btn" onClick={() => fileInputRef.current?.click()}>Upload Image</button>
              <input ref={fileInputRef} type="file" accept="image/*" style={{ display: "none" }} onChange={handleFileUpload} />
              <select className="ms-select" value={sortBy} onChange={e => setSortBy(e.target.value as typeof sortBy)}>
                <option value="date">Date</option>
                <option value="name">Name</option>
                <option value="size">Size</option>
              </select>
              <div className="ms-grid-toggle">
                {(["sm", "md", "lg"] as const).map(s => (
                  <button key={s} className={`ms-grid-btn ${gridSize === s ? "active" : ""}`} onClick={() => setGridSize(s)}>
                    {s === "sm" ? "▪" : s === "md" ? "◼" : "⬛"}
                  </button>
                ))}
              </div>
            </div>
            <div className={`ms-grid ms-grid-${gridSize}`}>
              {filteredAssets.map(asset => (
                <div key={asset.id} className={`ms-card ${selectedId === asset.id ? "selected" : ""}`} onClick={() => setSelectedId(asset.id)} onDoubleClick={() => openEditor(asset.id)}>
                  <div className="ms-card-thumb" style={{ background: asset.thumbnail }}>
                    {asset.agent && <span className="ms-card-agent">⬢</span>}
                    <span className="ms-card-type">{asset.type.toUpperCase()}</span>
                  </div>
                  <div className="ms-card-info">
                    <div className="ms-card-name">{asset.name}</div>
                    <div className="ms-card-meta">{asset.width}×{asset.height} · {formatSize(asset.size)}</div>
                  </div>
                </div>
              ))}
              {filteredAssets.length === 0 && (
                <div className="ms-empty">
                  <div className="ms-empty-icon">🖼</div>
                  <div>No media files</div>
                </div>
              )}
            </div>
          </div>
        )}

        {/* ═══ EDITOR VIEW ═══ */}
        {view === "editor" && (
          <div className="ms-editor">
            {selectedAsset ? (
              <>
                <div className="ms-editor-toolbar">
                  <span className="ms-editor-name">{selectedAsset.name}</span>
                  <div className="ms-editor-btns">
                    <button className={`ms-tool-btn ${showCrop ? "active" : ""}`} onClick={() => { setShowCrop(!showCrop); setCrop(showCrop ? null : { x: 10, y: 10, w: 80, h: 80 }); }}>✂ Crop</button>
                    <button className="ms-tool-btn" onClick={() => setRotation(r => (r + 90) % 360)}>↻ Rotate</button>
                    <button className={`ms-tool-btn ${flipH ? "active" : ""}`} onClick={() => setFlipH(!flipH)}>⇆ Flip H</button>
                    <button className={`ms-tool-btn ${flipV ? "active" : ""}`} onClick={() => setFlipV(!flipV)}>⇅ Flip V</button>
                    <span className="ms-tool-sep">|</span>
                    {(["rect", "circle", "arrow", "text"] as const).map(t => (
                      <button key={t} className={`ms-tool-btn ${annotTool === t ? "active" : ""}`} onClick={() => setAnnotTool(annotTool === t ? null : t)}>
                        {t === "rect" ? "▭" : t === "circle" ? "○" : t === "arrow" ? "→" : "T"} {t}
                      </button>
                    ))}
                    <input type="color" className="ms-color-pick" value={annotColor} onChange={e => setAnnotColor(e.target.value)} title="Annotation color" />
                    <span className="ms-tool-sep">|</span>
                    <button className="ms-tool-btn" onClick={resetEditor}>Reset</button>
                  </div>
                </div>

                <div className="ms-editor-body">
                  {/* canvas */}
                  <div className="ms-canvas-wrap">
                    <div className="ms-canvas" onClick={addAnnotation} style={{
                      background: imageDataUrls[selectedAsset.id] ? "transparent" : selectedAsset.thumbnail,
                      filter: getFilterCSS(),
                      transform: getTransformCSS(),
                    }}>
                      {imageDataUrls[selectedAsset.id] && (
                        <img src={imageDataUrls[selectedAsset.id]} alt={selectedAsset.name} style={{ width: "100%", height: "100%", objectFit: "contain", position: "absolute", top: 0, left: 0 }} />
                      )}
                      <div className="ms-canvas-label">{selectedAsset.width} × {selectedAsset.height}</div>
                      {/* crop overlay */}
                      {showCrop && crop && (
                        <div className="ms-crop-overlay">
                          <div className="ms-crop-box" style={{ left: `${crop.x}%`, top: `${crop.y}%`, width: `${crop.w}%`, height: `${crop.h}%` }}>
                            <div className="ms-crop-handle ms-crop-tl" />
                            <div className="ms-crop-handle ms-crop-tr" />
                            <div className="ms-crop-handle ms-crop-bl" />
                            <div className="ms-crop-handle ms-crop-br" />
                          </div>
                        </div>
                      )}
                      {/* annotations */}
                      {annotations.map(ann => (
                        <div key={ann.id} className={`ms-annotation ms-ann-${ann.type}`} style={{
                          left: `${ann.x}%`, top: `${ann.y}%`,
                          width: `${ann.w}%`, height: `${ann.h}%`,
                          borderColor: ann.color,
                          color: ann.color,
                        }}>
                          {ann.type === "text" && <span className="ms-ann-text">{ann.label}</span>}
                          {ann.type === "arrow" && <span className="ms-ann-arrow">→</span>}
                        </div>
                      ))}
                    </div>
                  </div>

                  {/* side panel */}
                  <div className="ms-editor-panel">
                    {/* filters */}
                    <div className="ms-section-header">Filters</div>
                    <div className="ms-filters">
                      {FILTERS.map(f => (
                        <button key={f.id} className={`ms-filter-btn ${activeFilter === f.id ? "active" : ""}`} onClick={() => setActiveFilter(f.id)}>
                          <div className="ms-filter-preview" style={{ background: selectedAsset.thumbnail, filter: f.css === "none" ? "none" : f.css }} />
                          <span>{f.label}</span>
                        </button>
                      ))}
                    </div>

                    {/* adjustments */}
                    <div className="ms-section-header">Adjustments</div>
                    <div className="ms-adjustments">
                      <label className="ms-adj-label">
                        Brightness <span>{brightness}%</span>
                        <input type="range" min="0" max="200" value={brightness} onChange={e => setBrightness(Number(e.target.value))} />
                      </label>
                      <label className="ms-adj-label">
                        Contrast <span>{contrast}%</span>
                        <input type="range" min="0" max="200" value={contrast} onChange={e => setContrast(Number(e.target.value))} />
                      </label>
                      <label className="ms-adj-label">
                        Saturation <span>{saturation}%</span>
                        <input type="range" min="0" max="200" value={saturation} onChange={e => setSaturation(Number(e.target.value))} />
                      </label>
                    </div>

                    {/* crop inputs */}
                    {showCrop && crop && (
                      <>
                        <div className="ms-section-header">Crop</div>
                        <div className="ms-crop-inputs">
                          <label>X <input type="number" value={crop.x} onChange={e => setCrop({ ...crop, x: Number(e.target.value) })} /></label>
                          <label>Y <input type="number" value={crop.y} onChange={e => setCrop({ ...crop, y: Number(e.target.value) })} /></label>
                          <label>W <input type="number" value={crop.w} onChange={e => setCrop({ ...crop, w: Number(e.target.value) })} /></label>
                          <label>H <input type="number" value={crop.h} onChange={e => setCrop({ ...crop, h: Number(e.target.value) })} /></label>
                        </div>
                        <button className="ms-sm-btn ms-btn-crop" onClick={() => { setShowCrop(false); setCrop(null); logAudit("Crop applied"); }}>Apply Crop</button>
                      </>
                    )}

                    {/* export */}
                    <div className="ms-section-header">Export (real download)</div>
                    <div className="ms-export-btns">
                      {EXPORT_FORMATS.map(f => (
                        <button key={f} className="ms-export-btn" onClick={() => handleExport(f)}>{f}</button>
                      ))}
                    </div>

                    {annotations.length > 0 && (
                      <>
                        <div className="ms-section-header">Annotations ({annotations.length})</div>
                        <button className="ms-sm-btn ms-btn-danger" onClick={() => { setAnnotations([]); logAudit("Cleared annotations"); }}>Clear All</button>
                      </>
                    )}
                  </div>
                </div>
              </>
            ) : (
              <div className="ms-empty-view">
                <div className="ms-empty-icon">🖌</div>
                <div>Select an image from the library to edit</div>
                <button className="ms-sm-btn" onClick={() => setView("library")}>Open Library</button>
              </div>
            )}
          </div>
        )}

        {/* ═══ AI GENERATE VIEW ═══ */}
        {view === "generate" && (
          <div className="ms-generate">
            <div className="ms-gen-header">
              <h3 className="ms-gen-title">✦ AI Image Generation</h3>
              <span className="ms-gen-fuel">⚡ 15 fuel per generation</span>
            </div>

            <div className="ms-gen-body">
              <div className="ms-gen-form">
                <label className="ms-gen-label">Prompt</label>
                <textarea className="ms-gen-prompt" value={genPrompt} onChange={e => setGenPrompt(e.target.value)} placeholder="Describe the image you want to generate..." rows={4} />

                <label className="ms-gen-label">Style</label>
                <div className="ms-gen-styles">
                  {AI_STYLES.map(s => (
                    <button key={s} className={`ms-style-btn ${genStyle === s ? "active" : ""}`} onClick={() => setGenStyle(s)}>{s}</button>
                  ))}
                </div>

                <label className="ms-gen-label">Size</label>
                <div className="ms-gen-sizes">
                  {["512x512", "1024x1024", "1024x768", "768x1024", "2048x1024"].map(s => (
                    <button key={s} className={`ms-size-btn ${genSize === s ? "active" : ""}`} onClick={() => setGenSize(s)}>{s}</button>
                  ))}
                </div>

                <button className="ms-gen-btn" onClick={handleGenerate} disabled={!genPrompt.trim() || generating}>
                  {generating ? "Generating..." : "✦ Generate Image"}
                </button>
              </div>

              <div className="ms-gen-preview">
                <div className="ms-gen-preview-label">Recent Generations</div>
                <div className="ms-gen-grid">
                  {assets.filter(a => a.folder === "Generated").slice(0, 6).map(a => (
                    <div key={a.id} className="ms-gen-card" onClick={() => { setSelectedId(a.id); setView("editor"); }}>
                      <div className="ms-gen-thumb" style={{ background: a.thumbnail }} />
                      <div className="ms-gen-name">{a.name}</div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        )}

        {/* ═══ COMPARE VIEW ═══ */}
        {view === "compare" && (
          <div className="ms-compare">
            <div className="ms-cmp-header">
              <h3 className="ms-cmp-title">⇔ Before / After Comparison</h3>
            </div>
            <div className="ms-cmp-selectors">
              <div className="ms-cmp-select">
                <label>Image A (Before)</label>
                <select value={compareA ?? ""} onChange={e => setCompareA(e.target.value || null)}>
                  <option value="">Select image...</option>
                  {assets.map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
                </select>
              </div>
              <div className="ms-cmp-select">
                <label>Image B (After)</label>
                <select value={compareB ?? ""} onChange={e => setCompareB(e.target.value || null)}>
                  <option value="">Select image...</option>
                  {assets.map(a => <option key={a.id} value={a.id}>{a.name}</option>)}
                </select>
              </div>
            </div>
            {compareA && compareB ? (
              <div className="ms-cmp-viewer" ref={sliderRef} onMouseMove={e => { if (e.buttons === 1) handleCompareSlider(e); }} onClick={handleCompareSlider}>
                <div className="ms-cmp-img ms-cmp-a" style={{ background: assets.find(a => a.id === compareA)?.thumbnail, clipPath: `inset(0 ${100 - sliderPos}% 0 0)` }}>
                  <span className="ms-cmp-label-tag">A — Before</span>
                </div>
                <div className="ms-cmp-img ms-cmp-b" style={{ background: assets.find(a => a.id === compareB)?.thumbnail }}>
                  <span className="ms-cmp-label-tag ms-cmp-tag-right">B — After</span>
                </div>
                <div className="ms-cmp-slider" style={{ left: `${sliderPos}%` }}>
                  <div className="ms-cmp-handle">⇔</div>
                </div>
              </div>
            ) : (
              <div className="ms-empty-view">
                <div className="ms-empty-icon">⇔</div>
                <div>Select two images to compare</div>
              </div>
            )}
          </div>
        )}

      </div>

      {/* ─── Status Bar ─── */}
      <div className="ms-status-bar">
        <span className="ms-status-item">{view.charAt(0).toUpperCase() + view.slice(1)}</span>
        <span className="ms-status-item">{assets.length} assets</span>
        <span className="ms-status-item">{formatSize(assets.reduce((s, a) => s + a.size, 0))} total</span>
        {selectedAsset && <span className="ms-status-item">{selectedAsset.name}</span>}
        <span className="ms-status-item ms-status-right">⚡ {fuelUsed} fuel</span>
        <span className="ms-status-item">{assets.filter(a => a.agent).length} agent-generated</span>
      </div>
    </div>
  );
}
