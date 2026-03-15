import type { Config } from 'tailwindcss';

const config: Config = {
content: ['./index.html', './src/**/*.{ts,tsx}'],
theme: {
extend: {
colors: {
bg: '#0A1228',
surface: '#101B3D',
text: '#EAF2FF',
accent: '#4CC9F0',
accent2: '#4361EE',
},
fontFamily: {
display: ['Sora', 'sans-serif'],
body: ['Inter', 'sans-serif'],
mono: ['JetBrains Mono', 'monospace'],
},
transitionTimingFunction: {
brand: 'cubic-bezier(0.2, 0.8, 0.2, 1)',
},
screens: {
xs: '420px',
sm: '640px',
md: '768px',
lg: '1024px',
xl: '1280px',
},
},
},
plugins: [],
};

export default config;
