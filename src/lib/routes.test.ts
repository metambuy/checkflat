import { test } from "node:test";
import assert from "node:assert/strict";
import { listRoute, planFromList } from "./list/rows.ts";
import { parent, type Screen } from "./routes.ts";

const rowB = { id: "obs-b", planId: "plan-b" };

test("Back climbs project → projects, settings → project, dev → projects; the root has no parent", () => {
  assert.equal(parent({ name: "projects" }), null);
  assert.deepEqual(parent({ name: "project", id: "p" }), { name: "projects" });
  assert.deepEqual(parent({ name: "projectSettings", id: "p" }), { name: "project", id: "p" });
  assert.deepEqual(parent({ name: "dev" }), { name: "projects" });
  assert.deepEqual(parent({ name: "plan", projectId: "p", planId: "a" }), { name: "project", id: "p" });
});

test("a list opened from the project screen goes back to the project; opened from a plan, to that plan", () => {
  assert.deepEqual(parent(listRoute("p")), { name: "project", id: "p" });
  assert.deepEqual(parent(listRoute("p", "plan-a")), { name: "plan", projectId: "p", planId: "plan-a" });
});

test("plan A → list → plan B → Back → list (filter and scroll kept) → Back → plan A", () => {
  const list = listRoute("p", "plan-a");
  // The user filtered on fraction B and scrolled before tapping a row of another plan.
  const planB = planFromList(list, rowB, "B", 420);
  assert.deepEqual(planB, {
    name: "plan",
    projectId: "p",
    planId: "plan-b",
    focusId: "obs-b",
    from: { name: "observations", projectId: "p", from: { planId: "plan-a" }, filter: "B", scrollTop: 420 },
  });

  const backToList = parent(planB)!;
  assert.deepEqual(backToList, { name: "observations", projectId: "p", from: { planId: "plan-a" }, filter: "B", scrollTop: 420 });

  const backToA = parent(backToList)!;
  assert.deepEqual(backToA, { name: "plan", projectId: "p", planId: "plan-a" }, "plan A again, without the focus or the recovered draft");
  assert.equal(parent(backToA)!.name, "project", "the chain ends at the project");
});

test("a list opened from the project: tap a row → Back → list → Back → project", () => {
  const list = listRoute("p");
  const plan = planFromList(list, rowB, null, 0);
  const again = parent(plan) as Extract<Screen, { name: "observations" }>;
  assert.equal(again.name, "observations");
  assert.equal(again.from, null);
  assert.deepEqual(parent(again), { name: "project", id: "p" });
});

test("a second jump from the same list replaces the remembered position, it does not stack", () => {
  const list = listRoute("p", "plan-a");
  const first = planFromList(list, rowB, "A", 100);
  const listAgain = parent(first) as Extract<Screen, { name: "observations" }>;
  const second = planFromList(listAgain, { id: "obs-c", planId: "plan-c" }, "A", 250);
  assert.deepEqual(parent(second), { ...list, filter: "A", scrollTop: 250 });
  assert.deepEqual(list, listRoute("p", "plan-a"), "the route object it started from is not modified");
});
