import React, { useMemo, useState } from 'react';
import HomePage from './pages/HomePage';

type RouteDef = { key: string; label: string; component: () => JSX.Element };

const routes: RouteDef[] = [
  { key: 'home', label: 'Home', component: HomePage },
];

export default function App(): JSX.Element {
const [active, setActive] = useState<string>(routes[0]?.key ?? 'home');
const current = useMemo(() => routes.find((route) => route.key === active) ?? routes[0], [active]);
const CurrentComponent = current.component;

return (
<div className="min-h-screen bg-bg text-text">
<header className="sticky top-0 z-20 border-b border-white/10 bg-surface/80 backdrop-blur">
<nav aria-label="page navigation" className="mx-auto flex max-w-6xl gap-2 px-4 py-3">
{routes.map((route) => (
<button
key={route.key}
type="button"
onClick={() => setActive(route.key)}
className={`rounded-full px-4 py-2 text-sm transition-colors ${active === route.key ? 'bg-accent text-black' : 'bg-black/20 text-white'}`}
>
{route.label}
</button>
))}
</nav>
</header>

<main className="mx-auto max-w-6xl px-4 py-8">
<CurrentComponent />
</main>
</div>
);
}
