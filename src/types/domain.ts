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

export interface AttachToolEvidenceInput {
  sessionId?: string;
  environmentId: string;
  title: string;
  toolName: string;
  content: string;
}

export interface ChatResponse {
  session: ChatSession;
  messages: ChatMessage[];
  assistantMessage: ChatMessage;
  toolCatalog: ToolDefinition[];
  modelUsed: string;
}

export type LogTimeRange = "15m" | "1h" | "6h" | "24h";

export interface LogSearchInput {
  environmentId: string;
  service?: string;
  pod?: string;
  keyword?: string;
  traceId?: string;
  timeRange: LogTimeRange;
}

export interface LogEntry {
  id: string;
  timestamp: string;
  environmentId: string;
  service: string;
  pod: string;
  level: string;
  traceId: string | null;
  message: string;
}

export interface LogCluster {
  id: string;
  label: string;
  level: string;
  count: number;
  services: string[];
  exampleMessage: string;
  traceId: string | null;
}

export interface LogSummary {
  headline: string;
  likelyCauses: string[];
  recommendedNextSteps: string[];
}

export interface LogSearchResponse {
  environmentId: string;
  timeRange: LogTimeRange;
  adapterMode: string;
  executedQuery: string;
  entries: LogEntry[];
  clusters: LogCluster[];
  summary: LogSummary;
}

export type SshCommandPreset =
  | "system_overview"
  | "check_process_ports"
  | "tail_app_log"
  | "tail_nginx_error";

export interface SshDiagnosticsInput {
  environmentId: string;
  host?: string;
  commandPreset: SshCommandPreset;
  logPath?: string;
}

export interface SshHealthMetric {
  label: string;
  status: string;
  value: string;
  detail: string;
}

export interface SshLogLine {
  timestamp: string;
  source: string;
  level: string;
  message: string;
}

export interface SshDiagnosticsResponse {
  environmentId: string;
  adapterMode: string;
  targetHost: string;
  commandPreset: SshCommandPreset;
  executedCommand: string;
  allowedCommands: string[];
  healthSummary: SshHealthMetric[];
  logLines: SshLogLine[];
  summaryHeadline: string;
  recommendedActions: string[];
}

export interface CompareNacosConfigInput {
  sourceEnvironmentId: string;
  targetEnvironmentId: string;
  dataId: string;
  group: string;
  namespaceId?: string;
}

export interface NacosConfigVersion {
  environmentId: string;
  profileName: string;
  namespaceId: string | null;
  value: string;
}

export interface NacosDiffEntry {
  key: string;
  status: string;
  sourceValue: string | null;
  targetValue: string | null;
}

export interface NacosDiffSummary {
  headline: string;
  likelyImpact: string[];
  explanation: string[];
}

export interface CompareNacosConfigResponse {
  sourceEnvironmentId: string;
  targetEnvironmentId: string;
  dataId: string;
  group: string;
  namespaceId: string | null;
  adapterMode: string;
  source: NacosConfigVersion;
  target: NacosConfigVersion;
  diffEntries: NacosDiffEntry[];
  summary: NacosDiffSummary;
}

export interface InvestigationSummary {
  id: string;
  title: string;
  environmentId: string;
  status: string;
  createdAt: string;
  updatedAt: string;
}

export interface InvestigationEvidence {
  id: string;
  investigationId: string;
  evidenceType: string;
  title: string;
  summary: string;
  contentJson: string;
  createdAt: string;
}

export interface SaveInvestigationInput {
  investigationId?: string;
  title?: string;
  environmentId: string;
  evidenceType: string;
  evidenceTitle: string;
  summary: string;
  contentJson: string;
}

export interface InvestigationSaveResponse {
  investigation: InvestigationSummary;
  evidence: InvestigationEvidence;
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
