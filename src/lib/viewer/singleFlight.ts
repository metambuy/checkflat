// One run at a time per key (tile generation per plan, D-014): a new run asks the previous one to
// stop and waits until it has fully settled, including its cleanup, before it starts. Without this,
// leaving a plan mid-generation and reopening it at once had two PDF.js documents alive (~780 MB
// each for an A1 plan).
interface Run {
  stop: boolean;
  done: Promise<void>;
}

const runs = new Map<string, Run>();

/** Runs `run` once every earlier run for `key` has settled. `superseded()` turns true when a later
 * call for the same key is waiting; the run should then return as soon as it can. */
export async function singleFlight<T>(key: string, run: (superseded: () => boolean) => Promise<T>): Promise<T> {
  const prev = runs.get(key);
  let release!: () => void;
  const mine: Run = { stop: false, done: new Promise<void>((r) => (release = r)) };
  runs.set(key, mine); // before any await, so a third caller queues behind this one
  if (prev) {
    prev.stop = true;
    await prev.done;
  }
  try {
    return await run(() => mine.stop);
  } finally {
    if (runs.get(key) === mine) runs.delete(key);
    release();
  }
}
