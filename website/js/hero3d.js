(function () {
  const container = document.getElementById("hero-3d");
  if (!container || typeof THREE === "undefined") return;

  const width = () => container.clientWidth || window.innerWidth;
  const height = () => container.clientHeight || window.innerHeight;

  let scene;
  let camera;
  let renderer;
  let frame;
  let networkGroup;
  let haloMesh;
  let nodes = [];
  let lines = [];
  let targetRotationX = 0;
  let targetRotationY = 0;

  const NODE_COUNT = 88;
  const EDGE_LIMIT = 132;
  const SPREAD = 9.6;

  function init() {
    scene = new THREE.Scene();

    camera = new THREE.PerspectiveCamera(52, width() / height(), 0.1, 100);
    camera.position.set(0, 0, 18);

    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.75));
    renderer.setSize(width(), height());
    renderer.setClearColor(0x000000, 0);
    container.appendChild(renderer.domElement);

    networkGroup = new THREE.Group();
    scene.add(networkGroup);

    const ambient = new THREE.AmbientLight(0x66f6ff, 0.52);
    scene.add(ambient);

    const point = new THREE.PointLight(0x00f0ff, 1.8, 40);
    point.position.set(0, 0, 10);
    scene.add(point);

    buildHalo();
    buildNodes();
    buildLines();

    window.addEventListener("resize", onResize);
    window.addEventListener("mousemove", onMouseMove, { passive: true });
    animate();
  }

  function buildHalo() {
    const geometry = new THREE.RingGeometry(3.8, 4.5, 64);
    const material = new THREE.MeshBasicMaterial({
      color: 0x00f0ff,
      transparent: true,
      opacity: 0.08,
      side: THREE.DoubleSide,
      blending: THREE.AdditiveBlending
    });

    haloMesh = new THREE.Mesh(geometry, material);
    haloMesh.rotation.x = Math.PI / 2.1;
    scene.add(haloMesh);
  }

  function buildNodes() {
    const sprites = [];

    for (let index = 0; index < NODE_COUNT; index += 1) {
      const elevated = index < 8;
      const geometry = new THREE.SphereGeometry(elevated ? 0.12 : 0.06 + Math.random() * 0.045, 10, 10);
      const material = new THREE.MeshBasicMaterial({
        color: elevated ? 0x7effd7 : 0x00f0ff,
        transparent: true,
        opacity: elevated ? 0.95 : 0.36 + Math.random() * 0.26
      });
      const mesh = new THREE.Mesh(geometry, material);
      mesh.position.set(
        (Math.random() - 0.5) * SPREAD,
        (Math.random() - 0.5) * SPREAD * 0.76,
        (Math.random() - 0.5) * SPREAD * 0.62
      );
      mesh.userData = {
        pulseSpeed: 0.7 + Math.random() * 1.4,
        pulseOffset: Math.random() * Math.PI * 2,
        baseOpacity: material.opacity
      };
      nodes.push(mesh);
      networkGroup.add(mesh);

      if (elevated) {
        const spriteMaterial = new THREE.SpriteMaterial({
          color: 0x00f0ff,
          transparent: true,
          opacity: 0.12,
          blending: THREE.AdditiveBlending
        });
        const sprite = new THREE.Sprite(spriteMaterial);
        sprite.scale.set(1.2, 1.2, 1.2);
        sprite.position.copy(mesh.position);
        sprites.push(sprite);
        networkGroup.add(sprite);
      }
    }

    const pointsGeometry = new THREE.BufferGeometry().setFromPoints(nodes.map((node) => node.position));
    const pointsMaterial = new THREE.PointsMaterial({
      color: 0x00f0ff,
      size: 0.05,
      transparent: true,
      opacity: 0.12,
      blending: THREE.AdditiveBlending
    });
    const points = new THREE.Points(pointsGeometry, pointsMaterial);
    networkGroup.add(points);
  }

  function buildLines() {
    const pairs = [];
    for (let i = 0; i < nodes.length; i += 1) {
      for (let j = i + 1; j < nodes.length; j += 1) {
        const distance = nodes[i].position.distanceTo(nodes[j].position);
        pairs.push({ i, j, distance });
      }
    }

    pairs.sort((left, right) => left.distance - right.distance);

    for (let index = 0; index < pairs.length && lines.length < EDGE_LIMIT; index += 1) {
      const pair = pairs[index];
      if (pair.distance > 3.2) break;

      const geometry = new THREE.BufferGeometry().setFromPoints([
        nodes[pair.i].position.clone(),
        nodes[pair.j].position.clone()
      ]);
      const material = new THREE.LineBasicMaterial({
        color: pair.distance < 1.8 ? 0x22ffe4 : 0x00c8d4,
        transparent: true,
        opacity: 0.06 + Math.random() * 0.05,
        blending: THREE.AdditiveBlending
      });
      const line = new THREE.Line(geometry, material);
      line.userData = {
        pulse: 0.9 + Math.random() * 1.5,
        offset: Math.random() * Math.PI * 2,
        baseOpacity: material.opacity
      };
      lines.push(line);
      networkGroup.add(line);
    }
  }

  function onMouseMove(event) {
    const normalizedX = event.clientX / window.innerWidth - 0.5;
    const normalizedY = event.clientY / window.innerHeight - 0.5;
    targetRotationY = normalizedX * 0.58;
    targetRotationX = normalizedY * 0.24;
  }

  function onResize() {
    if (!camera || !renderer) return;
    camera.aspect = width() / height();
    camera.updateProjectionMatrix();
    renderer.setSize(width(), height());
  }

  function animate() {
    frame = requestAnimationFrame(animate);
    const time = performance.now() * 0.001;

    networkGroup.rotation.y += 0.0009;
    networkGroup.rotation.z += 0.00018;
    networkGroup.rotation.y += (targetRotationY - networkGroup.rotation.y) * 0.035;
    networkGroup.rotation.x += (-targetRotationX - networkGroup.rotation.x) * 0.04;

    nodes.forEach((node) => {
      const { pulseSpeed, pulseOffset, baseOpacity } = node.userData;
      node.material.opacity = baseOpacity * (0.74 + 0.32 * Math.sin(time * pulseSpeed + pulseOffset));
    });

    lines.forEach((line) => {
      const { pulse, offset, baseOpacity } = line.userData;
      line.material.opacity = baseOpacity * (0.7 + 0.28 * Math.sin(time * pulse + offset));
    });

    if (haloMesh) {
      haloMesh.rotation.z += 0.0012;
      haloMesh.material.opacity = 0.06 + Math.sin(time * 0.8) * 0.02;
    }

    renderer.render(scene, camera);
  }

  init();

  window.addEventListener("beforeunload", () => {
    if (frame) cancelAnimationFrame(frame);
    window.removeEventListener("resize", onResize);
    window.removeEventListener("mousemove", onMouseMove);
  });
})();
