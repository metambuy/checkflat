import { test } from "node:test";
import assert from "node:assert/strict";
import { descriptionStart, filterRows, fractionChips, listRoute, planFromList, thumbBatches, withThumbs } from "./rows.ts";

const rows = [
  { id: "1", fraction: "A" },
  { id: "2", fraction: "B" },
  { id: "3", fraction: "" },
  { id: "4", fraction: "A" },
];

test("the filter keeps the rows of one fraction in their order; null keeps all", () => {
  assert.deepEqual(filterRows(rows, null).map((r) => r.id), ["1", "2", "3", "4"]);
  assert.deepEqual(filterRows(rows, "A").map((r) => r.id), ["1", "4"]);
  assert.deepEqual(filterRows(rows, "Z"), []);
  assert.deepEqual(filterRows(rows, ""), [rows[2]], "an empty code selects the rows without a fraction");
});

test("chips: the project's fractions first, then codes only the rows carry; no empty chip, no duplicates", () => {
  assert.deepEqual(fractionChips(["A", "B"], rows), ["A", "B"]);
  assert.deepEqual(fractionChips(["B"], rows), ["B", "A"], "A was deleted from the list but still has observations");
  assert.deepEqual(fractionChips([], []), []);
  assert.deepEqual(fractionChips(["PC"], [{ fraction: "X" }, { fraction: "X" }]), ["PC", "X"]);
});

test("description start: one line, whitespace collapsed, cut with an ellipsis", () => {
  assert.equal(descriptionStart("  Fissura\n\nna   parede \t da sala "), "Fissura na parede da sala");
  assert.equal(descriptionStart(""), "");
  const long = "x".repeat(200);
  const cut = descriptionStart(long, 80);
  assert.equal(Array.from(cut).length, 80);
  assert.ok(cut.endsWith("…"));
  assert.equal(descriptionStart("a".repeat(80), 80), "a".repeat(80), "exactly the limit stays whole");
  assert.equal(descriptionStart("ação ".repeat(30), 10), "ação ação…", "never cuts inside a character; trailing space dropped");
});

test("the routes in and out of the list", () => {
  assert.deepEqual(listRoute("p"), { name: "observations", projectId: "p", from: null, filter: null, scrollTop: 0 });
  assert.deepEqual(listRoute("p", "pl", "A", 12), { name: "observations", projectId: "p", from: { planId: "pl" }, filter: "A", scrollTop: 12 });
  const plan = planFromList(listRoute("p"), { id: "o", planId: "pl" }, "A", 5);
  assert.equal(plan.name, "plan");
  if (plan.name === "plan") {
    assert.equal(plan.focusId, "o");
    assert.equal(plan.from?.filter, "A");
    assert.equal(plan.from?.scrollTop, 5);
  }
});

const thumbRow = (id: string, photoId: string | null, thumbPath: string | null) => ({ id, photoId, thumbPath });

test("thumbnails are asked for in list order, in batches, only for rows that have a photo and no thumbnail yet", () => {
  const rows = [
    thumbRow("1", "p1", null),
    thumbRow("2", null, null), // no photo: nothing to make
    thumbRow("3", "p3", "/t/p3.thumb.jpg"), // already made
    thumbRow("4", "p4", null),
    thumbRow("5", "p5", null),
    thumbRow("6", "p6", null),
  ];
  assert.deepEqual(thumbBatches(rows, 2), [["p1", "p4"], ["p5", "p6"]]);
  assert.deepEqual(thumbBatches(rows, 4), [["p1", "p4", "p5", "p6"]]);
  assert.deepEqual(thumbBatches(rows.slice(2, 3)), [], "nothing to make: no call at all");
  assert.deepEqual(thumbBatches([]), []);
  assert.equal(thumbBatches(Array.from({ length: 9 }, (_, i) => thumbRow(String(i), `p${i}`, null))).length, 3, "default batch of 4");
});

test("arrived thumbnails go to their rows; the others, and rows without a photo, are untouched", () => {
  const rows = [thumbRow("1", "p1", null), thumbRow("2", null, null), thumbRow("3", "p3", null)];
  const next = withThumbs(rows, [{ photoId: "p3", path: "/t/p3.thumb.jpg" }, { photoId: "ghost", path: "/t/x" }]);
  assert.deepEqual(next, [thumbRow("1", "p1", null), thumbRow("2", null, null), thumbRow("3", "p3", "/t/p3.thumb.jpg")]);
  assert.equal(next[0], rows[0], "an untouched row is the same object");
  assert.equal(withThumbs(rows, []), rows, "nothing arrived: the same array, no re-render");
  assert.deepEqual(rows[2], thumbRow("3", "p3", null), "the input is not modified");
});
