import { test } from "node:test";
import assert from "node:assert/strict";
import { revertMove, withPosition } from "./pins.ts";

const pin = (id: string, x: number, y: number) => ({ id, refNo: Number(id), xNorm: x, yNorm: y });

test("a failed move reverts only that pin; pins created or deleted meanwhile stay", () => {
  const before = [pin("3", 0.1, 0.1), pin("5", 0.5, 0.5)];
  let pins = withPosition(before, "3", { x: 0.3, y: 0.3 }); // optimistic
  pins = [...pins, pin("8", 0.8, 0.8)]; // a draft confirmed while the move is pending
  pins = pins.filter((p) => p.id !== "5"); // and a pin deleted
  pins = revertMove(pins, "3", { x: 0.3, y: 0.3 }, { x: 0.1, y: 0.1 });
  assert.deepEqual(pins, [pin("3", 0.1, 0.1), pin("8", 0.8, 0.8)]);
});

test("a later move of the same pin is not undone by an earlier failure", () => {
  let pins = withPosition([pin("3", 0.1, 0.1)], "3", { x: 0.3, y: 0.3 });
  pins = withPosition(pins, "3", { x: 0.6, y: 0.6 }); // second drag
  pins = revertMove(pins, "3", { x: 0.3, y: 0.3 }, { x: 0.1, y: 0.1 });
  assert.deepEqual(pins, [pin("3", 0.6, 0.6)]);
});

test("reverting a pin that was deleted meanwhile changes nothing", () => {
  assert.deepEqual(revertMove([pin("5", 0.5, 0.5)], "3", { x: 0.3, y: 0.3 }, { x: 0.1, y: 0.1 }), [pin("5", 0.5, 0.5)]);
});
