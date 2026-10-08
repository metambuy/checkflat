// The save of the sheet in edit mode. The sheet closes as soon as the save has succeeded: the staged
// photos are consumed by then, so leaving it open (e.g. because the refresh that follows failed)
// would let a retry send tokens that no longer exist. A failure of the refresh belongs on the
// plan screen, not in the closed sheet.
export interface SaveFlow {
  save: () => Promise<void>;
  onSaved: () => void;
  refresh: () => Promise<void>;
  /** The save failed: the sheet stays open with the error. */
  onSaveError: (e: unknown) => void;
  /** The save worked but re-reading the list failed: the sheet is already closed. */
  onRefreshError: (e: unknown) => void;
}

/** Returns whether the save itself succeeded. */
export async function saveThenRefresh(f: SaveFlow): Promise<boolean> {
  try {
    await f.save();
  } catch (e) {
    f.onSaveError(e);
    return false;
  }
  f.onSaved();
  try {
    await f.refresh();
  } catch (e) {
    f.onRefreshError(e);
  }
  return true;
}
