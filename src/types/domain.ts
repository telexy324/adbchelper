import type { LucideIcon } from "lucide-react";

export type AppSection =
  | "overview"
  | "chat"
  | "resources"
  | "investigations"
  | "settings";

export type EnvironmentKind = "dev" | "test" | "prod";

export interface EnvironmentProfile {
  id: string;
  name: string;
  kind: EnvironmentKind;
  kubernetesEnabled: boolean;
  elkEnabled: boolean;
  sshEnabled: boolean;
  nacosEnabled: boolean;
  redisEnabled: boolean;
}

export interface AppHealth {
  appName: string;
  version: string;
  databaseReady: boolean;
  storagePath: string;
}

export interface NavigationItem {
  id: AppSection;
  label: string;
  description: string;
  icon: LucideIcon;
}
