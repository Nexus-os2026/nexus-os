import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { hasDesktopRuntime, projectList, projectSave, projectDelete as projectDeleteApi } from "../api/backend";
import {
  LayoutGrid, List, Clock, PieChart, Plus, Circle, CircleDot,
  Target, CheckCircle2, Zap, Hexagon, GitCommit, GitBranch,
  ExternalLink, FileText, Workflow, CheckSquare, Square,
  AlertCircle, ArrowUpCircle, MinusCircle,
} from "lucide-react";
import "./project-manager.css";

/* ─── types ─── */
type Priority = "critical" | "high" | "medium" | "low";
type ColumnId = "backlog" | "todo" | "in-progress" | "review" | "done";
type ViewMode = "board" | "list" | "timeline" | "metrics";

interface TaskTag {
  id: string;
  name: string;
  color: string;
}

interface TaskLink {
  type: "commit" | "branch" | "pr" | "note" | "workflow";
  label: string;
  id: string;
}

interface Task {
  id: string;
  title: string;
  description: string;
  column: ColumnId;
  priority: Priority;
  assignee: string;
  tags: string[];
  links: TaskLink[];
  complexity: number;
  timeSpent: number;
  fuelCost: number;
  createdAt: number;
  updatedAt: number;
  createdBy: string;
  sprintId: string;
  dueDate?: number;
  subtasks: { label: string; done: boolean }[];
}

interface Sprint {
  id: string;
  name: string;
  startDate: number;
  endDate: number;
  goal: string;
  velocity: number;
  totalPoints: number;
  completedPoints: number;
  active: boolean;
}

interface Automation {
  id: string;
  trigger: string;
  action: string;
  enabled: boolean;
  agent: string;
  lastRun?: number;
}

interface ProjectData {
  tasks: Task[];
  sprints: Sprint[];
  automations: Automation[];
  savedAt: number;
}

/* ─── constants ─── */
const COLUMNS: { id: ColumnId; label: string; color: string; icon: React.ReactNode }[] = [
  { id: "backlog", label: "Backlog", color: "#64748b", icon: <Circle size={12} /> },
  { id: "todo", label: "To Do", color: "#38bdf8", icon: <Circle size={12} /> },
  { id: "in-progress", label: "In Progress", color: "#a78bfa", icon: <CircleDot size={12} /> },
  { id: "review", label: "Review", color: "#fbbf24", icon: <Target size={12} /> },
  { id: "done", label: "Done", color: "#34d399", icon: <CheckCircle2 size={12} /> },
];

const PRIORITY_MAP: Record<Priority, { label: string; color: string; icon: React.ReactNode }> = {
  critical: { label: "Critical", color: "#ef4444", icon: <AlertCircle size={12} /> },
  high: { label: "High", color: "#f97316", icon: <ArrowUpCircle size={12} /> },
  medium: { label: "Medium", color: "#fbbf24", icon: <MinusCircle size={12} /> },
  low: { label: "Low", color: "var(--nexus-accent)", icon: <Circle size={12} /> },
};

const TAGS: TaskTag[] = [
  { id: "tg-feat", name: "feature", color: "#a78bfa" },
  { id: "tg-bug", name: "bug", color: "#f87171" },
  { id: "tg-refactor", name: "refactor", color: "#38bdf8" },
  { id: "tg-docs", name: "docs", color: "#34d399" },
  { id: "tg-infra", name: "infra", color: "#fbbf24" },
  { id: "tg-agent", name: "agent-task", color: "#818cf8" },
  { id: "tg-perf", name: "performance", color: "#fb923c" },
];

const ASSIGNEES = ["You", "Coder Agent", "Research Agent", "Planner Agent", "Self-Improve Agent", "Content Agent", "Unassigned"];

const PROJECT_ID = "default";

const DEFAULT_SPRINTS: Sprint[] = [
  { id: "sp-1", name: "Sprint 1", startDate: Date.now() - 86400000 * 10, endDate: Date.now() + 86400000 * 4, goal: "Initial setup", velocity: 0, totalPoints: 0, completedPoints: 0, active: true },
];

const DEFAULT_AUTOMATIONS: Automation[] = [
  { id: "auto-1", trigger: "Task moved to In Progress", action: "Notify Coder Agent, create branch", enabled: true, agent: "Coder Agent" },
  { id: "auto-2", trigger: "Task moved to Review", action: "Run tests, request code review", enabled: true, agent: "Self-Improve Agent" },
  { id: "auto-3", trigger: "Task moved to Done", action: "Update audit trail, close PR", enabled: true, agent: "Planner Agent" },
  { id: "auto-4", trigger: "Critical bug created", action: "Alert team, escalate priority", enabled: true, agent: "Planner Agent" },
  { id: "auto-5", trigger: "Sprint ends", action: "Generate velocity report, archive sprint", enabled: false, agent: "Content Agent" },
];

/* ─── persistence ─── */
async function loadProject(): Promise<ProjectData | null> {
  if (!hasDesktopRuntime()) return null;
  try {
    const raw = await projectList();
    const projects = JSON.parse(raw) as ProjectData[];
    return projects.length > 0 ? projects[0] : null;
  } catch {
    return null;
  }
}

async function saveProject(data: ProjectData): Promise<void> {
  if (!hasDesktopRuntime()) return;
  try {
    await projectSave(PROJECT_ID, JSON.stringify({ ...data, savedAt: Date.now() }));
  } catch (e) {
    if (import.meta.env.DEV) console.error("Failed to save project:", e);
  }
}

/* ─── component ─── */
export default function ProjectManager() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [sprints, setSprints] = useState<Sprint[]>(DEFAULT_SPRINTS);
  const [automations, setAutomations] = useState<Automation[]>(DEFAULT_AUTOMATIONS);
  const [loaded, setLoaded] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>("board");
  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [filterAssignee, setFilterAssignee] = useState<string>("All");
  const [filterPriority, setFilterPriority] = useState<string>("All");
  const [filterTag, setFilterTag] = useState<string>("All");
  const [searchQuery, setSearchQuery] = useState("");
  const [dragTaskId, setDragTaskId] = useState<string | null>(null);
  const [showNewTask, setShowNewTask] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState("");
  const [newTaskColumn, setNewTaskColumn] = useState<ColumnId>("todo");
  const [auditLog, setAuditLog] = useState<string[]>([]);

  // Ref to persist on changes (debounced)
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const tasksRef = useRef(tasks);
  tasksRef.current = tasks;
  const sprintsRef = useRef(sprints);
  sprintsRef.current = sprints;
  const automationsRef = useRef(automations);
  automationsRef.current = automations;

  const scheduleSave = useCallback(() => {
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => {
      saveProject({
        tasks: tasksRef.current,
        sprints: sprintsRef.current,
        automations: automationsRef.current,
        savedAt: Date.now(),
      });
    }, 500);
  }, []);

  const activeSprint = useMemo(() => sprints.find(s => s.active) ?? sprints[0], [sprints]);

  useEffect(() => {
    return () => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
    };
  }, []);

  /* ─── load on mount ─── */
  useEffect(() => {
    loadProject().then(data => {
      if (data) {
        if (data.tasks) setTasks(data.tasks);
        if (data.sprints?.length) setSprints(data.sprints);
        if (data.automations?.length) setAutomations(data.automations);
        logAudit(`Loaded project from disk (${data.tasks?.length ?? 0} tasks)`);
      }
      setLoaded(true);
    });
  }, []);

  /* ─── filtering ─── */
  const filteredTasks = useMemo(() => {
    let list = tasks;
    if (filterAssignee !== "All") list = list.filter(t => t.assignee === filterAssignee);
    if (filterPriority !== "All") list = list.filter(t => t.priority === filterPriority);
    if (filterTag !== "All") list = list.filter(t => t.tags.includes(filterTag));
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(t => t.title.toLowerCase().includes(q) || t.description.toLowerCase().includes(q));
    }
    return list;
  }, [tasks, filterAssignee, filterPriority, filterTag, searchQuery]);

  const getColumnTasks = useCallback((colId: ColumnId) => filteredTasks.filter(t => t.column === colId), [filteredTasks]);

  /* ─── handlers ─── */
  const moveTask = useCallback((taskId: string, toColumn: ColumnId) => {
    setTasks(prev => {
      const next = prev.map(t => {
        if (t.id !== taskId) return t;
        const col = COLUMNS.find(c => c.id === toColumn);
        setAuditLog(a => [`Task moved: ${t.title} → ${col?.label}`, ...a].slice(0, 20));
        return { ...t, column: toColumn, updatedAt: Date.now() };
      });
      return next;
    });
    scheduleSave();
  }, [scheduleSave]);

  const createTask = useCallback(() => {
    if (!newTaskTitle.trim()) return;
    const task: Task = {
      id: `task-${Date.now()}`, title: newTaskTitle, description: "", column: newTaskColumn,
      priority: "medium", assignee: "Unassigned", tags: [], links: [], complexity: 1,
      timeSpent: 0, fuelCost: 0, createdAt: Date.now(), updatedAt: Date.now(),
      createdBy: "You", sprintId: activeSprint.id, subtasks: [],
    };
    setTasks(prev => [...prev, task]);
    setNewTaskTitle("");
    setShowNewTask(false);
    setAuditLog(a => [`Task created: ${task.title}`, ...a].slice(0, 20));
    scheduleSave();
  }, [newTaskTitle, newTaskColumn, activeSprint.id, scheduleSave]);

  const deleteTask = useCallback((id: string) => {
    const task = tasks.find(t => t.id === id);
    setTasks(prev => prev.filter(t => t.id !== id));
    if (selectedTask?.id === id) setSelectedTask(null);
    if (task) setAuditLog(a => [`Task deleted: ${task.title}`, ...a].slice(0, 20));
    scheduleSave();
  }, [tasks, selectedTask, scheduleSave]);

  const updateTask = useCallback((id: string, updates: Partial<Task>) => {
    setTasks(prev => prev.map(t => t.id === id ? { ...t, ...updates, updatedAt: Date.now() } : t));
    scheduleSave();
  }, [scheduleSave]);

  const toggleSubtask = useCallback((taskId: string, idx: number) => {
    setTasks(prev => prev.map(t => {
      if (t.id !== taskId) return t;
      const subs = [...t.subtasks];
      subs[idx] = { ...subs[idx], done: !subs[idx].done };
      return { ...t, subtasks: subs, updatedAt: Date.now() };
    }));
    scheduleSave();
  }, [scheduleSave]);

  const toggleAutomation = useCallback((id: string) => {
    setAutomations(prev => prev.map(a => a.id === id ? { ...a, enabled: !a.enabled } : a));
    scheduleSave();
  }, [scheduleSave]);

  /* ─── drag & drop ─── */
  const handleDragStart = (taskId: string) => setDragTaskId(taskId);
  const handleDragOver = (e: React.DragEvent) => e.preventDefault();
  const handleDrop = (colId: ColumnId) => {
    if (dragTaskId) { moveTask(dragTaskId, colId); setDragTaskId(null); }
  };

  /* ─── stats ─── */
  const stats = useMemo(() => {
    const sprintTasks = tasks.filter(t => t.sprintId === activeSprint.id);
    const done = sprintTasks.filter(t => t.column === "done");
    const inProgress = sprintTasks.filter(t => t.column === "in-progress");
    const totalPoints = sprintTasks.reduce((s, t) => s + t.complexity, 0);
    const donePoints = done.reduce((s, t) => s + t.complexity, 0);
    const totalFuel = sprintTasks.reduce((s, t) => s + t.fuelCost, 0);
    const totalTime = sprintTasks.reduce((s, t) => s + t.timeSpent, 0);
    const daysLeft = Math.max(0, Math.ceil((activeSprint.endDate - Date.now()) / 86400000));
    return { total: sprintTasks.length, done: done.length, inProgress: inProgress.length, totalPoints, donePoints, totalFuel, totalTime, daysLeft };
  }, [tasks, activeSprint]);

  const formatTime = (mins: number) => {
    if (mins < 60) return `${mins}m`;
    return `${Math.floor(mins / 60)}h ${mins % 60}m`;
  };

  const formatDate = (ts: number) => new Date(ts).toLocaleDateString();

  const logAudit = (msg: string) => setAuditLog(prev => [msg, ...prev].slice(0, 20));

  /* ─── render ─── */
  return (
    <div className="pm-container">
      {/* ─── Header ─── */}
      <div className="pm-header">
        <div className="pm-header-left">
          <h2 className="pm-title">Project Manager</h2>
          <div className="pm-sprint-badge">
            <span className="pm-sprint-dot" />
            {activeSprint.name} — {stats.daysLeft}d left
          </div>
          <span className="pm-sprint-goal">{activeSprint.goal}</span>
        </div>
        <div className="pm-header-right">
          <div className="pm-view-toggle">
            {(["board", "list", "timeline", "metrics"] as ViewMode[]).map(v => (
              <button type="button" key={v} className={`pm-view-btn cursor-pointer ${viewMode === v ? "active" : ""}`} onClick={() => setViewMode(v)}>
                {v === "board" ? <LayoutGrid size={14} /> : v === "list" ? <List size={14} /> : v === "timeline" ? <Clock size={14} /> : <PieChart size={14} />} {v.charAt(0).toUpperCase() + v.slice(1)}
              </button>
            ))}
          </div>
          <button type="button" className="pm-btn-new cursor-pointer" onClick={() => setShowNewTask(true)}><Plus size={14} style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} />New Task</button>
        </div>
      </div>

      {/* ─── Filters ─── */}
      <div className="pm-filters">
        <div className="pm-filter-group">
          <input className="pm-search" placeholder="Search tasks..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />
          <select className="pm-filter-select" value={filterAssignee} onChange={e => setFilterAssignee(e.target.value)}>
            <option value="All">All assignees</option>
            {ASSIGNEES.map(a => <option key={a} value={a}>{a}</option>)}
          </select>
          <select className="pm-filter-select" value={filterPriority} onChange={e => setFilterPriority(e.target.value)}>
            <option value="All">All priorities</option>
            {(["critical", "high", "medium", "low"] as Priority[]).map(p => <option key={p} value={p}>{PRIORITY_MAP[p].label}</option>)}
          </select>
          <select className="pm-filter-select" value={filterTag} onChange={e => setFilterTag(e.target.value)}>
            <option value="All">All tags</option>
            {TAGS.map(t => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </div>
        <div className="pm-stats-strip">
          <span className="pm-stat">{stats.total} tasks</span>
          <span className="pm-stat">{stats.donePoints}/{stats.totalPoints} pts</span>
          <span className="pm-stat"><Clock size={10} style={{ display: "inline", verticalAlign: "middle", marginRight: 2 }} /> {formatTime(stats.totalTime)}</span>
          {!loaded && <span className="pm-stat">Loading...</span>}
        </div>
      </div>

      {/* ─── New Task Modal ─── */}
      {showNewTask && (
        <div className="pm-modal-overlay" onClick={() => setShowNewTask(false)}>
          <div className="pm-modal" onClick={e => e.stopPropagation()}>
            <div className="pm-modal-header">
              <span>New Task</span>
              <button type="button" className="pm-modal-close" onClick={() => setShowNewTask(false)}>×</button>
            </div>
            <div className="pm-modal-body">
              <input className="pm-modal-input" placeholder="Task title..." value={newTaskTitle} onChange={e => setNewTaskTitle(e.target.value)} autoFocus onKeyDown={e => e.key === "Enter" && createTask()} />
              <select className="pm-modal-select" value={newTaskColumn} onChange={e => setNewTaskColumn(e.target.value as ColumnId)}>
                {COLUMNS.map(c => <option key={c.id} value={c.id}>{c.label}</option>)}
              </select>
              <button type="button" className="pm-btn-create" onClick={createTask} disabled={!newTaskTitle.trim()}>Create Task</button>
            </div>
          </div>
        </div>
      )}

      {/* ─── Board View ─── */}
      {viewMode === "board" && (
        <div className="pm-board">
          {COLUMNS.map(col => {
            const colTasks = getColumnTasks(col.id);
            const colPoints = colTasks.reduce((s, t) => s + t.complexity, 0);
            return (
              <div key={col.id} className="pm-column" onDragOver={handleDragOver} onDrop={() => handleDrop(col.id)}>
                <div className="pm-column-header" style={{ borderBottomColor: col.color }}>
                  <span className="pm-column-icon" style={{ color: col.color }}>{col.icon}</span>
                  <span className="pm-column-label">{col.label}</span>
                  <span className="pm-column-count">{colTasks.length}</span>
                  <span className="pm-column-points">{colPoints} pts</span>
                </div>
                <div className="pm-column-body">
                  {colTasks.map(task => (
                    <div
                      key={task.id}
                      className={`pm-card ${selectedTask?.id === task.id ? "selected" : ""}`}
                      draggable
                      onDragStart={() => handleDragStart(task.id)}
                      onClick={() => setSelectedTask(task)}
                    >
                      <div className="pm-card-top">
                        <span className="pm-card-priority" style={{ background: PRIORITY_MAP[task.priority].color }}>{PRIORITY_MAP[task.priority].icon}</span>
                        <span className="pm-card-id">{task.id.replace("task-", "#")}</span>
                        <span className="pm-card-complexity">{task.complexity}pt</span>
                      </div>
                      <div className="pm-card-title">{task.title}</div>
                      {task.subtasks.length > 0 && (
                        <div className="pm-card-progress">
                          <div className="pm-card-progress-bar">
                            <div className="pm-card-progress-fill" style={{ width: `${(task.subtasks.filter(s => s.done).length / task.subtasks.length) * 100}%` }} />
                          </div>
                          <span className="pm-card-progress-text">{task.subtasks.filter(s => s.done).length}/{task.subtasks.length}</span>
                        </div>
                      )}
                      <div className="pm-card-bottom">
                        <div className="pm-card-tags">
                          {task.tags.map(tId => {
                            const tag = TAGS.find(t => t.id === tId);
                            return tag ? <span key={tId} className="pm-card-tag" style={{ background: tag.color + "22", color: tag.color }}>{tag.name}</span> : null;
                          })}
                        </div>
                        <span className="pm-card-assignee">{task.assignee === "Unassigned" ? "—" : task.assignee.replace(" Agent", "")}</span>
                      </div>
                      {task.links.length > 0 && (
                        <div className="pm-card-links">
                          {task.links.map((l, i) => (
                            <span key={i} className={`pm-card-link pm-link-${l.type}`} title={l.label}>
                              {l.type === "commit" ? <GitCommit size={10} /> : l.type === "branch" ? <GitBranch size={10} /> : l.type === "pr" ? <ExternalLink size={10} /> : l.type === "note" ? <FileText size={10} /> : <Workflow size={10} />}
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* ─── List View ─── */}
      {viewMode === "list" && (
        <div className="pm-list-view">
          <table className="pm-table">
            <thead>
              <tr>
                <th>Priority</th>
                <th>Title</th>
                <th>Status</th>
                <th>Assignee</th>
                <th>Points</th>
                <th>Time</th>
                <th>Fuel</th>
                <th>Tags</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {filteredTasks.map(task => {
                const col = COLUMNS.find(c => c.id === task.column);
                return (
                  <tr key={task.id} className={selectedTask?.id === task.id ? "selected" : ""} onClick={() => setSelectedTask(task)}>
                    <td><span style={{ color: PRIORITY_MAP[task.priority].color }}>{PRIORITY_MAP[task.priority].icon}</span></td>
                    <td className="pm-table-title">{task.title}</td>
                    <td><span className="pm-table-status" style={{ color: col?.color }}>{col?.icon} {col?.label}</span></td>
                    <td className="pm-table-assignee">{task.assignee}</td>
                    <td>{task.complexity}</td>
                    <td>{formatTime(task.timeSpent)}</td>
                    <td><Zap size={10} style={{ display: "inline", verticalAlign: "middle" }} />{task.fuelCost}</td>
                    <td>
                      <div className="pm-table-tags">
                        {task.tags.map(tId => {
                          const tag = TAGS.find(t => t.id === tId);
                          return tag ? <span key={tId} className="pm-card-tag" style={{ background: tag.color + "22", color: tag.color }}>{tag.name}</span> : null;
                        })}
                      </div>
                    </td>
                    <td>
                      <select className="pm-table-move" value={task.column} onChange={e => moveTask(task.id, e.target.value as ColumnId)}>
                        {COLUMNS.map(c => <option key={c.id} value={c.id}>{c.label}</option>)}
                      </select>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {/* ─── Timeline View (Sprint) ─── */}
      {viewMode === "timeline" && (
        <div className="pm-timeline-view">
          <div className="pm-timeline-header">
            <h3>{activeSprint.name}: {formatDate(activeSprint.startDate)} — {formatDate(activeSprint.endDate)}</h3>
            <div className="pm-timeline-progress">
              <div className="pm-timeline-bar">
                <div className="pm-timeline-fill" style={{ width: `${stats.totalPoints > 0 ? (stats.donePoints / stats.totalPoints) * 100 : 0}%` }} />
              </div>
              <span>{stats.totalPoints > 0 ? Math.round((stats.donePoints / stats.totalPoints) * 100) : 0}% complete</span>
            </div>
          </div>
          <div className="pm-timeline-grid">
            {COLUMNS.map(col => {
              const colTasks = getColumnTasks(col.id);
              return (
                <div key={col.id} className="pm-timeline-col">
                  <div className="pm-timeline-col-header" style={{ color: col.color }}>{col.icon} {col.label} ({colTasks.length})</div>
                  {colTasks.map(task => (
                    <div key={task.id} className="pm-timeline-task" onClick={() => setSelectedTask(task)}>
                      <span className="pm-timeline-task-priority" style={{ background: PRIORITY_MAP[task.priority].color }} />
                      <span className="pm-timeline-task-title">{task.title}</span>
                      <span className="pm-timeline-task-pts">{task.complexity}pt</span>
                    </div>
                  ))}
                </div>
              );
            })}
          </div>

          {/* automations */}
          <div className="pm-automations">
            <h3 className="pm-section-title">Workflow Automations</h3>
            <div className="pm-automations-grid">
              {automations.map(auto => (
                <div key={auto.id} className={`pm-automation-card ${auto.enabled ? "enabled" : "disabled"}`}>
                  <div className="pm-automation-header">
                    <span className="pm-automation-trigger">{auto.trigger}</span>
                    <button type="button" className={`pm-automation-toggle ${auto.enabled ? "on" : "off"}`} onClick={() => toggleAutomation(auto.id)}>
                      {auto.enabled ? "ON" : "OFF"}
                    </button>
                  </div>
                  <div className="pm-automation-action">{auto.action}</div>
                  <div className="pm-automation-meta">
                    <span className="pm-automation-agent"><Hexagon size={10} style={{ display: "inline", verticalAlign: "middle", marginRight: 2 }} /> {auto.agent}</span>
                    {auto.lastRun && <span className="pm-automation-last">Last: {new Date(auto.lastRun).toLocaleTimeString()}</span>}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* ─── Metrics View ─── */}
      {viewMode === "metrics" && (
        <div className="pm-metrics-view">
          {/* summary cards */}
          <div className="pm-metrics-cards">
            <div className="pm-metric-card">
              <div className="pm-metric-label">Total Tasks</div>
              <div className="pm-metric-value">{stats.total}</div>
            </div>
            <div className="pm-metric-card">
              <div className="pm-metric-label">Completed</div>
              <div className="pm-metric-value pm-metric-green">{stats.done}</div>
            </div>
            <div className="pm-metric-card">
              <div className="pm-metric-label">In Progress</div>
              <div className="pm-metric-value pm-metric-purple">{stats.inProgress}</div>
            </div>
            <div className="pm-metric-card">
              <div className="pm-metric-label">Story Points</div>
              <div className="pm-metric-value">{stats.donePoints}<span className="pm-metric-sub">/{stats.totalPoints}</span></div>
            </div>
            <div className="pm-metric-card">
              <div className="pm-metric-label">Total Fuel</div>
              <div className="pm-metric-value pm-metric-yellow"><Zap size={14} style={{ display: "inline", verticalAlign: "middle" }} />{stats.totalFuel}</div>
            </div>
            <div className="pm-metric-card">
              <div className="pm-metric-label">Time Tracked</div>
              <div className="pm-metric-value">{formatTime(stats.totalTime)}</div>
            </div>
          </div>

          <div className="pm-charts-row">
            <div className="pm-chart-card">
              <div className="pm-chart-header">Burndown Chart</div>
              <div style={{ display: "grid", placeItems: "center", height: 200, opacity: 0.8, textAlign: "center", gap: 8 }}>
                <strong>{stats.donePoints} / {stats.totalPoints || 0} points completed</strong>
                <span>{Math.max(stats.totalPoints - stats.donePoints, 0)} points remaining in the active sprint.</span>
              </div>
            </div>
            <div className="pm-chart-card">
              <div className="pm-chart-header">Velocity History</div>
              <div style={{ display: "grid", placeItems: "center", height: 200, opacity: 0.8, textAlign: "center", gap: 8 }}>
                <strong>{sprints.length} sprint{sprints.length === 1 ? "" : "s"} tracked</strong>
                <span>
                  {sprints.length >= 3
                    ? `Average completed points: ${Math.round(sprints.reduce((sum, sprint) => sum + sprint.completedPoints, 0) / sprints.length)}`
                    : "Complete a few more sprints to unlock a richer velocity history."}
                </span>
              </div>
            </div>
          </div>

          {/* sprint history */}
          <div className="pm-sprint-history">
            <div className="pm-section-title">Sprint History</div>
            <div className="pm-sprint-cards">
              {sprints.map(sp => (
                <div key={sp.id} className={`pm-sprint-card ${sp.active ? "active" : ""}`}>
                  <div className="pm-sprint-name">{sp.name} {sp.active && <span className="pm-sprint-active-badge">Active</span>}</div>
                  <div className="pm-sprint-dates">{formatDate(sp.startDate)} — {formatDate(sp.endDate)}</div>
                  <div className="pm-sprint-bar-wrap">
                    <div className="pm-sprint-bar">
                      <div className="pm-sprint-fill" style={{ width: `${sp.totalPoints > 0 ? (sp.completedPoints / sp.totalPoints) * 100 : 0}%` }} />
                    </div>
                    <span>{sp.completedPoints}/{sp.totalPoints} pts</span>
                  </div>
                  <div className="pm-sprint-velocity">Velocity: {sp.velocity} pts/sprint</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* ─── Task Detail Panel ─── */}
      {selectedTask && (
        <aside className="pm-detail-panel">
          <div className="pm-detail-header">
            <span className="pm-detail-id">{selectedTask.id.replace("task-", "#")}</span>
            <button type="button" className="pm-detail-close" onClick={() => setSelectedTask(null)}>×</button>
          </div>
          <input className="pm-detail-title" value={selectedTask.title} onChange={e => { updateTask(selectedTask.id, { title: e.target.value }); setSelectedTask({ ...selectedTask, title: e.target.value }); }} />
          <textarea className="pm-detail-desc" value={selectedTask.description} onChange={e => { updateTask(selectedTask.id, { description: e.target.value }); setSelectedTask({ ...selectedTask, description: e.target.value }); }} placeholder="Add description..." />

          <div className="pm-detail-fields">
            <div className="pm-detail-field">
              <label>Status</label>
              <select value={selectedTask.column} onChange={e => { const col = e.target.value as ColumnId; moveTask(selectedTask.id, col); setSelectedTask({ ...selectedTask, column: col }); }}>
                {COLUMNS.map(c => <option key={c.id} value={c.id}>{c.label}</option>)}
              </select>
            </div>
            <div className="pm-detail-field">
              <label>Priority</label>
              <select value={selectedTask.priority} onChange={e => { const p = e.target.value as Priority; updateTask(selectedTask.id, { priority: p }); setSelectedTask({ ...selectedTask, priority: p }); }}>
                {(["critical", "high", "medium", "low"] as Priority[]).map(p => <option key={p} value={p}>{PRIORITY_MAP[p].label}</option>)}
              </select>
            </div>
            <div className="pm-detail-field">
              <label>Assignee</label>
              <select value={selectedTask.assignee} onChange={e => { updateTask(selectedTask.id, { assignee: e.target.value }); setSelectedTask({ ...selectedTask, assignee: e.target.value }); }}>
                {ASSIGNEES.map(a => <option key={a} value={a}>{a}</option>)}
              </select>
            </div>
            <div className="pm-detail-field">
              <label>Complexity</label>
              <select value={selectedTask.complexity} onChange={e => { const c = Number(e.target.value); updateTask(selectedTask.id, { complexity: c }); setSelectedTask({ ...selectedTask, complexity: c }); }}>
                {[1, 2, 3, 5, 8].map(p => <option key={p} value={p}>{p} pts</option>)}
              </select>
            </div>
          </div>

          {/* subtasks */}
          <div className="pm-detail-subtasks">
            <div className="pm-detail-label">Subtasks ({selectedTask.subtasks.filter(s => s.done).length}/{selectedTask.subtasks.length})</div>
            {selectedTask.subtasks.map((sub, i) => (
              <div key={i} className={`pm-subtask ${sub.done ? "done" : ""}`} onClick={() => { toggleSubtask(selectedTask.id, i); setSelectedTask({ ...selectedTask, subtasks: selectedTask.subtasks.map((s, j) => j === i ? { ...s, done: !s.done } : s) }); }}>
                <span className="pm-subtask-check">{sub.done ? <CheckSquare size={14} /> : <Square size={14} />}</span>
                <span className="pm-subtask-label">{sub.label}</span>
              </div>
            ))}
          </div>

          {/* links */}
          {selectedTask.links.length > 0 && (
            <div className="pm-detail-links">
              <div className="pm-detail-label">Links</div>
              {selectedTask.links.map((link, i) => (
                <div key={i} className="pm-detail-link">
                  <span className={`pm-link-icon pm-link-${link.type}`}>
                    {link.type === "commit" ? "\u25CE" : link.type === "branch" ? "\u2387" : link.type === "pr" ? "\u2934" : link.type === "note" ? "\u25A1" : "\u2387"}
                  </span>
                  <span className="pm-link-text">{link.label}</span>
                </div>
              ))}
            </div>
          )}

          {/* meta */}
          <div className="pm-detail-meta">
            <div className="pm-detail-meta-row"><span>Time</span><span>{formatTime(selectedTask.timeSpent)}</span></div>
            <div className="pm-detail-meta-row"><span>Fuel</span><span><Zap size={10} style={{ display: "inline", verticalAlign: "middle" }} />{selectedTask.fuelCost}</span></div>
            <div className="pm-detail-meta-row"><span>Created by</span><span>{selectedTask.createdBy}</span></div>
            <div className="pm-detail-meta-row"><span>Created</span><span>{formatDate(selectedTask.createdAt)}</span></div>
            <div className="pm-detail-meta-row"><span>Updated</span><span>{new Date(selectedTask.updatedAt).toLocaleString()}</span></div>
          </div>

          <button type="button" className="pm-btn-delete" onClick={() => deleteTask(selectedTask.id)}>Delete Task</button>
        </aside>
      )}

      {/* ─── Status Bar ─── */}
      <div className="pm-status-bar">
        <span className="pm-status-item">{activeSprint.name}</span>
        <span className="pm-status-item">{stats.done}/{stats.total} tasks done</span>
        <span className="pm-status-item">{stats.donePoints}/{stats.totalPoints} points</span>
        <span className="pm-status-item"><Clock size={10} style={{ display: "inline", verticalAlign: "middle", marginRight: 2 }} /> {formatTime(stats.totalTime)}</span>
        <span className="pm-status-item pm-status-right">{stats.daysLeft} days remaining</span>
        <span className="pm-status-item">{auditLog[0] ?? "Ready"}</span>
      </div>
    </div>
  );
}
