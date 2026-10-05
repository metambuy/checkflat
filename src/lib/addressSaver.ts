// The project address field saves itself: a while after typing stops, on blur and on Enter. Saves
// run one at a time; the typed text is never overwritten by a reply and stays in the field if
// saving fails. Pure (the screen passes in the field and the backend call) so it is unit-tested.
export type SaveStatus = "idle" | "saving" | "saved" | "error";

export interface AddressSaverOptions {
  /** Current text of the field. */
  get(): string;
  /** The value last known to be stored; null until the project is loaded. */
  stored(): string | null;
  /** Stores the value; `stored()` returns it afterwards. */
  put(value: string): Promise<void>;
  status(s: SaveStatus): void;
  error(e: unknown): void;
  delayMs?: number;
}

export function addressSaver(o: AddressSaverOptions) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let chain: Promise<void> = Promise.resolve();
  let status: SaveStatus = "idle";
  // The field holds text the user typed that is not stored (yet): waiting, saving, or failed.
  let dirty = false;
  const set = (s: SaveStatus) => {
    status = s;
    o.status(s);
  };

  function save(): Promise<void> {
    clearTimeout(timer);
    timer = undefined;
    chain = chain.then(async () => {
      const value = o.get().trim();
      const stored = o.stored();
      if (stored === null || value === stored) {
        if (stored !== null) dirty = false;
        if (status === "saving") set("saved");
        return;
      }
      set("saving");
      try {
        await o.put(value);
        if (o.get().trim() === o.stored()) {
          dirty = false;
          set("saved");
        } else void save(); // typed more while saving
      } catch (e) {
        set("error");
        o.error(e);
      }
    });
    return chain;
  }

  return {
    /** The user typed: save once typing stops. */
    input() {
      dirty = true;
      set("idle");
      clearTimeout(timer);
      timer = setTimeout(save, o.delayMs ?? 800);
    },
    /** May a value loaded from the backend replace the field's text? Not while the field holds an
     * edit that is unsaved, being saved, or whose save failed. */
    mayReplace(): boolean {
      return !dirty;
    },
    /** Blur or Enter: save now. */
    save,
    /** The screen is going away (Back, Escape, navigation): a save still waiting for typing to
     * stop runs now instead of being dropped. */
    flush(): Promise<void> {
      return timer === undefined ? chain : save();
    },
  };
}
