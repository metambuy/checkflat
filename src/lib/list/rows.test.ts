import { test } from "node:test";
import assert from "node:assert/strict";
import { descriptionStart, filterRows, fractionChips, listRoute, planFromList } from "./rows.ts";

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
