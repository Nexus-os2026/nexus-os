import * as THREE from 'three';

function disposeMaterial(material) {
  if (!material) {
    return;
  }

  if (Array.isArray(material)) {
    material.forEach(disposeMaterial);
    return;
  }

  Object.values(material).forEach((value) => {
    if (value && typeof value.dispose === 'function') {
      value.dispose();
    }
  });

  material.dispose?.();
}

function disposeObject(root) {
  if (!root) {
    return;
  }

  root.traverse((child) => {
    child.geometry?.dispose?.();
    disposeMaterial(child.material);
  });
}

function createLineMesh(geometry, color, opacity = 0.7) {
  return new THREE.LineSegments(
    new THREE.EdgesGeometry(geometry),
    new THREE.LineBasicMaterial({
      color,
      transparent: true,
      opacity,
    }),
  );
}

function createGlowSphere(color, radius = 0.22, opacity = 0.18) {
  return new THREE.Mesh(
    new THREE.SphereGeometry(radius, 16, 16),
    new THREE.MeshBasicMaterial({
      color,
      transparent: true,
      opacity,
    }),
  );
}

function addStarfield(scene, color = 0x00d4ff) {
  const positions = new Float32Array(160 * 3);
  for (let index = 0; index < positions.length; index += 3) {
    positions[index] = (Math.random() - 0.5) * 18;
    positions[index + 1] = (Math.random() - 0.5) * 12;
    positions[index + 2] = (Math.random() - 0.5) * 18;
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  const material = new THREE.PointsMaterial({
    color,
    size: 0.035,
    transparent: true,
    opacity: 0.25,
    blending: THREE.AdditiveBlending,
  });
  const stars = new THREE.Points(geometry, material);
  scene.add(stars);

  return stars;
}

function createOctahedron(color = 0x00d4ff) {
  const group = new THREE.Group();
  const geometry = new THREE.OctahedronGeometry(1.12, 0);
  const frame = createLineMesh(geometry, color, 0.95);
  const fill = new THREE.Mesh(
    geometry,
    new THREE.MeshBasicMaterial({
      color,
      transparent: true,
      opacity: 0.08,
    }),
  );
  const core = createGlowSphere(0xff6a00, 0.18, 0.25);
  group.add(fill, frame, core);

  return {
    root: group,
    animate: ({ time }) => {
      group.rotation.x += 0.006;
      group.rotation.y += 0.012;
      const pulse = 1 + Math.sin(time * 0.0022) * 0.05;
      core.scale.setScalar(pulse);
    },
  };
}

function createHelix() {
  const group = new THREE.Group();
  const strandMaterialA = new THREE.MeshBasicMaterial({ color: 0x00d4ff, transparent: true, opacity: 0.9 });
  const strandMaterialB = new THREE.MeshBasicMaterial({ color: 0x00ff88, transparent: true, opacity: 0.88 });
  const sphereGeometry = new THREE.SphereGeometry(0.06, 10, 10);
  const braceMaterial = new THREE.LineBasicMaterial({ color: 0x00d4ff, transparent: true, opacity: 0.22 });

  for (let index = 0; index < 72; index += 1) {
    const theta = (index / 72) * Math.PI * 5;
    const y = (index / 72) * 4.4 - 2.2;
    const left = new THREE.Mesh(sphereGeometry, strandMaterialA);
    const right = new THREE.Mesh(sphereGeometry, strandMaterialB);
    left.position.set(Math.cos(theta) * 0.82, y, Math.sin(theta) * 0.82);
    right.position.set(Math.cos(theta + Math.PI) * 0.82, y, Math.sin(theta + Math.PI) * 0.82);
    group.add(left, right);

    if (index % 4 === 0) {
      const braceGeometry = new THREE.BufferGeometry().setFromPoints([left.position, right.position]);
      const brace = new THREE.Line(braceGeometry, braceMaterial);
      group.add(brace);
    }
  }

  return {
    root: group,
    animate: ({ time }) => {
      group.rotation.y += 0.012;
      group.rotation.z = Math.sin(time * 0.0011) * 0.16;
    },
  };
}

function createNestedCubes() {
  const group = new THREE.Group();
  const outerGeometry = new THREE.BoxGeometry(1.8, 1.8, 1.8);
  const middleGeometry = new THREE.BoxGeometry(1.2, 1.2, 1.2);
  const innerGeometry = new THREE.BoxGeometry(0.72, 0.72, 0.72);

  const outer = createLineMesh(outerGeometry, 0x00d4ff, 0.45);
  const middle = createLineMesh(middleGeometry, 0x00ff88, 0.35);
  const inner = createLineMesh(innerGeometry, 0xff6a00, 0.85);

  group.add(outer, middle, inner);

  return {
    root: group,
    animate: ({ time }) => {
      outer.rotation.x += 0.006;
      outer.rotation.y += 0.01;
      middle.rotation.y -= 0.012;
      middle.rotation.z += 0.008;
      inner.rotation.x -= 0.015;
      inner.rotation.y += 0.02;
      group.rotation.z = Math.sin(time * 0.0013) * 0.12;
    },
  };
}

function createIcosahedron(color = 0x00d4ff) {
  const group = new THREE.Group();
  const geometry = new THREE.IcosahedronGeometry(1.24, 0);
  const frame = createLineMesh(geometry, color, 0.82);
  const glow = new THREE.Mesh(
    geometry,
    new THREE.MeshBasicMaterial({
      color,
      transparent: true,
      opacity: 0.05,
    }),
  );
  group.add(glow, frame);

  return {
    root: group,
    animate: ({ time }) => {
      group.rotation.x += 0.008;
      group.rotation.y += 0.011;
      group.rotation.z = Math.sin(time * 0.0014) * 0.18;
    },
  };
}

function createChainLinks() {
  const group = new THREE.Group();
  const linkMaterial = new THREE.MeshBasicMaterial({ color: 0x00d4ff, wireframe: true });

  for (let index = 0; index < 5; index += 1) {
    const link = new THREE.Mesh(new THREE.TorusGeometry(0.34, 0.08, 10, 18), linkMaterial);
    link.position.x = index * 0.62 - 1.24;
    link.rotation.y = index % 2 === 0 ? 0 : Math.PI / 2;
    group.add(link);
  }

  return {
    root: group,
    animate: () => {
      group.rotation.y += 0.015;
      group.rotation.x += 0.004;
    },
  };
}

function createOrbitalRings() {
  const group = new THREE.Group();
  const core = createGlowSphere(0x00d4ff, 0.28, 0.22);
  const ringA = new THREE.Mesh(
    new THREE.TorusGeometry(1.18, 0.025, 18, 80),
    new THREE.MeshBasicMaterial({ color: 0x00d4ff, transparent: true, opacity: 0.92 }),
  );
  const ringB = new THREE.Mesh(
    new THREE.TorusGeometry(0.82, 0.025, 18, 80),
    new THREE.MeshBasicMaterial({ color: 0x00ff88, transparent: true, opacity: 0.92 }),
  );
  const ringC = new THREE.Mesh(
    new THREE.TorusGeometry(1.46, 0.018, 18, 80),
    new THREE.MeshBasicMaterial({ color: 0xff6a00, transparent: true, opacity: 0.7 }),
  );
  ringB.rotation.x = Math.PI / 2;
  ringC.rotation.z = Math.PI / 3;
  group.add(core, ringA, ringB, ringC);

  return {
    root: group,
    animate: ({ time }) => {
      ringA.rotation.y += 0.013;
      ringB.rotation.x += 0.017;
      ringB.rotation.z += 0.01;
      ringC.rotation.x += 0.008;
      ringC.rotation.z += 0.01;
      core.scale.setScalar(1 + Math.sin(time * 0.0028) * 0.08);
    },
  };
}

function createClockwork() {
  const group = new THREE.Group();

  const createGear = (radius, teeth, color) => {
    const gear = new THREE.Group();
    const ring = new THREE.Mesh(
      new THREE.TorusGeometry(radius, 0.06, 10, 24),
      new THREE.MeshBasicMaterial({ color, wireframe: true }),
    );
    gear.add(ring);

    for (let toothIndex = 0; toothIndex < teeth; toothIndex += 1) {
      const tooth = new THREE.Mesh(
        new THREE.BoxGeometry(0.14, 0.28, 0.08),
        new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.7 }),
      );
      const angle = (toothIndex / teeth) * Math.PI * 2;
      tooth.position.set(Math.cos(angle) * (radius + 0.1), Math.sin(angle) * (radius + 0.1), 0);
      tooth.rotation.z = angle;
      gear.add(tooth);
    }

    return gear;
  };

  const gearA = createGear(0.7, 12, 0x00d4ff);
  const gearB = createGear(0.48, 10, 0xff6a00);
  const gearC = createGear(0.38, 8, 0x00ff88);

  gearA.position.x = -0.65;
  gearB.position.x = 0.55;
  gearB.position.y = 0.28;
  gearC.position.x = 0.42;
  gearC.position.y = -0.62;

  group.add(gearA, gearB, gearC);

  return {
    root: group,
    animate: () => {
      gearA.rotation.z += 0.015;
      gearB.rotation.z -= 0.02;
      gearC.rotation.z += 0.025;
      group.rotation.y += 0.006;
    },
  };
}

function createCylinderStack() {
  const group = new THREE.Group();
  const cylinderMaterial = new THREE.MeshBasicMaterial({ color: 0x00d4ff, wireframe: true, transparent: true, opacity: 0.8 });
  const ringMaterial = new THREE.MeshBasicMaterial({ color: 0xff6a00, transparent: true, opacity: 0.85 });

  for (let index = 0; index < 3; index += 1) {
    const stack = new THREE.Mesh(new THREE.CylinderGeometry(0.9, 0.9, 0.38, 32, 1, true), cylinderMaterial);
    stack.position.y = index * 0.55 - 0.55;
    group.add(stack);
  }

  const ring = new THREE.Mesh(new THREE.TorusGeometry(1.18, 0.03, 12, 60), ringMaterial);
  ring.rotation.x = Math.PI / 2;
  group.add(ring);

  return {
    root: group,
    animate: () => {
      group.rotation.y += 0.01;
      ring.rotation.z += 0.015;
    },
  };
}

function createKeyPair() {
  const group = new THREE.Group();
  const left = new THREE.Mesh(
    new THREE.TorusKnotGeometry(0.52, 0.14, 80, 12, 2, 3),
    new THREE.MeshBasicMaterial({ color: 0x00d4ff, wireframe: true }),
  );
  const right = new THREE.Mesh(
    new THREE.TorusKnotGeometry(0.42, 0.12, 80, 12, 3, 5),
    new THREE.MeshBasicMaterial({ color: 0x00ff88, wireframe: true }),
  );
  left.position.x = -0.45;
  right.position.x = 0.52;
  group.add(left, right);

  return {
    root: group,
    animate: () => {
      left.rotation.x += 0.012;
      left.rotation.y += 0.009;
      right.rotation.x -= 0.01;
      right.rotation.y += 0.014;
      group.rotation.z += 0.002;
    },
  };
}

function createStarNetwork() {
  const group = new THREE.Group();
  const nodeGeometry = new THREE.SphereGeometry(0.09, 12, 12);
  const center = new THREE.Mesh(nodeGeometry, new THREE.MeshBasicMaterial({ color: 0xff6a00 }));
  group.add(center);

  const positions = [
    [1.2, 0, 0],
    [-1.1, 0.45, 0.2],
    [0.2, 1.1, -0.6],
    [-0.3, -1.1, 0.7],
    [1.05, -0.72, -0.4],
  ];

  const linePositions = [];
  positions.forEach((position, index) => {
    const node = new THREE.Mesh(
      nodeGeometry,
      new THREE.MeshBasicMaterial({
        color: index % 2 === 0 ? 0x00d4ff : 0x00ff88,
      }),
    );
    node.position.set(position[0], position[1], position[2]);
    group.add(node);
    linePositions.push(0, 0, 0, position[0], position[1], position[2]);
  });

  const lines = new THREE.LineSegments(
    new THREE.BufferGeometry().setAttribute('position', new THREE.Float32BufferAttribute(linePositions, 3)),
    new THREE.LineBasicMaterial({ color: 0x00d4ff, transparent: true, opacity: 0.42 }),
  );
  group.add(lines);

  return {
    root: group,
    animate: ({ time }) => {
      group.rotation.y += 0.01;
      group.rotation.x = Math.sin(time * 0.0012) * 0.22;
    },
  };
}

function createShieldNode() {
  const group = new THREE.Group();
  const geometry = new THREE.DodecahedronGeometry(1.28, 0);
  const frame = createLineMesh(geometry, 0x00d4ff, 0.95);
  const fill = new THREE.Mesh(
    geometry,
    new THREE.MeshBasicMaterial({
      color: 0x00d4ff,
      transparent: true,
      opacity: 0.05,
    }),
  );
  const orbit = new THREE.Mesh(
    new THREE.TorusGeometry(1.84, 0.02, 18, 96),
    new THREE.MeshBasicMaterial({ color: 0xff6a00, transparent: true, opacity: 0.78 }),
  );
  orbit.rotation.x = Math.PI / 2.8;
  group.add(fill, frame, orbit);

  return {
    root: group,
    animate: () => {
      frame.rotation.y += 0.012;
      fill.rotation.y += 0.012;
      orbit.rotation.z += 0.008;
      orbit.rotation.x += 0.01;
    },
  };
}

function getModelFactory(modelKey) {
  switch (modelKey) {
    case 'governance':
    case 'octahedron':
      return createOctahedron;
    case 'darwin':
    case 'helix':
      return createHelix;
    case 'wasm':
    case 'sandbox':
    case 'cube':
      return createNestedCubes;
    case 'agents':
    case 'flash':
    case 'icosahedron':
      return createIcosahedron;
    case 'audit':
    case 'chain':
    case 'torus':
      return createChainLinks;
    case 'mcp':
    case 'protocol':
    case 'rings':
      return createOrbitalRings;
    case 'scheduler':
      return createClockwork;
    case 'ghost':
    case 'identity':
      return createKeyPair;
    case 'connectors':
    case 'integrations':
      return createStarNetwork;
    case 'enterprise':
      return createShieldNode;
    case 'persistence':
      return createCylinderStack;
    default:
      return createIcosahedron;
  }
}

export function createModelScene(modelKey, options = {}) {
  return (scene, camera, _renderer, mouse) => {
    const { cameraZ = 4.8, scale = 1, stars = true } = options;
    camera.position.set(0, 0, cameraZ);
    const cleanupTargets = [];

    if (stars) {
      cleanupTargets.push(addStarfield(scene));
    }

    const factory = getModelFactory(modelKey);
    const { root, animate: animateRoot } = factory(options.accent);
    root.scale.setScalar(scale);
    scene.add(root);
    cleanupTargets.push(root);

    const targetRotation = { x: 0.18, z: 0 };

    return {
      animate: ({ time }) => {
        targetRotation.x = mouse.y * 0.25;
        targetRotation.z = mouse.x * 0.2;
        root.rotation.x += (targetRotation.x - root.rotation.x) * 0.04;
        root.rotation.z += (targetRotation.z - root.rotation.z) * 0.04;
        animateRoot?.({ time });
      },
      cleanup: () => {
        cleanupTargets.forEach(disposeObject);
      },
    };
  };
}

export function createNeuralCoreScene(options = {}) {
  return (scene, camera) => {
    const { color = 0x00d4ff, particleCount = 520 } = options;
    camera.position.set(0, 0, 7.8);
    const cleanupTargets = [];

    const positions = new Float32Array(particleCount * 3);
    for (let index = 0; index < particleCount; index += 1) {
      const phi = Math.acos(2 * Math.random() - 1);
      const theta = Math.PI * 2 * Math.random();
      const radius = 3.9 + Math.random() * 0.6;
      positions[index * 3] = radius * Math.sin(phi) * Math.cos(theta);
      positions[index * 3 + 1] = radius * Math.sin(phi) * Math.sin(theta);
      positions[index * 3 + 2] = radius * Math.cos(phi);
    }

    const pointGeometry = new THREE.BufferGeometry();
    pointGeometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    const pointMaterial = new THREE.PointsMaterial({
      color,
      size: 0.055,
      transparent: true,
      opacity: 0.88,
      blending: THREE.AdditiveBlending,
    });
    const points = new THREE.Points(pointGeometry, pointMaterial);

    const linePositions = [];
    for (let outerIndex = 0; outerIndex < particleCount; outerIndex += 1) {
      for (let innerIndex = outerIndex + 1; innerIndex < particleCount; innerIndex += 1) {
        const dx = positions[outerIndex * 3] - positions[innerIndex * 3];
        const dy = positions[outerIndex * 3 + 1] - positions[innerIndex * 3 + 1];
        const dz = positions[outerIndex * 3 + 2] - positions[innerIndex * 3 + 2];
        const distance = Math.sqrt(dx * dx + dy * dy + dz * dz);
        if (distance < 1.16) {
          linePositions.push(
            positions[outerIndex * 3],
            positions[outerIndex * 3 + 1],
            positions[outerIndex * 3 + 2],
            positions[innerIndex * 3],
            positions[innerIndex * 3 + 1],
            positions[innerIndex * 3 + 2],
          );
        }
      }
    }

    const lineGeometry = new THREE.BufferGeometry();
    lineGeometry.setAttribute('position', new THREE.Float32BufferAttribute(linePositions, 3));
    const lineMaterial = new THREE.LineBasicMaterial({
      color,
      transparent: true,
      opacity: 0.14,
      blending: THREE.AdditiveBlending,
    });
    const lines = new THREE.LineSegments(lineGeometry, lineMaterial);

    const ringA = new THREE.Mesh(
      new THREE.TorusGeometry(5.1, 0.012, 10, 120),
      new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.22 }),
    );
    const ringB = new THREE.Mesh(
      new THREE.TorusGeometry(4.3, 0.014, 10, 120),
      new THREE.MeshBasicMaterial({ color: 0xff6a00, transparent: true, opacity: 0.1 }),
    );
    ringA.rotation.x = Math.PI / 2.2;
    ringB.rotation.y = Math.PI / 2.6;

    const shell = new THREE.Mesh(
      new THREE.SphereGeometry(4.75, 18, 18),
      new THREE.MeshBasicMaterial({
        color,
        wireframe: true,
        transparent: true,
        opacity: 0.04,
      }),
    );

    const group = new THREE.Group();
    group.add(points, lines, ringA, ringB, shell);
    scene.add(group);
    cleanupTargets.push(group);
    cleanupTargets.push(addStarfield(scene));

    return {
      animate: ({ mouse }) => {
        group.rotation.y += 0.0018;
        group.rotation.x += (mouse.y * 0.28 - group.rotation.x) * 0.02;
        group.rotation.z += (mouse.x * 0.2 - group.rotation.z) * 0.02;
        ringA.rotation.z += 0.0026;
        ringB.rotation.x -= 0.0018;
      },
      cleanup: () => {
        cleanupTargets.forEach(disposeObject);
      },
    };
  };
}

export function createArchitectureBlueprintScene(layers, options = {}) {
  return (scene, camera, _renderer, mouse, container) => {
    camera.position.set(0, 2.4, 9.8);
    const cleanupTargets = [];

    const grid = new THREE.GridHelper(18, 22, 0x00d4ff, 0x14314d);
    grid.position.y = -4.25;
    grid.material.transparent = true;
    grid.material.opacity = 0.22;
    scene.add(grid);
    cleanupTargets.push(grid);

    const group = new THREE.Group();
    scene.add(group);
    cleanupTargets.push(group);

    const layerItems = [];
    const slabGeometry = new THREE.BoxGeometry(5.6, 0.66, 3.2);
    const top = (layers.length - 1) * 1.15 * 0.5;

    layers.forEach((layer, index) => {
      const y = top - index * 1.15;
      const color = new THREE.Color(layer.color);
      const fill = new THREE.Mesh(
        slabGeometry,
        new THREE.MeshBasicMaterial({
          color,
          transparent: true,
          opacity: index === options.activeIndex ? 0.18 : 0.08,
        }),
      );
      fill.position.y = y;
      const frame = createLineMesh(slabGeometry, color, index === options.activeIndex ? 1 : 0.55);
      frame.position.y = y;

      const marker = new THREE.Mesh(
        new THREE.BoxGeometry(1.1, 0.06, 0.06),
        new THREE.MeshBasicMaterial({
          color,
          transparent: true,
          opacity: 0.8,
        }),
      );
      marker.position.set(-2.95, y + 0.42, 1.7);

      group.add(fill, frame, marker);
      layerItems.push({ fill, frame, marker, baseY: y });
    });

    const particleCount = 42;
    const particlePositions = new Float32Array(particleCount * 3);
    for (let index = 0; index < particleCount; index += 1) {
      particlePositions[index * 3] = (Math.random() - 0.5) * 2.2;
      particlePositions[index * 3 + 1] = (Math.random() - 0.5) * 7.6;
      particlePositions[index * 3 + 2] = (Math.random() - 0.5) * 1.7;
    }

    const particles = new THREE.Points(
      new THREE.BufferGeometry().setAttribute('position', new THREE.BufferAttribute(particlePositions, 3)),
      new THREE.PointsMaterial({
        color: 0x00d4ff,
        size: 0.08,
        transparent: true,
        opacity: 0.65,
      }),
    );
    group.add(particles);

    let rotationY = 0.6;
    let rotationX = 0.18;
    let isDragging = false;
    let pointerX = 0;
    let pointerY = 0;

    const onPointerDown = (event) => {
      isDragging = true;
      pointerX = event.clientX;
      pointerY = event.clientY;
      container.style.cursor = 'grabbing';
    };

    const onPointerMove = (event) => {
      if (!isDragging) {
        return;
      }
      rotationY += (event.clientX - pointerX) * 0.005;
      rotationX += (event.clientY - pointerY) * 0.003;
      pointerX = event.clientX;
      pointerY = event.clientY;
    };

    const onPointerUp = () => {
      isDragging = false;
      container.style.cursor = 'grab';
    };

    container.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);

    return {
      animate: ({ time }) => {
        if (!isDragging) {
          rotationY += 0.0034;
          rotationX += (0.18 + mouse.y * 0.18 - rotationX) * 0.04;
        }

        group.rotation.y = rotationY;
        group.rotation.x = rotationX;

        layerItems.forEach((item, index) => {
          const isActive = index === options.activeIndex;
          item.fill.position.y = item.baseY + Math.sin(time * 0.001 + index * 0.4) * 0.04;
          item.frame.position.y = item.fill.position.y;
          item.marker.position.y = item.fill.position.y + 0.42;
          item.fill.material.opacity += ((isActive ? 0.18 : 0.08) - item.fill.material.opacity) * 0.08;
          item.frame.material.opacity += ((isActive ? 1 : 0.55) - item.frame.material.opacity) * 0.08;
          item.marker.material.opacity += ((isActive ? 1 : 0.45) - item.marker.material.opacity) * 0.08;
        });

        const particleAttribute = particles.geometry.attributes.position;
        for (let index = 0; index < particleCount; index += 1) {
          const yIndex = index * 3 + 1;
          particlePositions[yIndex] += 0.025 + index * 0.0008;
          if (particlePositions[yIndex] > 4.5) {
            particlePositions[yIndex] = -4.5;
          }
        }
        particleAttribute.needsUpdate = true;
      },
      cleanup: () => {
        container.removeEventListener('pointerdown', onPointerDown);
        window.removeEventListener('pointermove', onPointerMove);
        window.removeEventListener('pointerup', onPointerUp);
        container.style.cursor = '';
        cleanupTargets.forEach(disposeObject);
      },
    };
  };
}

export function createAgentSphereScene(agentEntries, options = {}) {
  return (scene, camera, _renderer, mouse, container) => {
    camera.position.set(0, 0, 8.6);
    const cleanupTargets = [];
    const colors = options.colors || {};
    const sphere = new THREE.Group();
    scene.add(sphere);
    cleanupTargets.push(sphere);
    cleanupTargets.push(addStarfield(scene));

    agentEntries.forEach((agent, index) => {
      const phi = Math.acos(1 - (2 * (index + 0.5)) / agentEntries.length);
      const theta = Math.PI * (3 - Math.sqrt(5)) * index;
      const color = colors[agent.level] || 0x00d4ff;
      const dot = new THREE.Mesh(
        new THREE.SphereGeometry(agent.status === 'active' ? 0.09 : 0.07, 10, 10),
        new THREE.MeshBasicMaterial({
          color,
          transparent: true,
          opacity: agent.status === 'active' ? 1 : 0.42,
        }),
      );
      dot.position.set(
        3.1 * Math.sin(phi) * Math.cos(theta),
        3.1 * Math.cos(phi),
        3.1 * Math.sin(phi) * Math.sin(theta),
      );
      sphere.add(dot);
    });

    const shell = new THREE.Mesh(
      new THREE.SphereGeometry(3.15, 28, 28),
      new THREE.MeshBasicMaterial({
        color: 0x00d4ff,
        wireframe: true,
        transparent: true,
        opacity: 0.07,
      }),
    );
    const orbit = new THREE.Mesh(
      new THREE.TorusGeometry(3.8, 0.02, 18, 90),
      new THREE.MeshBasicMaterial({
        color: 0xff6a00,
        transparent: true,
        opacity: 0.65,
      }),
    );
    orbit.rotation.x = Math.PI / 2.5;
    sphere.add(shell, orbit);

    let rotationY = 0.3;
    let rotationX = 0.12;
    let isDragging = false;
    let pointerX = 0;
    let pointerY = 0;

    const onPointerDown = (event) => {
      isDragging = true;
      pointerX = event.clientX;
      pointerY = event.clientY;
      container.style.cursor = 'grabbing';
    };

    const onPointerMove = (event) => {
      if (!isDragging) {
        return;
      }
      rotationY += (event.clientX - pointerX) * 0.0046;
      rotationX += (event.clientY - pointerY) * 0.0032;
      pointerX = event.clientX;
      pointerY = event.clientY;
    };

    const onPointerUp = () => {
      isDragging = false;
      container.style.cursor = 'grab';
    };

    container.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);

    return {
      animate: ({ time }) => {
        if (!isDragging) {
          rotationY += 0.003;
          rotationX += (0.08 + mouse.y * 0.22 - rotationX) * 0.04;
        }
        sphere.rotation.y = rotationY;
        sphere.rotation.x = rotationX;
        orbit.rotation.z += 0.006;
        orbit.rotation.x = Math.PI / 2.5 + Math.sin(time * 0.0006) * 0.06;
      },
      cleanup: () => {
        container.removeEventListener('pointerdown', onPointerDown);
        window.removeEventListener('pointermove', onPointerMove);
        window.removeEventListener('pointerup', onPointerUp);
        container.style.cursor = '';
        cleanupTargets.forEach(disposeObject);
      },
    };
  };
}

export function createComparisonShieldScene() {
  return createModelScene('enterprise', { cameraZ: 6.2, scale: 1.18 });
}

export function createDocsArchiveScene() {
  return (scene, camera, _renderer, mouse) => {
    camera.position.set(0, 0.3, 7.4);
    const cleanupTargets = [];
    cleanupTargets.push(addStarfield(scene));

    const cylinderScene = createCylinderStack();
    const ringsScene = createOrbitalRings();
    cylinderScene.root.scale.setScalar(1.05);
    ringsScene.root.scale.setScalar(0.68);
    ringsScene.root.position.y = 0.1;
    scene.add(cylinderScene.root, ringsScene.root);
    cleanupTargets.push(cylinderScene.root, ringsScene.root);

    return {
      animate: ({ time }) => {
        cylinderScene.root.rotation.x += (mouse.y * 0.2 - cylinderScene.root.rotation.x) * 0.05;
        cylinderScene.root.rotation.z += (mouse.x * 0.12 - cylinderScene.root.rotation.z) * 0.05;
        cylinderScene.animate({ time });
        ringsScene.animate({ time });
      },
      cleanup: () => {
        cleanupTargets.forEach(disposeObject);
      },
    };
  };
}
