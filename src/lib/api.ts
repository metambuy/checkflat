// Typed wrappers over Tauri commands. Errors are { code, message, pages?, count? } (src-tauri/src/error.rs).
import { invoke } from "@tauri-apps/api/core";
import type { Annotation } from "./annotate/model";

export interface AppError {
  code: string;
  message: string;
  pages?: number;
  count?: number;
}
export interface ProjectSummary {
  id: string;
  name: string;
  address: string;
  planCount: number;
  createdAt: string;
  updatedAt: string;
}
export type Scope = "project" | "fraction";
export interface Project {
  id: string;
  name: string;
  address: string;
  /** Project code shown by {PROJ} in the ref template (may be empty). */
  code: string;
  /** Ref template, e.g. "{PROJ}-{FRAC}-{SEQ:2}". */
  refTemplate: string;
  /** Whether sequence numbers run per project or per fraction (D-020). */
  seqScope: Scope;
  nextSeq: number;
  /** The scope can no longer change: a number has been issued (even if every pin was deleted since). */
  scopeLocked: boolean;
  observationCount: number;
  createdAt: string;
  updatedAt: string;
}
export interface Fraction {
  id: string;
  projectId: string;
  code: string;
  nextSeq: number;
  createdAt: string;
}
export interface Plan {
  id: string;
  projectId: string;
  filePath: string;
  title: string;
  widthPt: number;
  heightPt: number;
  createdAt: string;
}
export interface PdfInfo {
  pageCount: number;
  widthPt: number;
  heightPt: number;
}
export interface StagedPlan {
  token: string;
  displayName: string | null;
  /** null when the source has no usable name; use t("plan.default_title") */
  defaultTitle: string | null;
  info: PdfInfo;
}

export interface Observation {
  id: string;
  projectId: string;
  planId: string;
  /** Fraction code (canonical spelling); "" when none. */
  fraction: string;
  seq: number;
  /** Full displayed ref, computed by the backend from the project's template (e.g. "LAM-A-03"). */
  ref: string;
  /** Short pin-marker label: "3" (numbers per project) or "A-03" (per fraction). */
  marker: string;
  xNorm: number;
  yNorm: number;
  description: string;
  createdVisitId: string | null;
  resolvedVisitId: string | null;
  resolvedAt: string | null;
  createdAt: string;
  updatedAt: string;
}
/**
 * A row of the observation list: the observation, its plan's title and its first photo. `photoId` is
 * null for an observation without a photo; `thumbPath` is the photo's small list thumbnail if it has
 * been made — null while it has not (ask for it with `ensureThumbnails`); `thumbAnnotation` is drawn
 * over it by the page, never baked into the file.
 */
export interface ObservationRow extends Observation {
  planTitle: string;
  photoId: string | null;
  thumbPath: string | null;
  thumbAnnotation: Annotation | null;
}
/** A list thumbnail that exists (absolute path, for convertFileSrc). */
export interface ThumbReady {
  photoId: string;
  path: string;
}
/** A stored photo; `path` is absolute, for convertFileSrc (asset protocol, `$APPDATA/projects/**`). */
export interface Photo {
  id: string;
  observationId: string;
  path: string;
  /** UTC, RFC 3339. */
  takenAt: string;
  /** The shapes drawn on it (Sprint 4); null when nothing is drawn. */
  annotation: Annotation | null;
}
/** A processed photo (≤ 1600 px JPEG) waiting for the observation save; `path` is under `$APPDATA/tmp/**`. */
export interface StagedPhoto {
  token: string;
  path: string;
  takenAt: string;
  /** Drawn in the sheet before the save; null until then (the backend never sets it). */
  annotation: Annotation | null;
}
/** What the save takes for each new photo. */
export interface PhotoInput {
  token: string;
  takenAt: string;
  annotation: Annotation | null;
}
/** A stored photo's new annotation (edit mode); null clears it. */
export interface PhotoAnnotation {
  photoId: string;
  annotation: Annotation | null;
}
export interface TileLevel {
  /** Long side in px; the level's directory name. */
  size: number;
  width: number;
  height: number;
  cols: number;
  rows: number;
}
export interface TileManifest {
  version: number;
  tile: number;
  widthPt: number;
  heightPt: number;
  /** Finished levels, smallest first. */
  levels: TileLevel[];
}
export interface TileInfo {
  /** Absolute paths for convertFileSrc (asset protocol, scope $APPDATA/projects/**). */
  dir: string;
  pdf: string;
  manifest: TileManifest | null;
}

/** A project's latest report (`reports/<project>.json`, Sprint 6a, D-028). */
export interface ReportInfo {
  v: number;
  /** The stored file's name (internal). */
  file: string;
  /** What other apps show: "<project> - yyyy-mm-dd - PT.pdf". */
  name: string;
  lang: "en" | "pt";
  pages: number;
  bytes: number;
  /** UTC, RFC 3339: when it was generated. */
  createdAt: string;
}
/** What `generate_report` did. The timings are shown only in spike builds. */
export interface ReportResult {
  report: ReportInfo;
  stats: {
    observations: number;
    photos: number;
    /** Photos whose file could not be read; left out of the report. */
    skippedPhotos: number;
    /** Observations without a plan snapshot (plan never prepared on this device). */
    withoutSnapshot: number;
    snapshotMs: number;
    photoMs: number;
  };
  compileMs: number;
  pdfMs: number;
  totalMs: number;
}
/** The installed version: Android's versionName + versionCode (the CI run number); no build on desktop. */
export interface AppVersion {
  version: string;
  build: string | null;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}
export function asAppError(e: unknown): AppError {
  return isAppError(e) ? e : { code: "internal", message: String(e) };
}

export const api = {
  listProjects: () => invoke<ProjectSummary[]>("list_projects"),
  getProject: (id: string) => invoke<Project>("get_project", { id }),
  createProject: (name: string, address: string) => invoke<Project>("create_project", { name, address }),
  renameProject: (id: string, name: string) => invoke<Project>("rename_project", { id, name }),
  updateProjectAddress: (id: string, address: string) => invoke<Project>("update_project_address", { id, address }),
  deleteProject: (id: string) => invoke<void>("delete_project", { id }),
  updateRefSettings: (id: string, code: string, template: string, scope: Scope) =>
    invoke<Project>("update_ref_settings", { id, code, template, scope }),
  /** Next ref under unsaved settings (validation errors: invalid_template, fraction_required). */
  previewRefSettings: (id: string, code: string, template: string, scope: Scope, fraction: string) =>
    invoke<string>("preview_ref_settings", { id, code, template, scope, fraction }),
  listFractions: (projectId: string) => invoke<Fraction[]>("list_fractions", { projectId }),
  addFraction: (projectId: string, code: string) => invoke<Fraction>("add_fraction", { projectId, code }),
  deleteFraction: (id: string) => invoke<void>("delete_fraction", { id }),
  listPlans: (projectId: string) => invoke<Plan[]>("list_plans", { projectId }),
  stagePlanSource: (source: string) => invoke<StagedPlan>("stage_plan_source", { source }),
  importPlan: (projectId: string, token: string, title: string) => invoke<Plan>("import_plan", { projectId, token, title }),
  discardStagedPlan: (token: string) => invoke<void>("discard_staged_plan", { token }),
  renamePlan: (id: string, title: string) => invoke<Plan>("rename_plan", { id, title }),
  deletePlan: (id: string) => invoke<void>("delete_plan", { id }),
  getPlan: (id: string) => invoke<Plan>("get_plan", { id }),
  listPins: (planId: string) => invoke<Observation[]>("list_pins", { planId }),
  /** The project's open observations, newest first (the report's order). */
  listObservations: (projectId: string) => invoke<ObservationRow[]>("list_observations", { projectId }),
  /** Makes the missing list thumbnails among these photos and returns those that are ready (a broken photo is left out). */
  ensureThumbnails: (projectId: string, photoIds: string[]) => invoke<ThumbReady[]>("ensure_thumbnails", { projectId, photoIds }),
  /** Save of the observation sheet: adds the fraction if new and takes the next number. */
  /** One to three photos are required (`photo_required` / `photo_limit`, D-025). */
  createObservation: (planId: string, x: number, y: number, fraction: string, description: string, photos: PhotoInput[]) =>
    invoke<Observation>("create_observation", { planId, x, y, fraction, description, photos }),
  /** Edit mode: description, photos and the kept photos' annotations (fraction and ref are fixed). One to three photos must remain. */
  updateObservation: (id: string, description: string, photos: PhotoInput[], removePhotoIds: string[], annotations: PhotoAnnotation[]) =>
    invoke<Observation>("update_observation", { id, description, photos, removePhotoIds, annotations }),
  /** `source`: absolute path (Android plugin cache copy, Windows picker) or content:// URI. `utcOffsetMin`: minutes east of UTC. */
  stagePhoto: (source: string, utcOffsetMin: number) =>
    invoke<Omit<StagedPhoto, "annotation">>("stage_photo", { source, utcOffsetMin }).then((p): StagedPhoto => ({ ...p, annotation: null })),
  discardStagedPhoto: (token: string) => invoke<void>("discard_staged_photo", { token }),
  /** The subset of `tokens` whose staged file still exists. */
  existingStagedPhotos: (tokens: string[]) => invoke<string[]>("existing_staged_photos", { tokens }),
  /** Per-device settings (not part of project data). */
  getSetting: (key: string) => invoke<string | null>("get_setting", { key }),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  listPhotos: (observationId: string) => invoke<Photo[]>("list_photos", { observationId }),
  /** The ref the next observation in `fraction` would get (stored settings). */
  previewRef: (projectId: string, fraction: string) => invoke<string>("preview_ref", { projectId, fraction }),
  movePin: (id: string, x: number, y: number) => invoke<Observation>("move_pin", { id, x, y }),
  deletePin: (id: string) => invoke<void>("delete_pin", { id }),
  planTilesInfo: (planId: string) => invoke<TileInfo>("plan_tiles_info", { planId }),
  /** `data` is base64 WebP (Android IPC would send a Uint8Array as a JSON number array). */
  writePlanTile: (planId: string, size: number, x: number, y: number, data: string) =>
    invoke<void>("write_plan_tile", { planId, size, x, y, data }),
  writePlanTileManifest: (planId: string, manifest: TileManifest) => invoke<void>("write_plan_tile_manifest", { planId, manifest }),
  clearPlanTiles: (planId: string) => invoke<void>("clear_plan_tiles", { planId }),
  /** Writes the project's report PDF (one run at a time: `report_busy`). `timeZone`: IANA name of the device's zone; `utcOffsetMin`: its current offset (fallback). */
  generateReport: (projectId: string, lang: "en" | "pt", timeZone: string, utcOffsetMin: number) =>
    invoke<ReportResult>("generate_report", { projectId, lang, timeZone, utcOffsetMin }),
  /** The project's latest report; null when there is none (or it was made before Sprint 6a). */
  latestReport: (projectId: string) => invoke<ReportInfo | null>("latest_report", { projectId }),
  /** Android: opens the latest report in a PDF viewer (`no_pdf_app`, `no_report`). */
  openReport: (projectId: string) => invoke<void>("open_report", { projectId }),
  /** Android: shares the latest report through the share sheet (`no_share_app`, `no_report`). */
  shareReport: (projectId: string, title: string) => invoke<void>("share_report", { projectId, title }),
  appVersion: () => invoke<AppVersion>("app_version"),
  /** Debug builds only (the command does not exist in release builds, except local `dev-seed` ones): seeds `count` observations on a plan, with shapes on every photo when `annotate`. */
  devSeedObservations: (planId: string, count: number, annotate: boolean) => invoke<number>("dev_seed_observations", { planId, count, annotate }),
};

/** The device's IANA time zone (e.g. "Europe/Lisbon"); "" when the WebView cannot say. */
export function deviceTimeZone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone ?? "";
  } catch {
    return "";
  }
}
/** Current offset in minutes east of UTC (same convention as `stage_photo`). */
export const utcOffsetMin = () => -new Date().getTimezoneOffset();

export const ptToMm = (pt: number) => Math.round((pt / 72) * 25.4);
