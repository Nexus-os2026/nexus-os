(function () {
  const canvas = document.getElementById("particles-canvas");
  if (!canvas) return;

  const context = canvas.getContext("2d");
  const particles = [];
  const mouse = { x: -9999, y: -9999, active: false };
  const particleCount = 72;
  const connectionDistance = 138;
  const mouseDistance = 170;

  function resize() {
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
  }

  function createParticle() {
    return {
      x: Math.random() * canvas.width,
      y: Math.random() * canvas.height,
      vx: (Math.random() - 0.5) * 0.28,
      vy: (Math.random() - 0.5) * 0.28,
      radius: Math.random() * 1.8 + 0.7,
      alpha: Math.random() * 0.28 + 0.06
    };
  }

  function reset() {
    particles.length = 0;
    for (let index = 0; index < particleCount; index += 1) {
      particles.push(createParticle());
    }
  }

  function drawConnections() {
    for (let i = 0; i < particles.length; i += 1) {
      const a = particles[i];

      for (let j = i + 1; j < particles.length; j += 1) {
        const b = particles[j];
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        const distance = Math.hypot(dx, dy);
        if (distance > connectionDistance) continue;

        const alpha = ((connectionDistance - distance) / connectionDistance) * 0.12;
        context.beginPath();
        context.moveTo(a.x, a.y);
        context.lineTo(b.x, b.y);
        context.strokeStyle = `rgba(0, 240, 255, ${alpha})`;
        context.lineWidth = 0.65;
        context.stroke();
      }

      if (!mouse.active) continue;
      const mdx = a.x - mouse.x;
      const mdy = a.y - mouse.y;
      const mouseGap = Math.hypot(mdx, mdy);
      if (mouseGap > mouseDistance) continue;

      const alpha = ((mouseDistance - mouseGap) / mouseDistance) * 0.18;
      context.beginPath();
      context.moveTo(a.x, a.y);
      context.lineTo(mouse.x, mouse.y);
      context.strokeStyle = `rgba(0, 255, 136, ${alpha})`;
      context.lineWidth = 0.7;
      context.stroke();
    }
  }

  function drawParticles() {
    particles.forEach((particle) => {
      context.beginPath();
      context.arc(particle.x, particle.y, particle.radius, 0, Math.PI * 2);
      context.fillStyle = `rgba(190, 245, 255, ${particle.alpha})`;
      context.fill();
    });
  }

  function update() {
    context.clearRect(0, 0, canvas.width, canvas.height);

    particles.forEach((particle) => {
      particle.x += particle.vx;
      particle.y += particle.vy;

      if (particle.x < -20) particle.x = canvas.width + 20;
      if (particle.x > canvas.width + 20) particle.x = -20;
      if (particle.y < -20) particle.y = canvas.height + 20;
      if (particle.y > canvas.height + 20) particle.y = -20;
    });

    drawConnections();
    drawParticles();
    requestAnimationFrame(update);
  }

  window.addEventListener("mousemove", (event) => {
    mouse.x = event.clientX;
    mouse.y = event.clientY;
    mouse.active = true;
  }, { passive: true });

  window.addEventListener("mouseleave", () => {
    mouse.active = false;
  });

  window.addEventListener("resize", () => {
    resize();
    reset();
  });

  resize();
  reset();
  update();
})();
