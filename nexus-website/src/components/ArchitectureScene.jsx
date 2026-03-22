import { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { ARCHITECTURE_LAYERS } from '../data/constants';

export default function ArchitectureScene() {
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
      const camera = new THREE.PerspectiveCamera(50, width / height, 1, 1000);
      camera.position.set(0, 20, 500);

      const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
      renderer.setSize(width, height);
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      renderer.setClearColor(0x000000, 0);
      container.appendChild(renderer.domElement);

      const group = new THREE.Group();
      scene.add(group);

      // Wireframe grid floor
      const gridHelper = new THREE.GridHelper(400, 20, 0x0088aa, 0x0a2030);
      gridHelper.position.y = -220;
      gridHelper.material.transparent = true;
      gridHelper.material.opacity = 0.3;
      group.add(gridHelper);

      const layers = [];
      const layerHeight = 20;
      const gap = 30;
      const totalHeight = (ARCHITECTURE_LAYERS.length - 1) * (layerHeight + gap);

      ARCHITECTURE_LAYERS.forEach((layer, i) => {
        const color = new THREE.Color(layer.color);
        const geometry = new THREE.BoxGeometry(200, layerHeight, 100);

        // Semi-transparent fill
        const material = new THREE.MeshBasicMaterial({
          color,
          transparent: true,
          opacity: 0.08,
          wireframe: false,
        });
        const mesh = new THREE.Mesh(geometry, material);
        const y = totalHeight / 2 - i * (layerHeight + gap);
        mesh.position.y = y;

        // Wireframe edges
        const edgesGeo = new THREE.EdgesGeometry(geometry);
        const edgesMat = new THREE.LineBasicMaterial({
          color,
          transparent: true,
          opacity: 0.6,
        });
        const edges = new THREE.LineSegments(edgesGeo, edgesMat);
        mesh.add(edges);

        group.add(mesh);
        layers.push({ mesh, baseY: y, phase: i * 0.5 });
      });

      // Add particle connections between layers (data flow)
      const particleCount = 40;
      const particlePositions = new Float32Array(particleCount * 3);
      const particleVelocities = [];
      for (let i = 0; i < particleCount; i++) {
        particlePositions[i * 3] = (Math.random() - 0.5) * 160;
        particlePositions[i * 3 + 1] = (Math.random() - 0.5) * totalHeight;
        particlePositions[i * 3 + 2] = (Math.random() - 0.5) * 80;
        particleVelocities.push((Math.random() - 0.5) * 0.5);
      }
      const particleGeo = new THREE.BufferGeometry();
      particleGeo.setAttribute('position', new THREE.BufferAttribute(particlePositions, 3));
      const particleMat = new THREE.PointsMaterial({
        color: 0x00d4ff,
        size: 2.5,
        transparent: true,
        opacity: 0.5,
        blending: THREE.AdditiveBlending,
      });
      const particles = new THREE.Points(particleGeo, particleMat);
      group.add(particles);

      // Rotation state
      let rotY = 0;
      let isDragging = false;
      let dragStartX = 0;
      let dragRotY = 0;

      function onPointerDown(e) {
        isDragging = true;
        dragStartX = e.clientX;
        dragRotY = rotY;
        container.style.cursor = 'grabbing';
      }
      function onPointerMove(e) {
        if (isDragging) {
          rotY = dragRotY + (e.clientX - dragStartX) * 0.005;
        }
      }
      function onPointerUp() {
        isDragging = false;
        container.style.cursor = 'grab';
      }

      container.addEventListener('pointerdown', onPointerDown);
      window.addEventListener('pointermove', onPointerMove);
      window.addEventListener('pointerup', onPointerUp);

      function animate() {
        if (disposed) return;
        if (document.hidden) {
          animId = requestAnimationFrame(animate);
          return;
        }

        if (!isDragging) {
          rotY += 0.003;
        }
        group.rotation.y = rotY;

        const time = Date.now() * 0.001;
        layers.forEach(({ mesh, baseY, phase }) => {
          mesh.position.y = baseY + Math.sin(time + phase) * 3;
        });

        // Animate particles flowing between layers
        for (let i = 0; i < particleCount; i++) {
          particlePositions[i * 3 + 1] += particleVelocities[i];
          if (particlePositions[i * 3 + 1] > totalHeight / 2 + 20) {
            particlePositions[i * 3 + 1] = -totalHeight / 2 - 20;
          }
          if (particlePositions[i * 3 + 1] < -totalHeight / 2 - 20) {
            particlePositions[i * 3 + 1] = totalHeight / 2 + 20;
          }
        }
        particleGeo.attributes.position.needsUpdate = true;

        renderer.render(scene, camera);
        animId = requestAnimationFrame(animate);
      }

      animate();
      sceneRef.current = true;

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
        container.removeEventListener('pointerdown', onPointerDown);
        window.removeEventListener('pointermove', onPointerMove);
        window.removeEventListener('pointerup', onPointerUp);
        window.removeEventListener('resize', onResize);
        particleGeo.dispose();
        particleMat.dispose();
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
        width: '100%',
        height: 500,
        cursor: 'grab',
        position: 'relative',
      }}
    />
  );
}
