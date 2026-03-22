import { useEffect, useMemo, useRef, useState } from 'react';
import * as THREE from 'three';
import { useReducedMotion } from '../hooks/useReducedMotion';

export default function ThreeScene({
  setup,
  height = 400,
  className = '',
  fallback = null,
  disableOnMobile = true,
  minInteractiveWidth = 768,
  ariaLabel = 'Decorative 3D scene',
}) {
  const mountRef = useRef(null);
  const reducedMotion = useReducedMotion();
  const [fallbackMode, setFallbackMode] = useState(false);
  const resolvedStyle = useMemo(
    () => ({ width: '100%', height: typeof height === 'number' ? `${height}px` : height }),
    [height],
  );

  useEffect(() => {
    const updateFallbackMode = () => {
      const mobile = disableOnMobile && window.innerWidth < minInteractiveWidth;
      setFallbackMode(reducedMotion || mobile);
    };

    updateFallbackMode();
    window.addEventListener('resize', updateFallbackMode);

    return () => {
      window.removeEventListener('resize', updateFallbackMode);
    };
  }, [disableOnMobile, minInteractiveWidth, reducedMotion]);

  useEffect(() => {
    if (fallbackMode) {
      return undefined;
    }

    const mount = mountRef.current;
    if (!mount || typeof setup !== 'function') {
      return undefined;
    }

    let disposed = false;
    let frameId;
    let resizeObserver;

    try {
      const scene = new THREE.Scene();
      const initialHeight = mount.clientHeight || (typeof height === 'number' ? height : 400);
      const camera = new THREE.PerspectiveCamera(60, mount.clientWidth / initialHeight, 0.1, 1000);
      const renderer = new THREE.WebGLRenderer({
        antialias: true,
        alpha: true,
        powerPreference: 'high-performance',
      });
      renderer.setSize(mount.clientWidth, initialHeight);
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      renderer.setClearColor(0x000000, 0);
      mount.appendChild(renderer.domElement);

      const mouse = { x: 0, y: 0 };
      const onMouseMove = (e) => {
        const rect = mount.getBoundingClientRect();
        mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
        mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
      };
      mount.addEventListener('mousemove', onMouseMove);

      const sceneState = setup(scene, camera, renderer, mouse, mount) || {};
      const animate = typeof sceneState.animate === 'function' ? sceneState.animate : () => {};
      const sceneCleanup = typeof sceneState.cleanup === 'function' ? sceneState.cleanup : () => {};

      const onResize = () => {
        const width = mount.clientWidth || 1;
        const currentHeight = mount.clientHeight || initialHeight;
        camera.aspect = width / currentHeight;
        camera.updateProjectionMatrix();
        renderer.setSize(width, currentHeight, false);
        sceneState.onResize?.({ scene, camera, renderer, width, height: currentHeight });
      };

      const loop = (time) => {
        if (disposed) return;
        if (!document.hidden) {
          animate({ time, scene, camera, renderer, mouse, container: mount });
          renderer.render(scene, camera);
        }
        frameId = requestAnimationFrame(loop);
      };
      loop();

      resizeObserver = new ResizeObserver(onResize);
      resizeObserver.observe(mount);
      onResize();

      return () => {
        disposed = true;
        cancelAnimationFrame(frameId);
        mount.removeEventListener('mousemove', onMouseMove);
        resizeObserver?.disconnect();
        if (sceneCleanup) sceneCleanup();
        if (mount.contains(renderer.domElement)) {
          mount.removeChild(renderer.domElement);
        }
        renderer.dispose();
      };
    } catch (e) {
      console.warn('WebGL unavailable:', e.message);
    }
    return undefined;
  }, [fallbackMode, height, setup]);

  if (fallbackMode) {
    return (
      <div className={`scene-fallback ${className}`.trim()} style={resolvedStyle}>
        {fallback || (
          <div className="mobile-3d-icon">
            <span>TACTICAL MODEL</span>
          </div>
        )}
      </div>
    );
  }

  return <div ref={mountRef} className={className} style={resolvedStyle} aria-label={ariaLabel} />;
}
