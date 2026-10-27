// What an error says to the user, by its stable `code` (src-tauri/src/error.rs, the core's
// `CoreError::code`, the plugin's rejections). One place for the banner and for the messages under a
// field, so a code never shows the core's English text. Pure: `t` is passed in, so it is unit-tested
// (errors.test.ts also checks every code the Rust side can emit against this table and the strings).
import type { AppError } from "./api";
import type { Params } from "./interpolate";

export type Translate = (key: string, params?: Params) => string;

/** Template problems whose message needs nothing from the error (`error.<code>`). */
const TEMPLATE_PLAIN = new Set([
  "template_empty",
  "template_stray_close",
  "template_unclosed",
  "template_seq_width_nan",
  "template_seq_width_range",
  "template_needs_one_seq",
  "template_repeated_token",
  "template_frac_required",
]);

export function errorText(e: AppError, t: Translate): string {
  if (TEMPLATE_PLAIN.has(e.code)) return t(`error.${e.code}`);
  switch (e.code) {
    case "multi_page": return t("import.multi_page", { pages: e.pages ?? "?" });
    case "unreadable_pdf": return t("import.unreadable_pdf");
    case "source_unreadable": return t("import.source_unreadable");
    case "plan_has_observations": return t("plan.delete_blocked", { count: e.count ?? "?" });
    case "code_invalid_char": return t("error.code_invalid_char");
    case "code_too_long": return t("error.code_too_long", { max: e.count ?? "?" });
    case "template_too_long": return t("error.template_too_long", { max: e.count ?? "?" });
    case "template_unknown_token": return t("error.template_unknown_token", { token: e.detail ? `{${e.detail}}` : "?" });
    case "scope_locked": return t("error.scope_locked");
    case "fraction_in_use": return t("error.fraction_in_use");
    case "fraction_required": return t("error.fraction_required");
    case "photo_required": return t("error.photo_required");
    case "photo_limit": return t("error.photo_limit", { count: e.count ?? "?" });
    case "unreadable_image": return t("error.unreadable_image");
    case "image_unsupported": return t("error.image_unsupported");
    case "report": return t("error.report", { message: e.message });
    case "report_busy": return t("error.report_busy");
    case "no_report": return t("error.no_report");
    case "no_pdf_app": return t("error.no_pdf_app");
    case "no_share_app": return t("error.no_share_app");
    case "save_failed": return t("error.save_failed");
    case "validation": return t("error.validation");
    case "not_found": return t("error.not_found");
    case "db": return t("error.db");
    case "io": return t("error.io");
    default: return e.message ? `${t("error.internal")} (${e.message})` : t("error.internal");
  }
}

/** Whether `errorText` has a message of its own for this code (the rest says "Unexpected error"). */
export function hasOwnText(code: string): boolean {
  const generic = "\u0000generic";
  return errorText({ code, message: "" }, (key) => (key === "error.internal" ? generic : key)) !== generic;
}
