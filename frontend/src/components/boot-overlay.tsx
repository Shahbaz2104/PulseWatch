import { useEffect, useRef } from "react";

interface BootOverlayProps {
  /** Called when the sequence finishes (or is skipped) — the parent unmounts. */
  onDone: () => void;
}

const WORDMARK = "PULSEWATCH";

/** Minimal wireframe orb: icosahedron cage + point sparkle, emerald on black.
 * Guarded — any WebGL failure just skips the orb, the overlay still exits. */
function startOrb(THREE: typeof import("three"), canvas: HTMLCanvasElement) {
  let renderer: import("three").WebGLRenderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
  } catch {
    return undefined;
  }
  const size = 168;
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setSize(size, size, false);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(42, 1, 0.1, 10);
  camera.position.z = 3;

  const wireGeometry = new THREE.WireframeGeometry(new THREE.IcosahedronGeometry(1, 1));
  const wire = new THREE.LineSegments(
    wireGeometry,
    new THREE.LineBasicMaterial({ color: 0x10b981, transparent: true, opacity: 0.75 }),
  );
  scene.add(wire);

  const pointGeometry = new THREE.IcosahedronGeometry(1, 2);
  const points = new THREE.Points(
    pointGeometry,
    new THREE.PointsMaterial({ color: 0x6ee7b7, size: 0.05 }),
  );
  scene.add(points);

  let raf = 0;
  const loop = () => {
    raf = requestAnimationFrame(loop);
    wire.rotation.y += 0.009;
    wire.rotation.x += 0.003;
    points.rotation.y -= 0.005;
    renderer.render(scene, camera);
  };
  loop();

  return () => {
    cancelAnimationFrame(raf);
    wireGeometry.dispose();
    pointGeometry.dispose();
    wire.material.dispose();
    points.material.dispose();
    renderer.dispose();
  };
}

/** First-visit-per-session boot: three.js orb + anime.js letter reveal + a
 * GSAP master timeline that hands off to the app. Skippable by click. Not
 * rendered in tests or under `prefers-reduced-motion`. */
export function BootOverlay({ onDone }: BootOverlayProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const orbRef = useRef<HTMLCanvasElement>(null);
  // The parent passes an inline callback; keep it in a ref so the one-shot
  // effect can close over the latest without re-running the sequence.
  const onDoneRef = useRef(onDone);
  useEffect(() => {
    onDoneRef.current = onDone;
  }, [onDone]);

  useEffect(() => {
    let cancelled = false;
    let done = false;
    let disposeOrb: (() => void) | undefined;
    let timeline: ReturnType<typeof import("gsap").default.timeline> | undefined;

    const finish = () => {
      if (done) return;
      done = true;
      timeline?.kill();
      onDoneRef.current();
    };

    (async () => {
      const [gsapModule, animeModule, threeModule] = await Promise.all([
        import("gsap"),
        import("animejs"),
        import("three"),
      ]);
      if (cancelled || done) return;
      const gsap = gsapModule.default;
      const { animate, stagger } = animeModule;

      animate(".boot-letter", {
        translateY: [16, 0],
        opacity: [0, 1],
        delay: stagger(38),
        duration: 540,
        ease: "out(4)",
      });
      animate(".boot-bar", {
        scaleX: [0, 1],
        duration: 1100,
        ease: "inOut(3)",
      });

      if (orbRef.current) disposeOrb = startOrb(threeModule, orbRef.current);

      const content = rootRef.current?.querySelector(".boot-content");
      const shell = document.querySelector(".app-shell");
      timeline = gsap.timeline({ delay: 1.15, onComplete: finish });
      if (content) timeline.to(content, { y: -14, opacity: 0, duration: 0.4, ease: "power2.in" });
      if (rootRef.current) {
        timeline.to(
          rootRef.current,
          { autoAlpha: 0, duration: 0.5, ease: "power2.inOut" },
          "-=0.12",
        );
      }
      if (shell) {
        // Reveal the shell underneath as the curtain lifts (opacity only —
        // no transforms on the shell: it owns sticky children).
        timeline.fromTo(
          shell,
          { opacity: 0.35 },
          { opacity: 1, duration: 0.55, ease: "power2.out" },
          "-=0.42",
        );
      }
    })();

    const root = rootRef.current;
    root?.addEventListener("click", finish);

    return () => {
      cancelled = true;
      root?.removeEventListener("click", finish);
      timeline?.kill();
      disposeOrb?.();
    };
  }, []);

  return (
    <div
      ref={rootRef}
      role="status"
      aria-label="Loading PulseWatch"
      className="fixed inset-0 z-[100] flex cursor-pointer items-center justify-center bg-[#070b16]"
    >
      <div className="boot-content flex flex-col items-center gap-6">
        <canvas ref={orbRef} width={168} height={168} aria-hidden className="block" />
        <p aria-hidden className="font-mono text-lg font-semibold tracking-[0.35em]">
          {WORDMARK.split("").map((letter, index) => (
            <span key={index} className="boot-letter inline-block">
              {letter}
            </span>
          ))}
        </p>
        <div className="flex flex-col items-center gap-3">
          <span className="label-cap text-muted-foreground">Initializing telemetry</span>
          <div className="bg-muted h-0.5 w-40 overflow-hidden rounded-full">
            <div className="boot-bar h-full w-full origin-left scale-x-0 bg-emerald-500" />
          </div>
        </div>
      </div>
    </div>
  );
}
