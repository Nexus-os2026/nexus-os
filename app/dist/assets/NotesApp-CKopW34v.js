import{r as i,bI as he,j as e}from"./admin-gl7rYrJS.js";import{aI as xe,aJ as V,aK as Y,g as Z,aL as ge,e as T,am as fe,C as je,a as Ne,n as ve,aM as Q,aN as be,aO as we,aP as Ce,F as W,aQ as ke,a3 as Se,aR as ye,ap as $e,aS as Ae}from"./enterprise-D7FEnw0A.js";async function C(l,c){var p;return typeof window<"u"&&typeof((p=window.__TAURI__)==null?void 0:p.invoke)=="function"?window.__TAURI__.invoke(l,c):"[]"}const Ie=[{id:"t1",name:"research",color:"var(--nexus-accent)"},{id:"t2",name:"project",color:"#a78bfa"},{id:"t3",name:"meeting",color:"#f472b6"},{id:"t4",name:"architecture",color:"#34d399"},{id:"t5",name:"bug",color:"#f87171"},{id:"t6",name:"idea",color:"#fbbf24"},{id:"t7",name:"agent-generated",color:"#818cf8"}],Te=[{id:"f-all",name:"All Notes",icon:"clipboard-list",parentId:null,collapsed:!1},{id:"f-projects",name:"Projects",icon:"folder-open",parentId:null,collapsed:!1},{id:"f-research",name:"Research",icon:"microscope",parentId:null,collapsed:!1},{id:"f-meetings",name:"Meetings",icon:"calendar",parentId:null,collapsed:!0},{id:"f-agent",name:"Agent Notes",icon:"hexagon",parentId:null,collapsed:!1},{id:"f-templates",name:"Templates",icon:"file-text",parentId:null,collapsed:!0},{id:"f-archive",name:"Archive",icon:"package",parentId:null,collapsed:!0}],De={"clipboard-list":e.jsx(Ae,{size:14,"aria-hidden":"true"}),"folder-open":e.jsx(Z,{size:14,"aria-hidden":"true"}),microscope:e.jsx(Y,{size:14,"aria-hidden":"true"}),calendar:e.jsx(V,{size:14,"aria-hidden":"true"}),hexagon:e.jsx($e,{size:14,"aria-hidden":"true"}),"file-text":e.jsx(W,{size:14,"aria-hidden":"true"}),package:e.jsx(ye,{size:14,"aria-hidden":"true"})},D={meeting:{title:"Meeting Notes — ",content:`# Meeting Notes

**Date:** ${new Date().toLocaleDateString()}
**Attendees:** 
**Agenda:**

---

## Discussion Points

1. 

## Action Items

- [ ] 

## Decisions Made

- 

## Next Steps

- `,tags:["t3"]},research:{title:"Research: ",content:`# Research Summary

## Objective


## Key Findings

1. 

## Sources

- 

## Analysis


## Conclusions


## Related Links

- `,tags:["t1"]},project:{title:"Project: ",content:`# Project Document

## Overview


## Goals

- [ ] 

## Architecture

\`\`\`

\`\`\`

## Tasks

- [ ] 

## Timeline

| Phase | Description | Status |
|-------|------------|--------|
|       |            |        |

## Notes

`,tags:["t2"]},"bug-report":{title:"Bug: ",content:`# Bug Report

## Description


## Steps to Reproduce

1. 

## Expected Behavior


## Actual Behavior


## Environment

- OS: 
- Version: 

## Screenshots / Logs

\`\`\`

\`\`\`

## Fix

`,tags:["t5"]},blank:{title:"Untitled Note",content:"",tags:[]}};function ze(l){let c=l.replace(/```(\w*)\n([\s\S]*?)```/g,'<pre class="na-code-block"><code>$2</code></pre>').replace(/`([^`]+)`/g,'<code class="na-inline-code">$1</code>').replace(/^#### (.+)$/gm,"<h4>$1</h4>").replace(/^### (.+)$/gm,"<h3>$1</h3>").replace(/^## (.+)$/gm,"<h2>$1</h2>").replace(/^# (.+)$/gm,"<h1>$1</h1>").replace(/\*\*\*(.+?)\*\*\*/g,"<strong><em>$1</em></strong>").replace(/\*\*(.+?)\*\*/g,"<strong>$1</strong>").replace(/\*(.+?)\*/g,"<em>$1</em>").replace(/~~(.+?)~~/g,"<del>$1</del>").replace(/^> (.+)$/gm,"<blockquote>$1</blockquote>").replace(/^---$/gm,"<hr />").replace(/^- \[x\] (.+)$/gm,'<div class="na-checkbox checked">☑ $1</div>').replace(/^- \[ \] (.+)$/gm,'<div class="na-checkbox">☐ $1</div>').replace(/^- (.+)$/gm,"<li>$1</li>").replace(/^\d+\. (.+)$/gm,"<li>$1</li>").replace(/^\|(.+)\|$/gm,p=>{const k=p.split("|").filter(o=>o.trim());if(k.every(o=>/^[\s-:]+$/.test(o)))return"";const m="td";return`<tr>${k.map(o=>`<${m}>${o.trim()}</${m}>`).join("")}</tr>`}).replace(/!\[([^\]]*)\]\(([^)]+)\)/g,'<img alt="$1" src="$2" class="na-img" />').replace(/\[([^\]]+)\]\(([^)]+)\)/g,'<a href="$2" class="na-link">$1</a>').replace(/\n\n/g,"</p><p>").replace(/\n/g,"<br />");return c=c.replace(/((?:<li>.*?<\/li>\s*)+)/g,"<ul>$1</ul>"),c=c.replace(/((?:<tr>.*?<\/tr>\s*)+)/g,'<table class="na-table">$1</table>'),`<p>${c}</p>`}function _e(){var q,G,H;const[l,c]=i.useState([]),[p,k]=i.useState(Te),[m]=i.useState(Ie),[o,N]=i.useState(""),[h,X]=i.useState("f-all"),[x,z]=i.useState(null),[v,M]=i.useState(""),[g,S]=i.useState("split"),[L,y]=i.useState(!0),[_,E]=i.useState(!1),[P,R]=i.useState(!1),[$,ee]=i.useState(!1),[b,te]=i.useState("updated"),[ae,F]=i.useState(0),[B,O]=i.useState(!1),[ne,se]=i.useState(!1),ie=i.useRef(null),u=i.useRef(null),d=i.useMemo(()=>l.find(t=>t.id===o)??null,[l,o]);i.useEffect(()=>()=>{u.current&&clearTimeout(u.current)},[]),i.useEffect(()=>{(async()=>{try{const t=await C("notes_list"),n=JSON.parse(t).map(a=>({id:a.id,title:a.title,content:a.content,folderId:a.folderId||"f-projects",tags:a.tags||[],createdAt:a.createdAt,updatedAt:a.updatedAt,createdBy:"user",pinned:!1,wordCount:a.wordCount||0}));c(n),n.length>0&&N(n[0].id)}catch{}se(!0)})()},[]);const j=i.useCallback(async t=>{O(!0);try{await C("notes_save",{id:t.id,title:t.title,content:t.content,folderId:t.folderId,tagsJson:JSON.stringify(t.tags)})}catch{}O(!1)},[]),J=i.useMemo(()=>{let t=l;if(h!=="f-all"&&(t=t.filter(n=>n.folderId===h)),x&&(t=t.filter(n=>n.tags.includes(x))),v.trim()){const n=v.toLowerCase();t=t.filter(a=>a.title.toLowerCase().includes(n)||a.content.toLowerCase().includes(n)||a.tags.some(s=>{const r=m.find(f=>f.id===s);return r==null?void 0:r.name.toLowerCase().includes(n)}))}return t=[...t].sort((n,a)=>b==="pinned"?(a.pinned?1:0)-(n.pinned?1:0)||a.updatedAt-n.updatedAt:b==="title"?n.title.localeCompare(a.title):b==="created"?a.createdAt-n.createdAt:a.updatedAt-n.updatedAt),t},[l,h,x,v,b,m]),U=i.useCallback(async t=>{try{const n=await he(t),a=JSON.parse(n);a&&a.id&&c(s=>s.map(r=>r.id!==t?r:{...r,title:a.title||r.title,content:a.content??r.content,updatedAt:a.updatedAt||r.updatedAt}))}catch{}},[]);i.useEffect(()=>{o&&U(o)},[o,U]);const K=i.useCallback((t,n)=>{c(a=>a.map(r=>{if(r.id!==t)return r;const f={...r,...n,updatedAt:Date.now(),wordCount:(n.content??r.content).split(/\s+/).filter(Boolean).length};return u.current&&clearTimeout(u.current),u.current=setTimeout(()=>j(f),500),f}))},[j]),w=i.useCallback(async t=>{const n=t?D[t]:D.blank,a={id:`n-${Date.now()}`,title:n.title,content:n.content,folderId:h==="f-all"?"f-projects":h,tags:n.tags,createdAt:Date.now(),updatedAt:Date.now(),createdBy:"user",pinned:!1,wordCount:n.content.split(/\s+/).filter(Boolean).length,template:t};c(s=>[a,...s]),N(a.id),E(!1),F(s=>s+2);try{await C("notes_save",{id:a.id,title:a.title,content:a.content,folderId:a.folderId,tagsJson:JSON.stringify(a.tags)})}catch{}},[h]),re=i.useCallback(async t=>{var a;if(l.find(s=>s.id===t)){c(s=>s.filter(r=>r.id!==t)),o===t&&N(((a=l.find(s=>s.id!==t))==null?void 0:a.id)??"");try{await C("notes_delete",{id:t})}catch{}}},[l,o]),le=i.useCallback(async t=>{const n=l.find(s=>s.id===t);if(!n)return;const a={...n,id:`n-${Date.now()}`,title:`${n.title} (copy)`,createdAt:Date.now(),updatedAt:Date.now()};c(s=>[a,...s]),N(a.id);try{await C("notes_save",{id:a.id,title:a.title,content:a.content,folderId:a.folderId,tagsJson:JSON.stringify(a.tags)})}catch{}},[l]),ce=i.useCallback(t=>{c(n=>n.map(a=>a.id===t?{...a,pinned:!a.pinned}:a))},[]),de=i.useCallback(t=>{k(n=>n.map(a=>a.id===t?{...a,collapsed:!a.collapsed}:a))},[]),oe=i.useCallback((t,n)=>{c(a=>a.map(s=>{if(s.id!==t)return s;const r=s.tags.includes(n),f={...s,tags:r?s.tags.filter(me=>me!==n):[...s.tags,n],updatedAt:Date.now()};return u.current&&clearTimeout(u.current),u.current=setTimeout(()=>j(f),300),f}))},[j]),ue=i.useCallback((t,n)=>{c(a=>a.map(s=>{if(s.id!==t)return s;const r={...s,folderId:n,updatedAt:Date.now()};return u.current&&clearTimeout(u.current),u.current=setTimeout(()=>j(r),300),r}))},[j]),pe=i.useCallback(t=>t==="f-all"?l.length:l.filter(n=>n.folderId===t).length,[l]),A=t=>{const n=new Date(t),s=Date.now()-t;return s<6e4?"just now":s<36e5?`${Math.floor(s/6e4)}m ago`:s<864e5?`${Math.floor(s/36e5)}h ago`:s<6048e5?`${Math.floor(s/864e5)}d ago`:n.toLocaleDateString()},I=t=>m.find(n=>n.id===t);return i.useEffect(()=>{const t=n=>{var a;n.ctrlKey&&n.key==="n"&&(n.preventDefault(),w()),n.ctrlKey&&n.key==="b"&&(n.preventDefault(),y(s=>!s)),n.ctrlKey&&n.key==="f"&&(n.preventDefault(),(a=document.getElementById("na-search"))==null||a.focus()),n.ctrlKey&&n.key==="e"&&(n.preventDefault(),S(s=>s==="edit"?"preview":s==="preview"?"split":"edit"))};return window.addEventListener("keydown",t),()=>window.removeEventListener("keydown",t)},[w]),e.jsxs("div",{className:"na-container",children:[L&&e.jsxs("aside",{className:"na-sidebar",children:[e.jsxs("div",{className:"na-sidebar-header",children:[e.jsx("h2",{className:"na-sidebar-title",children:"Notes"}),e.jsxs("div",{className:"na-sidebar-actions",children:[e.jsx("button",{className:"na-btn-icon",onClick:()=>E(!_),title:"New note",children:"+"}),e.jsx("button",{className:"na-btn-icon cursor-pointer",onClick:()=>y(!1),title:"Hide sidebar",children:e.jsx(xe,{size:14,"aria-hidden":"true"})})]})]}),_&&e.jsxs("div",{className:"na-template-menu",children:[e.jsx("div",{className:"na-template-header",children:"New from template"}),Object.entries(D).map(([t,n])=>e.jsxs("button",{className:"na-template-item cursor-pointer",onClick:()=>w(t),children:[e.jsx("span",{className:"na-template-icon",children:t==="meeting"?e.jsx(V,{size:14,"aria-hidden":"true"}):t==="research"?e.jsx(Y,{size:14,"aria-hidden":"true"}):t==="project"?e.jsx(Z,{size:14,"aria-hidden":"true"}):t==="bug-report"?e.jsx(ge,{size:14,"aria-hidden":"true"}):e.jsx(T,{size:14,"aria-hidden":"true"})}),e.jsx("span",{children:t.replace("-"," ").replace(/\b\w/g,a=>a.toUpperCase())})]},t))]}),e.jsxs("div",{className:"na-search-box",children:[e.jsx("span",{className:"na-search-icon",children:e.jsx(fe,{size:14,"aria-hidden":"true"})}),e.jsx("input",{id:"na-search",className:"na-search-input",placeholder:"Search notes...",value:v,onChange:t=>M(t.target.value)}),v&&e.jsx("button",{className:"na-search-clear",onClick:()=>M(""),children:"×"})]}),e.jsx("div",{className:"na-folders",children:p.map(t=>e.jsxs("div",{className:`na-folder-item ${h===t.id?"active":""}`,children:[e.jsxs("button",{className:"na-folder-btn cursor-pointer",onClick:()=>{X(t.id),z(null)},children:[e.jsx("span",{className:"na-folder-icon",children:De[t.icon]??t.icon}),e.jsx("span",{className:"na-folder-name",children:t.name}),e.jsx("span",{className:"na-folder-count",children:pe(t.id)})]}),t.parentId===null&&t.id!=="f-all"&&e.jsx("button",{className:"na-folder-toggle cursor-pointer",onClick:()=>de(t.id),children:t.collapsed?e.jsx(je,{size:12,"aria-hidden":"true"}):e.jsx(Ne,{size:12,"aria-hidden":"true"})})]},t.id))}),e.jsxs("div",{className:"na-tags-section",children:[e.jsx("div",{className:"na-tags-header",children:"Tags"}),e.jsx("div",{className:"na-tags-list",children:m.map(t=>e.jsxs("button",{className:`na-tag-filter ${x===t.id?"active":""}`,onClick:()=>z(x===t.id?null:t.id),style:{borderColor:t.color},children:[e.jsx("span",{className:"na-tag-dot",style:{background:t.color}}),t.name]},t.id))})]}),e.jsxs("div",{className:"na-agent-activity",children:[e.jsx("div",{className:"na-agent-header",children:"Storage"}),e.jsxs("div",{className:"na-agent-log",children:[e.jsxs("div",{className:"na-agent-entry",children:[l.length," notes saved to ~/.nexus/notes/"]}),B&&e.jsx("div",{className:"na-agent-entry",children:"Saving..."}),ne&&l.length===0&&e.jsx("div",{className:"na-agent-entry",children:"No notes yet. Create one!"})]})]})]}),e.jsxs("div",{className:"na-note-list",children:[e.jsxs("div",{className:"na-list-header",children:[e.jsxs("div",{className:"na-list-title",children:[!L&&e.jsx("button",{className:"na-btn-icon cursor-pointer",onClick:()=>y(!0),title:"Show sidebar",children:e.jsx(ve,{size:12,"aria-hidden":"true"})}),e.jsx("span",{children:((q=p.find(t=>t.id===h))==null?void 0:q.name)??"All Notes"}),x&&e.jsxs("span",{className:"na-filter-badge",style:{borderColor:(G=I(x))==null?void 0:G.color},children:["#",(H=I(x))==null?void 0:H.name]})]}),e.jsx("div",{className:"na-list-controls",children:e.jsxs("select",{className:"na-sort-select",value:b,onChange:t=>te(t.target.value),children:[e.jsx("option",{value:"updated",children:"Last Modified"}),e.jsx("option",{value:"created",children:"Date Created"}),e.jsx("option",{value:"title",children:"Title"}),e.jsx("option",{value:"pinned",children:"Pinned First"})]})})]}),e.jsxs("div",{className:"na-list-items",children:[J.length===0&&e.jsxs("div",{className:"na-empty",children:[e.jsx("div",{className:"na-empty-icon",children:e.jsx(T,{size:24,"aria-hidden":"true"})}),e.jsx("div",{children:"No notes found"}),e.jsx("button",{className:"na-btn-create",onClick:()=>w(),children:"Create Note"})]}),J.map(t=>e.jsxs("div",{className:`na-note-card ${o===t.id?"active":""} ${t.pinned?"pinned":""}`,onClick:()=>N(t.id),children:[e.jsxs("div",{className:"na-note-card-header",children:[e.jsxs("span",{className:"na-note-card-title",children:[t.pinned&&e.jsx("span",{className:"na-pin",children:e.jsx(Q,{size:12,"aria-hidden":"true"})}),t.title]}),e.jsxs("div",{className:"na-note-card-actions",children:[e.jsx("button",{className:"na-btn-tiny cursor-pointer",onClick:n=>{n.stopPropagation(),ce(t.id)},children:t.pinned?e.jsx(be,{size:12,"aria-hidden":"true"}):e.jsx(Q,{size:12,"aria-hidden":"true"})}),e.jsx("button",{className:"na-btn-tiny",onClick:n=>{n.stopPropagation(),re(t.id)},children:"×"})]})]}),e.jsxs("div",{className:"na-note-card-preview",children:[t.content.replace(/[#*`>\[\]|_~-]/g,"").slice(0,100),"..."]}),e.jsxs("div",{className:"na-note-card-meta",children:[e.jsx("span",{className:"na-note-card-date",children:A(t.updatedAt)}),e.jsx("span",{className:"na-note-card-author",children:t.createdBy==="user"?"You":t.createdBy}),e.jsx("div",{className:"na-note-card-tags",children:t.tags.slice(0,3).map(n=>{const a=I(n);return a?e.jsx("span",{className:"na-tag-mini",style:{background:a.color+"22",color:a.color},children:a.name},n):null})})]})]},t.id))]})]}),e.jsx("div",{className:"na-editor-area",children:d?e.jsxs(e.Fragment,{children:[e.jsxs("div",{className:"na-toolbar",children:[e.jsx("div",{className:"na-toolbar-left",children:e.jsx("input",{className:"na-title-input",value:d.title,onChange:t=>K(d.id,{title:t.target.value}),placeholder:"Note title..."})}),e.jsxs("div",{className:"na-toolbar-right",children:[e.jsxs("div",{className:"na-view-toggle",children:[e.jsx("button",{className:`na-view-btn ${g==="edit"?"active":""}`,onClick:()=>S("edit"),children:"Edit"}),e.jsx("button",{className:`na-view-btn ${g==="split"?"active":""}`,onClick:()=>S("split"),children:"Split"}),e.jsx("button",{className:`na-view-btn ${g==="preview"?"active":""}`,onClick:()=>S("preview"),children:"Preview"})]}),e.jsx("button",{className:`na-btn-icon cursor-pointer ${$?"active":""}`,onClick:()=>ee(!$),title:"Tags",children:e.jsx(we,{size:14,"aria-hidden":"true"})}),e.jsxs("div",{className:"na-export-wrapper",children:[e.jsx("button",{className:"na-btn-icon cursor-pointer",onClick:()=>R(!P),title:"Export",children:e.jsx(Ce,{size:14,"aria-hidden":"true"})}),P&&e.jsx("div",{className:"na-export-menu",children:e.jsxs("button",{className:"na-export-item cursor-pointer",onClick:()=>{R(!1),F(t=>t+5)},children:[e.jsx(W,{size:12,"aria-hidden":"true",style:{display:"inline",verticalAlign:"middle",marginRight:4}})," Markdown (.md)"]})})]}),e.jsx("div",{className:"na-move-wrapper",children:e.jsx("select",{className:"na-move-select",value:d.folderId,onChange:t=>ue(d.id,t.target.value),children:p.filter(t=>t.id!=="f-all").map(t=>e.jsx("option",{value:t.id,children:t.name},t.id))})}),e.jsx("button",{className:"na-btn-icon cursor-pointer",onClick:()=>le(d.id),title:"Duplicate",children:e.jsx(ke,{size:14,"aria-hidden":"true"})})]})]}),$&&e.jsx("div",{className:"na-tag-picker",children:m.map(t=>e.jsxs("button",{className:`na-tag-pick ${d.tags.includes(t.id)?"selected":""}`,style:{borderColor:t.color,background:d.tags.includes(t.id)?t.color+"22":"transparent"},onClick:()=>oe(d.id,t.id),children:[e.jsx("span",{className:"na-tag-dot",style:{background:t.color}}),t.name]},t.id))}),e.jsxs("div",{className:`na-editor-body ${g}`,children:[(g==="edit"||g==="split")&&e.jsx("div",{className:"na-edit-pane",children:e.jsx("textarea",{ref:ie,className:"na-textarea",value:d.content,onChange:t=>K(d.id,{content:t.target.value}),placeholder:"Start writing... (Markdown supported)",spellCheck:!1})}),(g==="preview"||g==="split")&&e.jsx("div",{className:"na-preview-pane",children:e.jsx("div",{className:"na-preview-content",dangerouslySetInnerHTML:{__html:ze(d.content)}})})]}),e.jsxs("div",{className:"na-status-bar",children:[e.jsxs("span",{className:"na-status-item",children:[d.wordCount," words"]}),e.jsxs("span",{className:"na-status-item",children:["Created ",A(d.createdAt)]}),e.jsxs("span",{className:"na-status-item",children:["Modified ",A(d.updatedAt)]}),B&&e.jsx("span",{className:"na-status-item",children:"Saving..."}),d.template&&e.jsxs("span",{className:"na-status-item",children:["Template: ",d.template]}),e.jsxs("span",{className:"na-status-item na-status-right",children:[e.jsx("span",{className:"na-fuel-icon",children:e.jsx(Se,{size:12,"aria-hidden":"true"})})," ",ae," fuel used"]}),e.jsxs("span",{className:"na-status-item",children:[l.length," notes"]}),e.jsx("span",{className:"na-status-item",children:"Ctrl+N new · Ctrl+B sidebar · Ctrl+E view · Ctrl+F search"})]})]}):e.jsxs("div",{className:"na-no-note",children:[e.jsx("div",{className:"na-no-note-icon",children:e.jsx(T,{size:32,"aria-hidden":"true"})}),e.jsx("div",{className:"na-no-note-text",children:"Select a note or create a new one"}),e.jsx("button",{className:"na-btn-create",onClick:()=>w(),children:"New Note"})]})})]})}export{_e as default};
