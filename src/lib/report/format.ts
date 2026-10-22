// The latest report's line on the project screen (Sprint 6a, D-028): its date in the format of the
// report's captions and its size as Android shows file sizes (decimal units).
export type Lang = "en" | "pt";

const pad = (n: number) => String(n).padStart(2, "0");

/** `dd.mm.yyyy hh:mm` in the device's local time (the report's caption format). */
export function formatDateTime(d: Date): string {
  return `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/**
 * `17.6 MB` / `17,6 MB` (the language's decimal separator), `640 KB` below 1 MB; a non-breaking
 * space before the unit, so a wrapped line never splits a size. Decimal units
 * (1 MB = 1,000,000 bytes), as Android's file sizes and the mail apps' attachment limits count.
 */
export function formatSize(bytes: number, lang: Lang): string {
  const locale = lang === "pt" ? "pt-PT" : "en-GB";
  const kb = bytes > 0 ? Math.max(1, Math.round(bytes / 1000)) : 0;
  if (kb < 1000) return `${kb}\u00a0KB`;
  const mb = new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(bytes / 1_000_000);
  return `${mb}\u00a0MB`;
}
