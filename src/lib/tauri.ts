import { invoke } from "@tauri-apps/api/core";
import type { AppHealth, EnvironmentProfile } from "../types/domain";

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("get_app_health");
}

export async function listEnvironments(): Promise<EnvironmentProfile[]> {
  return invoke<EnvironmentProfile[]>("list_environments");
}
