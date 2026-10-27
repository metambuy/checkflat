import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { PLUGIN_CODES, pluginError } from "./pluginError.ts";
import { errorText, hasOwnText } from "./errors.ts";

const kotlin = readFileSync(fileURLToPath(new URL("../../src-tauri/plugins/camera-capture/android/src/main/java/CameraCapturePlugin.kt", import.meta.url)), "utf8");

test("a rejection the app understands keeps its code, whatever its developer text says", () => {
  const e = { code: "no_camera_app", message: "[no_camera_app] - no camera app available" };
  const lines: string[] = [];
  assert.deepEqual(pluginError(e, (l) => lines.push(l)), e);
  assert.deepEqual(lines, [], "nothing to log: the screen says it all");
  for (const code of PLUGIN_CODES) assert.equal(pluginError({ code, message: "x" }, () => {}).code, code);
});

test("an unknown rejection shows the generic error and its detail goes to the log, not the screen", () => {
  const lines: string[] = [];
  const log = (l: string) => lines.push(l);
  for (const unknown of [
    { code: "internal", message: "capture already in progress" },
    { code: "weird_new_code", message: "boom" },
    "cannot open content://media/picker/0/x", // a bare string, as the dialog plugin rejects
    new Error("this WebView cannot encode WebP"),
    null,
    undefined,
  ]) {
    const shown = pluginError(unknown, log);
    assert.deepEqual(shown, { code: "internal", message: "" }, String(unknown));
  }
  assert.equal(lines.length, 6);
  assert.match(lines[0], /capture already in progress/);
  assert.match(lines[2], /content:\/\/media\/picker/);
  assert.match(lines[3], /WebP/);
  // And the banner then reads just "Unexpected error." (no raw English in brackets).
  const pt = JSON.parse(readFileSync(fileURLToPath(new URL("../../i18n/pt.json", import.meta.url)), "utf8")) as Record<string, string>;
  assert.equal(errorText(pluginError("boom", () => {}), (k) => pt[k]), "Erro inesperado.");
});

test("every code the Kotlin plugin rejects the pick and the camera with is known here and has a message", () => {
  // `invoke.reject(text, NAME)` and `PickFailure(NAME, text)` give a failure its code; NAME is a constant.
  const names = new Set<string>();
  for (const m of kotlin.matchAll(/reject\([^\n]*?,\s*([A-Z_]+)\)/g)) names.add(m[1]);
  for (const m of kotlin.matchAll(/PickFailure\(([A-Z_]+),/g)) names.add(m[1]);
  names.delete("NO_APP"); // the report's viewPdf/sharePdf: the Rust side turns it into no_pdf_app / no_share_app
  assert.ok(names.size >= 5, `found ${[...names].join(", ")}`);
  const codes = [...names].map((n) => {
    const c = kotlin.match(new RegExp(`const val ${n} = "([a-z_]+)"`));
    assert.ok(c, `${n} is a constant with a code`);
    return c[1];
  });
  assert.deepEqual([...codes].sort(), [...PLUGIN_CODES].sort(), "the list in pluginError.ts is the list in CameraCapturePlugin.kt");
  for (const code of codes) assert.ok(hasOwnText(code), code);
});
