// Report runs in flight, per project (Sprint 6a, D-028). A run takes ≈ 20 s on the P30 and the user
// may leave the project screen and come back meanwhile: the screen that mounts again asks for the
// running job and shows "Generating…" until it settles, instead of offering a second Generate (the
// backend would refuse it, `report_busy`). Plain module state, outliving the screen's component.
const running = new Map<string, Promise<unknown>>();

/** Starts `run` for the project unless one is running; returns the running job either way. */
export function startJob<T>(projectId: string, run: () => Promise<T>): Promise<T> {
  const existing = running.get(projectId);
  if (existing) return existing as Promise<T>;
  const job: Promise<T> = run().finally(() => {
    if (running.get(projectId) === job) running.delete(projectId);
  });
  // A screen that was left may never handle the outcome; that is not an unhandled rejection.
  job.catch(() => {});
  running.set(projectId, job);
  return job;
}

/** The project's running job, or null. */
export function runningJob<T>(projectId: string): Promise<T> | null {
  return (running.get(projectId) as Promise<T> | undefined) ?? null;
}
