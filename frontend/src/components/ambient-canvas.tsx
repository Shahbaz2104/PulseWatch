import { useEffect, useRef } from "react";

interface Blob {
  /** Anchor position as a fraction of viewport width/height. */
  x: number;
  y: number;
  /** Radius as a fraction of the viewport diagonal. */
  r: number;
  /** RGB triple. */
  color: [number, number, number];
  phase: number;
  /** Angular speed for the slow drift orbit (radians/second). */
  speed: number;
  /** Orbit amplitude as a fraction of viewport size. */
  drift: number;
}

/** Slow, low-alpha wash behind the console — chrome is loud, data is calm.
 * Three orbits: emerald (accent), indigo (depth), teal (cool light). */
const BLOBS: Blob[] = [
  { x: 0.18, y: 0.2, r: 0.42, color: [16, 185, 129], phase: 0, speed: 0.11, drift: 0.06 },
  { x: 0.8, y: 0.28, r: 0.5, color: [99, 102, 241], phase: 2.1, speed: 0.08, drift: 0.07 },
  { x: 0.56, y: 0.82, r: 0.46, color: [45, 212, 191], phase: 4.2, speed: 0.09, drift: 0.05 },
];

function isDark(): boolean {
  return document.documentElement.classList.contains("dark");
}

/** Fixed full-viewport canvas aurora. Renders at reduced resolution (the
 * gradients are soft — no detail is lost) and caps at ~30fps; honors
 * `prefers-reduced-motion` with a single static frame and pauses while the
 * tab is hidden. */
export function AmbientCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let width = 0;
    let height = 0;
    let frame = 0;
    let lastDraw = 0;
    let raf = 0;
    let start = performance.now();

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      // 0.5x render scale: gradients are blurry by nature, halving the fill
      // cost keeps this free even on integrated GPUs.
      width = canvas.clientWidth * dpr * 0.5;
      height = canvas.clientHeight * dpr * 0.5;
      canvas.width = Math.max(1, Math.round(width));
      canvas.height = Math.max(1, Math.round(height));
    };

    const draw = (seconds: number) => {
      if (width < 2 || height < 2) return;
      const dark = isDark();
      const alpha = dark ? 0.17 : 0.09;
      const diagonal = Math.hypot(width, height);
      ctx.clearRect(0, 0, width, height);
      for (const blob of BLOBS) {
        const cx = (blob.x + Math.sin(seconds * blob.speed + blob.phase) * blob.drift) * width;
        const cy =
          (blob.y + Math.cos(seconds * blob.speed * 0.9 + blob.phase) * blob.drift) * height;
        const radius = blob.r * diagonal * 0.5;
        const [cr, cg, cb] = blob.color;
        const gradient = ctx.createRadialGradient(cx, cy, 0, cx, cy, radius);
        gradient.addColorStop(0, `rgba(${cr}, ${cg}, ${cb}, ${alpha})`);
        gradient.addColorStop(1, `rgba(${cr}, ${cg}, ${cb}, 0)`);
        ctx.fillStyle = gradient;
        ctx.fillRect(0, 0, width, height);
      }
    };

    const loop = (now: number) => {
      raf = requestAnimationFrame(loop);
      if (now - lastDraw < 33) return; // ~30fps cap
      lastDraw = now;
      if (document.hidden) return;
      draw((now - start) / 1000);
      frame++;
    };

    const onVisibility = () => {
      // Restart the clock after a hidden gap so blobs don't jump.
      if (!document.hidden) start = performance.now() - frame * 33;
    };

    resize();
    const observer = new ResizeObserver(() => {
      resize();
      draw((performance.now() - start) / 1000);
    });
    observer.observe(canvas);

    if (reducedMotion) {
      draw(0);
    } else {
      raf = requestAnimationFrame(loop);
      document.addEventListener("visibilitychange", onVisibility);
    }

    return () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, []);

  return <canvas ref={canvasRef} className="block h-full w-full" aria-hidden="true" />;
}
