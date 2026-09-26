// P0-002C4D2: Nexus-owned Tailwind CSS v3 configuration for Builder projects.
//
// The Builder generator's `tailwind.config.ts` is a fixed mapping (it does not
// depend on the project's tokens) from theme names to CSS custom properties
// whose values the project's own CSS supplies. Project configuration files are
// never trusted or loaded, so the trusted runtime owns this mapping instead; a
// backend test keeps it identical to the generator's output.
import path from 'node:path';

const THEME_EXTENSION = {
  colors: {
    primary: 'var(--color-primary)',
    secondary: 'var(--color-secondary)',
    accent: 'var(--color-accent)',
    bg: 'var(--color-bg)',
    'bg-secondary': 'var(--color-bg-secondary)',
    'text-primary': 'var(--color-text)',
    'text-secondary': 'var(--color-text-secondary)',
    border: 'var(--color-border)',
    'btn-bg': 'var(--btn-bg)',
    'btn-text': 'var(--btn-text)',
    'card-bg': 'var(--card-bg)',
    'card-border': 'var(--card-border)',
    'hero-bg': 'var(--hero-bg)',
    'hero-text': 'var(--hero-text)',
    'nav-bg': 'var(--nav-bg)',
    'nav-text': 'var(--nav-text)',
    'footer-bg': 'var(--footer-bg)',
    'footer-text': 'var(--footer-text)',
    'section-bg': 'var(--section-bg)',
    'section-text': 'var(--section-text)',
  },
  fontFamily: {
    heading: 'var(--font-heading)',
    body: 'var(--font-body)',
    mono: 'var(--font-mono)',
  },
  fontSize: {
    xs: 'var(--text-xs)',
    sm: 'var(--text-sm)',
    base: 'var(--text-base)',
    lg: 'var(--text-lg)',
    xl: 'var(--text-xl)',
    '2xl': 'var(--text-2xl)',
    '3xl': 'var(--text-3xl)',
    '4xl': 'var(--text-4xl)',
  },
  borderRadius: {
    sm: 'var(--radius-sm)',
    md: 'var(--radius-md)',
    lg: 'var(--radius-lg)',
    xl: 'var(--radius-xl)',
    full: 'var(--radius-full)',
  },
  spacing: {
    xs: 'var(--space-xs)',
    sm: 'var(--space-sm)',
    md: 'var(--space-md)',
    lg: 'var(--space-lg)',
    xl: 'var(--space-xl)',
    '2xl': 'var(--space-2xl)',
    section: 'var(--space-section)',
  },
  boxShadow: {
    sm: 'var(--shadow-sm)',
    md: 'var(--shadow-md)',
    lg: 'var(--shadow-lg)',
    xl: 'var(--shadow-xl)',
  },
  transitionDuration: {
    fast: 'var(--duration-fast)',
    normal: 'var(--duration-normal)',
    slow: 'var(--duration-slow)',
  },
};

// A fresh object per call: Tailwind may annotate the configuration it is given.
export function nexusTailwindConfig(projectRoot) {
  const root = projectRoot.split(path.sep).join('/');
  return {
    content: [`${root}/index.html`, `${root}/src/**/*.{js,jsx,ts,tsx}`],
    darkMode: 'media',
    theme: { extend: structuredClone(THEME_EXTENSION) },
    plugins: [],
  };
}
