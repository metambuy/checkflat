// Live "next ref" preview (observation sheet, project settings): recompute a while after the
// input stops changing, and never show a reply that belongs to an older input. Pure (the
// backend call is passed in) so it is unit-tested.
export interface LivePreviewOptions<I> {
  compute(input: I): Promise<string>;
  /** Called with the text, or with the error when `compute` rejected. */
  show(text: string | null, error: unknown): void;
  delayMs?: number;
}

export function livePreview<I>(o: LivePreviewOptions<I>) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let seq = 0;
  async function run(input: I): Promise<void> {
    const mine = ++seq;
    try {
      const text = await o.compute(input);
      if (mine === seq) o.show(text, null);
    } catch (e) {
      if (mine === seq) o.show(null, e);
    }
  }
  return {
    /** Schedules a recompute; an earlier pending one is dropped. */
    update(input: I): void {
      clearTimeout(timer);
      timer = setTimeout(() => void run(input), o.delayMs ?? 250);
    },
    /** Recomputes now (initial value). */
    now(input: I): Promise<void> {
      clearTimeout(timer);
      return run(input);
    },
    cancel(): void {
      clearTimeout(timer);
      seq++;
    },
  };
}
