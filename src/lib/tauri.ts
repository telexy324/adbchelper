import { invoke } from "@tauri-apps/api/core";
import type {
  AppHealth,
  ApprovalRequest,
  CreateApprovalInput,
  ExecuteApprovalInput,
  AttachToolEvidenceInput,
  CompareNacosConfigInput,
  CompareNacosConfigResponse,
  ChatResponse,
  ChatMessage,
  ChatSession,
  ConnectionProfile,
  EnvironmentProfile,
  InvestigationDetail,
  InvestigationEvidence,
  InvestigationCorrelation,
  InvestigationReport,
  InvestigationReportInput,
  InvestigationSaveResponse,
  InvestigationSummary,
  KubernetesEvent,
  KubernetesEventsSummary,
  ListKubernetesEventsInput,
  ListKubernetesEventsResponse,
  LogSearchInput,
  LogSearchResponse,
  SaveInvestigationInput,
  SendChatMessageInput,
  SshDiagnosticsInput,
  SshDiagnosticsResponse,
  ToolDefinition,
  UpsertConnectionProfileInput,
  UpsertEnvironmentInput,
  ValidationResult,
} from "../types/domain";

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("get_app_health");
}

export async function listApprovalRequests(): Promise<ApprovalRequest[]> {
  return invoke<ApprovalRequest[]>("list_approval_requests");
}

export async function createApprovalRequest(input: CreateApprovalInput): Promise<ApprovalRequest> {
  return invoke<ApprovalRequest>("create_approval_request", { input });
}

export async function approveRequest(approvalId: string): Promise<ApprovalRequest> {
  return invoke<ApprovalRequest>("approve_request", { approvalId });
}

export async function executeApprovalRequest(
  input: ExecuteApprovalInput,
): Promise<ApprovalRequest> {
  return invoke<ApprovalRequest>("execute_approval_request", { input });
}

export async function listEnvironments(): Promise<EnvironmentProfile[]> {
  return invoke<EnvironmentProfile[]>("list_environments");
}

export async function saveEnvironment(input: UpsertEnvironmentInput): Promise<EnvironmentProfile> {
  return invoke<EnvironmentProfile>("save_environment", { input });
}

export async function listConnectionProfiles(): Promise<ConnectionProfile[]> {
  return invoke<ConnectionProfile[]>("list_connection_profiles");
}

export async function validateConnectionProfile(
  input: UpsertConnectionProfileInput,
): Promise<ValidationResult> {
  return invoke<ValidationResult>("validate_connection_profile", { input });
}

export async function saveConnectionProfile(
  input: UpsertConnectionProfileInput,
): Promise<ConnectionProfile> {
  return invoke<ConnectionProfile>("save_connection_profile", { input });
}

export async function clearConnectionProfileSecret(profileId: string): Promise<void> {
  return invoke("clear_connection_profile_secret", { profileId });
}

export async function listChatSessions(): Promise<ChatSession[]> {
  return invoke<ChatSession[]>("list_chat_sessions");
}

export async function listChatMessages(sessionId: string): Promise<ChatMessage[]> {
  return invoke<ChatMessage[]>("list_chat_messages", { sessionId });
}

export async function listToolCatalog(): Promise<ToolDefinition[]> {
  return invoke<ToolDefinition[]>("list_tool_catalog");
}

export async function sendChatMessage(input: SendChatMessageInput): Promise<ChatResponse> {
  return invoke<ChatResponse>("send_chat_message", { input });
}

export async function attachToolEvidence(input: AttachToolEvidenceInput): Promise<ChatResponse> {
  return invoke<ChatResponse>("attach_tool_evidence", { input });
}

export async function listInvestigations(): Promise<InvestigationSummary[]> {
  return invoke<InvestigationSummary[]>("list_investigations");
}

export async function listInvestigationEvidence(
  investigationId: string,
): Promise<InvestigationEvidence[]> {
  return invoke<InvestigationEvidence[]>("list_investigation_evidence", { investigationId });
}

export async function getInvestigationDetail(
  investigationId: string,
): Promise<InvestigationDetail> {
  return invoke<InvestigationDetail>("get_investigation_detail", { investigationId });
}

export async function saveInvestigationEvidence(
  input: SaveInvestigationInput,
): Promise<InvestigationSaveResponse> {
  return invoke<InvestigationSaveResponse>("save_investigation_evidence", { input });
}

export async function generateInvestigationReport(
  input: InvestigationReportInput,
): Promise<InvestigationReport> {
  return invoke<InvestigationReport>("generate_investigation_report", { input });
}

export async function searchLogs(input: LogSearchInput): Promise<LogSearchResponse> {
  return invoke<LogSearchResponse>("search_logs", { input });
}

export async function listKubernetesEvents(
  input: ListKubernetesEventsInput,
): Promise<ListKubernetesEventsResponse> {
  return invoke<ListKubernetesEventsResponse>("list_kubernetes_events", { input });
}

export async function runSshDiagnostics(
  input: SshDiagnosticsInput,
): Promise<SshDiagnosticsResponse> {
  return invoke<SshDiagnosticsResponse>("run_ssh_diagnostics", { input });
}

export async function compareNacosConfig(
  input: CompareNacosConfigInput,
): Promise<CompareNacosConfigResponse> {
  return invoke<CompareNacosConfigResponse>("compare_nacos_config", { input });
}
