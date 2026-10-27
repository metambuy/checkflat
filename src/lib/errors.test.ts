import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { errorText, hasOwnText } from "./errors.ts";
import { interpolate } from "./interpolate.ts";

const root = fileURLToPath(new URL("../../", import.meta.url));
const read = (rel: string) => readFileSync(join(root, rel), "utf8");
const tables = { en: JSON.parse(read("i18n/en.json")) as Record<string, string>, pt: JSON.parse(read("i18n/pt.json")) as Record<string, string> };
const tr = (lang: "en" | "pt") => (key: string, params?: Record<string, string | number>) => interpolate(tables[lang][key] ?? `<<${key}>>`, params);

/** Every `.rs` file under `dir`, recursively. */
function rustFiles(dir: string): string[] {
  return readdirSync(join(root, dir)).flatMap((n) => {
    const rel = join(dir, n);
    return statSync(join(root, rel)).isDirectory() ? rustFiles(rel) : rel.endsWith(".rs") ? [rel] : [];
  });
}

/** The codes the Rust side can send: `CoreError::code` and the problem enums (`=> "code"`), and `AppError::new("code", …)`. */
function backendCodes(): string[] {
  const codes = new Set<string>();
  for (const m of read("crates/checkflat-core/src/error.rs").matchAll(/=> "([a-z_]+)"/g)) codes.add(m[1]);
  for (const f of rustFiles("src-tauri/src")) for (const m of read(f).matchAll(/AppError::new\(\s*"([a-z_]+)"/g)) codes.add(m[1]);
  return [...codes].sort();
}

/** Codes that never reach a user on purpose: an internal error shows the generic text with its detail. */
const INTERNAL_ONLY = ["internal", "migration", "invalid_path"];

test("every code the backend can send has a message of its own, except the internal ones", () => {
  const codes = backendCodes();
  assert.ok(codes.length > 30, `found the codes: ${codes.join(", ")}`);
  assert.ok(codes.includes("code_invalid_char") && codes.includes("template_unknown_token") && codes.includes("report_busy"), "the scan sees the core and the app");
  const missing = codes.filter((c) => !INTERNAL_ONLY.includes(c) && !hasOwnText(c));
  assert.deepEqual(missing, [], "these codes would show 'Unexpected error'");
  assert.ok(!hasOwnText("internal") && !hasOwnText("something_new"));
});

test("every string the messages use exists in English and Portuguese", () => {
  const codes = backendCodes();
  for (const lang of ["en", "pt"] as const) {
    for (const code of codes) {
      const text = errorText({ code, message: "m", pages: 2, count: 3, detail: "X" }, tr(lang));
      assert.ok(!text.includes("<<"), `${lang} ${code}: ${text}`);
    }
  }
});

test("the unknown-token message names the token as typed and keeps the real tokens literal", () => {
  const e = { code: "template_unknown_token", message: "invalid ref template: unknown token {NOPE}", detail: "NOPE" };
  assert.equal(errorText(e, tr("en")), "Unknown token {NOPE}. Use {PROJ}, {FRAC} or {SEQ:2}.");
  assert.equal(errorText(e, tr("pt")), "Elemento desconhecido {NOPE}. Use {PROJ}, {FRAC} ou {SEQ:2}.");
});

test("code messages state the rule the core applies: no whitespace or braces, at most 16 characters", () => {
  const invalid = { code: "code_invalid_char", message: "invalid code: contains whitespace, braces or a control character" };
  assert.equal(errorText(invalid, tr("pt")), "Sem espaços nem chavetas { } (ex.: 1D, 0.12, PC).");
  assert.equal(errorText(invalid, tr("en")), "No spaces or braces { } (e.g. 1D, 0.12, PC).");
  assert.equal(errorText({ code: "code_too_long", message: "", count: 16 }, tr("pt")), "No máximo 16 caracteres.");
  // The core's English text is never what the user reads.
  for (const code of backendCodes().filter((c) => c.startsWith("code_") || c.startsWith("template_"))) {
    const shown = errorText({ code, message: "RAW CORE TEXT", count: 6, detail: "X" }, tr("pt"));
    assert.ok(!shown.includes("RAW CORE TEXT"), `${code}: ${shown}`);
  }
});

test("the plan-delete message tells the user to delete the observations first, not to move them", () => {
  const e = { code: "plan_has_observations", message: "", count: 2 };
  assert.equal(errorText(e, tr("pt")), "Esta planta tem 2 observação(ões). Elimine-as primeiro.");
  assert.equal(errorText(e, tr("en")), "This plan has 2 observation(s). Delete them first.");
});

test("an unknown error shows the generic text, with its detail only when it has one", () => {
  assert.equal(errorText({ code: "something_new", message: "boom" }, tr("pt")), "Erro inesperado. (boom)");
  assert.equal(errorText({ code: "internal", message: "" }, tr("pt")), "Erro inesperado.");
});
