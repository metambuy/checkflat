// Single-pass `{name}` placeholder substitution. Every placeholder is resolved against the
// original params only, so a value that itself contains "{count}" is never re-substituted.
export type Params = Record<string, string | number>;

export function interpolate(template: string, params?: Params): string {
  if (!params) return template;
  return template.replace(/\{([A-Za-z0-9_]+)\}/g, (match, key: string) =>
    Object.prototype.hasOwnProperty.call(params, key) ? String(params[key]) : match,
  );
}
