// Pointer-event pan / pinch / wheel handling for the plan viewer.
// Pure input → transform; no rendering here. The viewer owns the transform state.

export interface Transform {
  x: number; // css px translate
  y: number;
  scale: number; // css scale applied to the base canvas
}

export interface GestureCallbacks {
  get(): Transform;
  set(t: Transform): void;
  /** Called after the gesture settles (pointer up / wheel idle). */
  settled(): void;
  minScale(): number;
  maxScale(): number;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Zoom around a fixed point in element coordinates. */
export function zoomAt(t: Transform, factor: number, px: number, py: number, lo: number, hi: number): Transform {
  const scale = clamp(t.scale * factor, lo, hi);
  const f = scale / t.scale;
  return { scale, x: px - (px - t.x) * f, y: py - (py - t.y) * f };
}

export function attachGestures(el: HTMLElement, cb: GestureCallbacks): () => void {
  const pointers = new Map<number, { x: number; y: number }>();
  let start: { t: Transform; cx: number; cy: number; dist: number } | null = null;
  let settleTimer: number | undefined;

  const local = (e: PointerEvent | WheelEvent) => {
    const r = el.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const centroid = () => {
    let x = 0, y = 0;
    for (const p of pointers.values()) { x += p.x; y += p.y; }
    return { x: x / pointers.size, y: y / pointers.size };
  };
  const spread = () => {
    if (pointers.size < 2) return 0;
    const [a, b] = [...pointers.values()];
    return Math.hypot(a.x - b.x, a.y - b.y);
  };
  const begin = () => {
    const c = centroid();
    start = { t: cb.get(), cx: c.x, cy: c.y, dist: spread() };
  };
  const scheduleSettle = () => {
    clearTimeout(settleTimer);
    settleTimer = window.setTimeout(() => cb.settled(), 150);
  };

  const down = (e: PointerEvent) => {
    if (e.pointerType === "mouse" && e.button !== 0) return;
    el.setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, local(e));
    begin();
    clearTimeout(settleTimer);
  };
  const move = (e: PointerEvent) => {
    if (!pointers.has(e.pointerId) || !start) return;
    pointers.set(e.pointerId, local(e));
    const c = centroid();
    let t: Transform = { ...start.t, x: start.t.x + (c.x - start.cx), y: start.t.y + (c.y - start.cy) };
    if (pointers.size >= 2 && start.dist > 0) {
      const factor = spread() / start.dist;
      // scale around the current centroid, keeping the pan applied above
      t = zoomAt(t, factor, c.x, c.y, cb.minScale(), cb.maxScale());
    }
    cb.set(t);
  };
  const up = (e: PointerEvent) => {
    if (!pointers.has(e.pointerId)) return;
    pointers.delete(e.pointerId);
    if (pointers.size > 0) begin(); else { start = null; scheduleSettle(); }
  };
  const wheel = (e: WheelEvent) => {
    e.preventDefault();
    const p = local(e);
    const factor = Math.exp(-e.deltaY * (e.deltaMode === 1 ? 0.05 : 0.0015));
    cb.set(zoomAt(cb.get(), factor, p.x, p.y, cb.minScale(), cb.maxScale()));
    scheduleSettle();
  };

  el.addEventListener("pointerdown", down);
  el.addEventListener("pointermove", move);
  el.addEventListener("pointerup", up);
  el.addEventListener("pointercancel", up);
  el.addEventListener("wheel", wheel, { passive: false });
  return () => {
    el.removeEventListener("pointerdown", down);
    el.removeEventListener("pointermove", move);
    el.removeEventListener("pointerup", up);
    el.removeEventListener("pointercancel", up);
    el.removeEventListener("wheel", wheel);
    clearTimeout(settleTimer);
  };
}
