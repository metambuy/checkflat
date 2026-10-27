// Report runs in flight, per project (Sprint 6a, D-028). A run takes ≈ 20 s on the P30 and the user
// may leave the project screen and come back meanwhile: the screen that mounts again asks for the
// running job and shows "Generating…" (with the run's language) until it settles, instead of
// offering a second Generate (the backend would refuse it, `report_busy`). Plain module state,
// outliving the screen's component.
interface Running<I> {
  job: Promise<unknown>;
  info: I;
}
const running = new Map<string, Running<unknown>>();
const watchers = new Set<(anyRunning: boolean) => void>();
const notify = () => watchers.forEach((w) => w(running.size > 0));

/**
 * Called with whether any job is running, now and whenever that changes (D-029: Back on the
 * Projects screen sends the app to the background during a run, instead of closing it and ending
 * the run). Returns the unsubscribe function.
 */
export function watchRunning(w: (anyRunning: boolean) => void): () => void {
  watchers.add(w);
  w(running.size > 0);
  return () => watchers.delete(w);
}

/** Starts `run` for the project unless one is running; returns the running job either way. */
export function startJob<T, I>(projectId: string, info: I, run: () => Promise<T>): Promise<T> {
  const existing = running.get(projectId);
  if (existing) return existing.job as Promise<T>;
  const job: Promise<T> = run().finally(() => {
    if (running.get(projectId)?.job === job) running.delete(projectId);
    notify();
  });
  // A screen that was left may never handle the outcome; that is not an unhandled rejection.
  job.catch(() => {});
  running.set(projectId, { job, info });
  notify();
  return job;
}

/** The project's running job and what it was started with, or null. */
export function runningJob<T, I>(projectId: string): { job: Promise<T>; info: I } | null {
  return (running.get(projectId) as { job: Promise<T>; info: I } | undefined) ?? null;
}

/** What [`followBackend`] needs: the backend's run in progress, and a pause between two looks. */
export interface Backend<L> {
  running: () => Promise<{ projectId: string; lang: L } | null>;
  sleep: (ms: number) => Promise<void>;
}

/** How often a card that found a backend run asks again whether it has ended. */
export const FOLLOW_INTERVAL_MS = 1000;

/**
 * A run this page did not start (D-029): the page was reloaded while the backend went on with the
 * run (an Android activity recreated in a living process), so the in-flight job above is gone with
 * the old page, and so is the run's result. Calls `onStart` with the run's language if the
 * backend is generating this project's report, then waits until that run has ended (its result
 * is in the latest report) or `signal` aborts. Resolves to whether such a run was found. Another
 * project's run is not followed.
 */
export async function followBackend<L>(projectId: string, backend: Backend<L>, signal: AbortSignal, onStart: (lang: L) => void): Promise<boolean> {
  let run = await backend.running();
  if (signal.aborted || run?.projectId !== projectId) return false;
  onStart(run.lang);
  while (run?.projectId === projectId && !signal.aborted) {
    await backend.sleep(FOLLOW_INTERVAL_MS);
    if (signal.aborted) break;
    run = await backend.running();
  }
  return true;
}
