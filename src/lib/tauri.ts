import { invoke } from "@tauri-apps/api/core";
import type {
  AppHealth,
  ChatResponse,
  ChatMessage,
  ChatSession,
  ConnectionProfile,
  EnvironmentProfile,
  SendChatMessageInput,
  ToolDefinition,
  UpsertConnectionProfileInput,
  UpsertEnvironmentInput,
  ValidationResult,
} from "../types/domain";

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("get_app_health");
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
