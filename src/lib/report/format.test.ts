import { test } from "node:test";
import assert from "node:assert/strict";
import { formatDateTime, formatSize } from "./format.ts";

test("date and time as the report's captions, in local time, zero-padded", () => {
  assert.equal(formatDateTime(new Date(2026, 9, 22, 14, 5)), "22.10.2026 14:05");
  assert.equal(formatDateTime(new Date(2026, 0, 3, 9, 7)), "03.01.2026 09:07");
  assert.equal(formatDateTime(new Date(2026, 11, 31, 0, 0)), "31.12.2026 00:00");
});

test("sizes in decimal units with the language's decimal separator", () => {
  assert.equal(formatSize(17_612_345, "en"), "17.6 MB");
  assert.equal(formatSize(17_612_345, "pt"), "17,6 MB");
  assert.equal(formatSize(1_000_000, "en"), "1.0 MB");
  assert.equal(formatSize(999_499, "en"), "999 KB");
  assert.equal(formatSize(999_500, "en"), "1.0 MB", "no '1000 KB'");
  assert.equal(formatSize(640_000, "pt"), "640 KB");
  assert.equal(formatSize(300, "en"), "1 KB", "a non-empty file is at least 1 KB");
  assert.equal(formatSize(0, "en"), "0 KB");
});
