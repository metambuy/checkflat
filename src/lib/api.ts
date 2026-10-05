// Typed wrappers over Tauri commands. Errors are { code, message, pages?, count? } (src-tauri/src/error.rs).
import { invoke } from "@tauri-apps/api/core";

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
export interface Project {
  id: string;
  name: string;
  address: string;
  logoPath: string | null;
  nextRefNo: number;
  createdAt: string;
  updatedAt: string;
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
  refNo: number;
  xNorm: number;
  yNorm: number;
  description: string;
  createdVisitId: string | null;
  resolvedVisitId: string | null;
  resolvedAt: string | null;
  createdAt: string;
  updatedAt: string;
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
  listPlans: (projectId: string) => invoke<Plan[]>("list_plans", { projectId }),
  stagePlanSource: (source: string) => invoke<StagedPlan>("stage_plan_source", { source }),
  importPlan: (projectId: string, token: string, title: string) => invoke<Plan>("import_plan", { projectId, token, title }),
  discardStagedPlan: (token: string) => invoke<void>("discard_staged_plan", { token }),
  renamePlan: (id: string, title: string) => invoke<Plan>("rename_plan", { id, title }),
  deletePlan: (id: string) => invoke<void>("delete_plan", { id }),
  getPlan: (id: string) => invoke<Plan>("get_plan", { id }),
  listPins: (planId: string) => invoke<Observation[]>("list_pins", { planId }),
  /** Confirms a draft pin: takes the next ref number. */
  createPin: (planId: string, x: number, y: number) => invoke<Observation>("create_pin", { planId, x, y }),
  movePin: (id: string, x: number, y: number) => invoke<Observation>("move_pin", { id, x, y }),
  deletePin: (id: string) => invoke<void>("delete_pin", { id }),
  planTilesInfo: (planId: string) => invoke<TileInfo>("plan_tiles_info", { planId }),
  /** `data` is base64 WebP (Android IPC would send a Uint8Array as a JSON number array). */
  writePlanTile: (planId: string, size: number, x: number, y: number, data: string) =>
    invoke<void>("write_plan_tile", { planId, size, x, y, data }),
  writePlanTileManifest: (planId: string, manifest: TileManifest) => invoke<void>("write_plan_tile_manifest", { planId, manifest }),
  clearPlanTiles: (planId: string) => invoke<void>("clear_plan_tiles", { planId }),
};

export const ptToMm = (pt: number) => Math.round((pt / 72) * 25.4);
