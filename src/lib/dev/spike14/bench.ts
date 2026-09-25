// Same scripted sequence for both designs: ready → 8× at the centre → 4 s programmatic pan → settle.
import type { Transform } from "../gestures";
import type { Log, Mark } from "./pdf";

export interface BenchTarget {
  zoomTo(z: number): Promise<void>; // resolves when the view is sharp
  get(): Transform;
  set(t: Transform): void;
  settle(): Promise<void>;
  width(): number;
}

export async function runBench(target: BenchTarget, log: Log, mark: Mark): Promise<void> {
  await mark("ready");
  let t = performance.now();
  await target.zoomTo(8);
  log(`8× sharp after ${(performance.now() - t).toFixed(0)} ms`);
  await mark("at8");

  const t0 = target.get();
  const amp = target.width() * 0.75;
  const deltas: number[] = [];
  const start = performance.now();
  let last = start;
  await new Promise<void>((done) => {
    const step = (now: number) => {
      deltas.push(now - last);
      last = now;
      const k = (now - start) / 4000;
      if (k >= 1) return done();
      target.set({ ...t0, x: t0.x + amp * Math.sin(k * 2 * Math.PI), y: t0.y + (amp / 2) * Math.sin(k * 4 * Math.PI) });
      requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  });
  deltas.shift();
  const sorted = [...deltas].sort((a, b) => a - b);
  const q = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))].toFixed(1);
  log(`pan 4 s: ${(deltas.length / 4).toFixed(0)} fps, frame median ${q(0.5)} p95 ${q(0.95)} max ${q(1)} ms, >34 ms: ${deltas.filter((d) => d > 34).length}`);
  t = performance.now();
  await target.settle();
  log(`settled after pan in ${(performance.now() - t).toFixed(0)} ms`);
  await mark("afterpan");
  log("END bench");
}
