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

/** Starts `run` for the project unless one is running; returns the running job either way. */
export function startJob<T, I>(projectId: string, info: I, run: () => Promise<T>): Promise<T> {
  const existing = running.get(projectId);
  if (existing) return existing.job as Promise<T>;
  const job: Promise<T> = run().finally(() => {
    if (running.get(projectId)?.job === job) running.delete(projectId);
  });
  // A screen that was left may never handle the outcome; that is not an unhandled rejection.
  job.catch(() => {});
  running.set(projectId, { job, info });
  return job;
}

/** The project's running job and what it was started with, or null. */
export function runningJob<T, I>(projectId: string): { job: Promise<T>; info: I } | null {
  return (running.get(projectId) as { job: Promise<T>; info: I } | undefined) ?? null;
}
