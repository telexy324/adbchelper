import type { LucideIcon } from "lucide-react";

export type AppSection =
  | "overview"
  | "chat"
  | "resources"
  | "investigations"
  | "settings";

export type EnvironmentKind = "dev" | "test" | "prod";
export type ConnectionProfileType = "kubernetes" | "elk" | "ssh" | "nacos" | "redis" | "qwen";

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

export interface ConnectionProfile {
  id: string;
  environmentId: string;
  profileType: ConnectionProfileType | string;
  name: string;
  endpoint: string;
  username: string | null;
  defaultScope: string | null;
  notes: string | null;
  configJson: string;
  hasSecret: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface UpsertEnvironmentInput {
  id: string;
  name: string;
  kind: EnvironmentKind;
  kubernetesEnabled: boolean;
  elkEnabled: boolean;
  sshEnabled: boolean;
  nacosEnabled: boolean;
  redisEnabled: boolean;
}

export interface UpsertConnectionProfileInput {
  id?: string;
  environmentId: string;
  profileType: ConnectionProfileType;
  name: string;
  endpoint: string;
  username?: string;
  defaultScope?: string;
  notes?: string;
  configJson?: string;
  secretValue?: string;
}

export interface ValidationResult {
  ok: boolean;
  messages: string[];
}

export interface ChatSession {
  id: string;
  environmentId: string;
  title: string;
  status: string;
  createdAt: string;
  updatedAt: string;
}

export interface ChatMessage {
  id: string;
  sessionId: string;
  role: "user" | "assistant" | "tool" | string;
  content: string;
  toolName: string | null;
  toolCallId: string | null;
  createdAt: string;
}

export interface ToolDefinition {
  name: string;
  description: string;
  inputHint: string;
}

export interface SendChatMessageInput {
  sessionId?: string;
  environmentId: string;
  content: string;
}

export interface ChatResponse {
  session: ChatSession;
  messages: ChatMessage[];
  assistantMessage: ChatMessage;
  toolCatalog: ToolDefinition[];
  modelUsed: string;
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
