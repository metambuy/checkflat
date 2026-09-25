// Tiny EN/PT helper over the shared i18n/*.json files (same files Rust reads for reports).
// Language is persisted in the `setting` table (key "language") through the backend.
import en from "../../i18n/en.json";
import pt from "../../i18n/pt.json";
import { invoke } from "@tauri-apps/api/core";
import { interpolate, type Params } from "./interpolate";

export type Lang = "en" | "pt";
const tables: Record<Lang, Record<string, string>> = { en, pt };

const state = $state<{ lang: Lang }>({ lang: navigator.language?.toLowerCase().startsWith("pt") ? "pt" : "en" });

export function lang(): Lang {
  return state.lang;
}

export function t(key: string, params?: Params): string {
  return interpolate(tables[state.lang][key] ?? tables.en[key] ?? key, params);
}

export async function loadLang(): Promise<void> {
  try {
    const saved = await invoke<string | null>("get_setting", { key: "language" });
    if (saved === "en" || saved === "pt") state.lang = saved;
  } catch (e) {
    console.warn("language setting unavailable", e);
  }
}

export async function setLang(l: Lang): Promise<void> {
  state.lang = l;
  try {
    await invoke("set_setting", { key: "language", value: l });
  } catch (e) {
    console.warn("could not persist language", e);
  }
}
