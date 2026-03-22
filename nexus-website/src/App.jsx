import React, { Suspense, lazy, useEffect } from 'react';
import { Routes, Route, useLocation } from 'react-router-dom';
import Navbar from './components/Navbar';
import Footer from './components/Footer';
import CursorGlow from './components/CursorGlow';
import BootSequence from './components/BootSequence';

const Home = lazy(() => import('./pages/Home'));
const Features = lazy(() => import('./pages/Features'));
const Architecture = lazy(() => import('./pages/Architecture'));
const Agents = lazy(() => import('./pages/Agents'));
const Comparison = lazy(() => import('./pages/Comparison'));
const Roadmap = lazy(() => import('./pages/Roadmap'));
const Docs = lazy(() => import('./pages/Docs'));
const Enterprise = lazy(() => import('./pages/Enterprise'));
const Changelog = lazy(() => import('./pages/Changelog'));
const Community = lazy(() => import('./pages/Community'));

function LoadingFallback() {
  return (
    <div style={{
      minHeight: '100vh',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      background: '#0a0e17',
    }}>
      <div style={{
        fontFamily: "'Orbitron', sans-serif",
        fontSize: '0.75rem',
        color: '#00d4ff',
        letterSpacing: '0.3em',
        textTransform: 'uppercase',
        animation: 'pulse-glow 1.5s ease-in-out infinite',
      }}>
        INITIALIZING
      </div>
    </div>
  );
}

function ScrollToTop() {
  const { pathname } = useLocation();
  useEffect(() => {
    window.scrollTo(0, 0);
  }, [pathname]);
  return null;
}

export default function App() {
  const location = useLocation();

  return (
    <>
      {location.pathname === '/' && <BootSequence />}
      <CursorGlow />
      <Navbar />
      <ScrollToTop />
      <main style={{ flex: 1, paddingTop: 88 }}>
        <Suspense fallback={<LoadingFallback />}>
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/features" element={<Features />} />
            <Route path="/architecture" element={<Architecture />} />
            <Route path="/agents" element={<Agents />} />
            <Route path="/comparison" element={<Comparison />} />
            <Route path="/roadmap" element={<Roadmap />} />
            <Route path="/docs" element={<Docs />} />
            <Route path="/enterprise" element={<Enterprise />} />
            <Route path="/changelog" element={<Changelog />} />
            <Route path="/community" element={<Community />} />
          </Routes>
        </Suspense>
      </main>
      <Footer />
    </>
  );
}
