import { useEffect, useRef } from 'react';
import * as THREE from 'three';

const PARTICLE_COUNT = 500;

export default function HeroScene() {
  const mountRef = useRef(null);
  const sceneRef = useRef(null);

  useEffect(() => {
    const container = mountRef.current;
    if (!container || sceneRef.current) return;

    let animId;
    let disposed = false;

    try {
      const width = container.clientWidth;
      const height = container.clientHeight;

      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(60, width / height, 0.1, 1000);
      camera.position.z = 8;

      const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
      renderer.setSize(width, height);
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      renderer.setClearColor(0x000000, 0);
      container.appendChild(renderer.domElement);

      // Build rotating neural sphere
      const positions = new Float32Array(PARTICLE_COUNT * 3);
      for (let i = 0; i < PARTICLE_COUNT; i++) {
        const phi = Math.acos(2 * Math.random() - 1);
        const theta = 2 * Math.PI * Math.random();
        const r = 4 + Math.random() * 0.5;
        positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
        positions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
        positions[i * 3 + 2] = r * Math.cos(phi);
      }

      const pointsGeometry = new THREE.BufferGeometry();
      pointsGeometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      const pointsMaterial = new THREE.PointsMaterial({
        color: 0x00d4ff,
        size: 0.05,
        transparent: true,
        opacity: 0.8,
        blending: THREE.AdditiveBlending,
      });
      const points = new THREE.Points(pointsGeometry, pointsMaterial);

      // Connection lines between nearby particles
      const linePositions = [];
      for (let i = 0; i < PARTICLE_COUNT; i++) {
        for (let j = i + 1; j < PARTICLE_COUNT; j++) {
          const dx = positions[i * 3] - positions[j * 3];
          const dy = positions[i * 3 + 1] - positions[j * 3 + 1];
          const dz = positions[i * 3 + 2] - positions[j * 3 + 2];
          const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
          if (dist < 1.2) {
            linePositions.push(
              positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2],
              positions[j * 3], positions[j * 3 + 1], positions[j * 3 + 2]
            );
          }
        }
      }
      const lineGeometry = new THREE.BufferGeometry();
      lineGeometry.setAttribute('position', new THREE.Float32BufferAttribute(linePositions, 3));
      const lineMaterial = new THREE.LineBasicMaterial({
        color: 0x00d4ff,
        transparent: true,
        opacity: 0.12,
        blending: THREE.AdditiveBlending,
      });
      const lines = new THREE.LineSegments(lineGeometry, lineMaterial);

      const group = new THREE.Group();
      group.add(points);
      group.add(lines);
      scene.add(group);

      // Mouse tracking
      const mouse = { x: 0, y: 0 };
      function onMouseMove(e) {
        mouse.x = (e.clientX / width - 0.5) * 2;
        mouse.y = -(e.clientY / height - 0.5) * 2;
      }
      window.addEventListener('mousemove', onMouseMove);

      function animate() {
        if (disposed) return;
        if (!document.hidden) {
          // Continuous 360° rotation
          group.rotation.y += 0.002;
          group.rotation.x += 0.001;
          // Mouse tilt influence
          group.rotation.x += (mouse.y * 0.3 - group.rotation.x) * 0.01;
          group.rotation.z += (mouse.x * 0.2 - group.rotation.z) * 0.01;
          renderer.render(scene, camera);
        }
        animId = requestAnimationFrame(animate);
      }

      animate();
      sceneRef.current = { renderer, scene, camera };

      function onResize() {
        const w = container.clientWidth;
        const h = container.clientHeight;
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
        renderer.setSize(w, h);
      }
      window.addEventListener('resize', onResize);

      return () => {
        disposed = true;
        cancelAnimationFrame(animId);
        window.removeEventListener('mousemove', onMouseMove);
        window.removeEventListener('resize', onResize);
        pointsGeometry.dispose();
        pointsMaterial.dispose();
        lineGeometry.dispose();
        lineMaterial.dispose();
        renderer.dispose();
        if (container.contains(renderer.domElement)) {
          container.removeChild(renderer.domElement);
        }
        sceneRef.current = null;
      };
    } catch (e) {
      console.warn('WebGL unavailable:', e.message);
    }
  }, []);

  return (
    <div
      ref={mountRef}
      style={{
        position: 'absolute',
        inset: 0,
        zIndex: 0,
        overflow: 'hidden',
      }}
    />
  );
}
