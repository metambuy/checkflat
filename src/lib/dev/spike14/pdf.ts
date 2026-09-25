// Sprint 2 viewer spike (D-014): PDF.js loading for both builds, plan sources, logging.
import { invoke } from "@tauri-apps/api/core";
import type { PDFDocumentProxy } from "pdfjs-dist";

export type Build = "legacy" | "modern";
export type PlanKey = "a1" | "a4";
/** Copied by scripts/copy-spike-plan.sh into public/spike/ (gitignored, client data). */
export const PLANS: Record<PlanKey, string> = { a1: "/spike/plan-a1.pdf", a4: "/spike/plan-a4.pdf" };

type Lib = typeof import("pdfjs-dist");
const libs = new Map<Build, Promise<Lib>>();

/** Legacy build works on WebView 124 (D-002); modern needs URL.parse (Chrome 126+). */
export function loadPdfjs(build: Build): Promise<Lib> {
  let p = libs.get(build);
  if (!p) {
    p =
      build === "legacy"
        ? import("pdfjs-dist/legacy/build/pdf.mjs").then((m) => {
            m.GlobalWorkerOptions.workerSrc = new URL("pdfjs-dist/legacy/build/pdf.worker.min.mjs", import.meta.url).toString();
            return m as unknown as Lib;
          })
        : import("pdfjs-dist/build/pdf.mjs").then((m) => {
            m.GlobalWorkerOptions.workerSrc = new URL("pdfjs-dist/build/pdf.worker.min.mjs", import.meta.url).toString();
            return m as unknown as Lib;
          });
    libs.set(build, p);
  }
  return p;
}

export async function fetchPlan(plan: PlanKey): Promise<Uint8Array> {
  const res = await fetch(PLANS[plan]);
  const ct = res.headers.get("content-type") ?? "";
  if (!res.ok || ct.includes("text/html")) throw new Error(`no spike plan ${PLANS[plan]} (run scripts/copy-spike-plan.sh)`);
  const data = new Uint8Array(await res.arrayBuffer());
  if (data.length < 5 || String.fromCharCode(...data.subarray(0, 4)) !== "%PDF") throw new Error(`${PLANS[plan]} is not a PDF`);
  return data;
}

export interface OpenDoc {
  doc: PDFDocumentProxy;
  /** Destroys the loading task: the document, its worker-side state and the worker. */
  destroy(): Promise<void>;
}

export async function openDoc(lib: Lib, data: Uint8Array, pdfBug = false): Promise<OpenDoc> {
  const task = lib.getDocument({
    data,
    pdfBug,
    cMapUrl: "/pdfjs/cmaps/",
    cMapPacked: true,
    standardFontDataUrl: "/pdfjs/standard_fonts/",
    wasmUrl: "/pdfjs/wasm/",
    iccUrl: "/pdfjs/iccs/",
  });
  return { doc: await task.promise, destroy: () => task.destroy() };
}

/** Frees a canvas' backing store now instead of at GC. */
export function releaseCanvas(c: HTMLCanvasElement | null | undefined): void {
  if (!c) return;
  c.width = 0;
  c.height = 0;
  c.remove();
}

export const DPR_CAP = 2;
export const dpr = () => Math.min(window.devicePixelRatio || 1, DPR_CAP);
export const ms = (t0: number) => `${(performance.now() - t0).toFixed(0)} ms`;
export const mb = (bytes: number) => (bytes / 1048576).toFixed(1) + " MB";
export const sleep = (n: number) => new Promise<void>((r) => setTimeout(r, n));
export function heap(): string {
  const m = (performance as any).memory;
  return m ? ` · JS heap ${mb(m.usedJSHeapSize)}` : "";
}

export type Log = (s: string) => void;
/** Pause so spikes/pss.sh (≈1 sample / 1–2 s) records memory at a named point. */
export type Mark = (name: string) => Promise<void>;

/** Lines go to the HUD and, batched, to spike.log (adb-readable on Android; EMUI hides the console). */
export function makeLogger(onLine: (line: string) => void): Log {
  let buf: string[] = [];
  let timer: number | undefined;
  let warned = false;
  const flush = () => {
    const lines = buf;
    buf = [];
    invoke<string>("spike_log", { lines }).catch((e) => {
      if (!warned) { warned = true; onLine(`spike_log failed: ${e}`); }
    });
  };
  return (s: string) => {
    const line = `${new Date().toISOString()} ${s}`;
    console.log("[spike14]", line);
    onLine(line);
    buf.push(line);
    clearTimeout(timer);
    timer = window.setTimeout(flush, 300);
  };
}

export function makeMark(log: Log, pauseMs: () => number): Mark {
  return async (name) => {
    log(`MARK ${name}${heap()}`);
    await sleep(pauseMs());
  };
}
