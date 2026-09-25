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
  defaultTitle: string;
  info: PdfInfo;
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
};

export const ptToMm = (pt: number) => Math.round((pt / 72) * 25.4);
