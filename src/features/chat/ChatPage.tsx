import { useEffect, useMemo, useState } from "react";
import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Separator } from "../../components/ui/separator";
import { Textarea } from "../../components/ui/textarea";
import {
  listChatMessages,
  listChatSessions,
  listEnvironments,
  listToolCatalog,
  sendChatMessage,
} from "../../lib/tauri";
import type { ChatMessage, ChatSession, EnvironmentProfile, ToolDefinition } from "../../types/domain";

export function ChatPage() {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [toolCatalog, setToolCatalog] = useState<ToolDefinition[]>([]);
  const [environments, setEnvironments] = useState<EnvironmentProfile[]>([]);
  const [environmentId, setEnvironmentId] = useState("dev");
  const [draft, setDraft] = useState("");
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [isSending, setIsSending] = useState(false);

  useEffect(() => {
    async function loadData() {
      try {
        const [sessionData, toolData, environmentData] = await Promise.all([
          listChatSessions(),
          listToolCatalog(),
          listEnvironments(),
        ]);
        setSessions(sessionData);
        setToolCatalog(toolData);
        setEnvironments(environmentData);
        if (environmentData.length > 0) {
          setEnvironmentId(environmentData[0].id);
        }
        if (sessionData.length > 0) {
          setSelectedSessionId(sessionData[0].id);
        }
      } catch (error) {
        setStatusMessage(error instanceof Error ? error.message : "Failed to load chat workspace.");
      }
    }

    void loadData();
  }, []);

  useEffect(() => {
    async function loadMessages() {
      if (!selectedSessionId) {
        return;
      }

      try {
        const sessionMessages = await listChatMessages(selectedSessionId);
        setMessages(sessionMessages);
      } catch (error) {
        setStatusMessage(error instanceof Error ? error.message : "Failed to load messages.");
      }
    }

    void loadMessages();
  }, [selectedSessionId]);

  const selectedSession = useMemo(
    () => sessions.find((session) => session.id === selectedSessionId) ?? null,
    [selectedSessionId, sessions],
  );

  async function handleSend() {
    if (!draft.trim()) {
      return;
    }

    setIsSending(true);
    setStatusMessage(null);
    try {
      const response = await sendChatMessage({
        sessionId: selectedSessionId ?? undefined,
        environmentId,
        content: draft,
      });
      setSelectedSessionId(response.session.id);
      setSessions((current) => {
        const next = current.filter((session) => session.id !== response.session.id);
        return [response.session, ...next];
      });
      setMessages(response.messages);
      setToolCatalog(response.toolCatalog);
      setDraft("");
      setStatusMessage(`Answered with ${response.modelUsed}.`);
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to send message.");
    } finally {
      setIsSending(false);
    }
  }

  return (
    <div className="grid gap-6 xl:grid-cols-[1.2fr_0.8fr]">
      <SectionCard
        eyebrow="Week 3"
        title="Chat Orchestrator"
        description="This panel is where Qwen-backed troubleshooting, tool traces, and guided next steps will land."
      >
        <div className="space-y-4">
          <div className="grid gap-4 md:grid-cols-[180px_1fr]">
            <div className="space-y-2">
              <label className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                Environment
              </label>
              <select
                className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
                value={environmentId}
                onChange={(event) => setEnvironmentId(event.target.value)}
              >
                {environments.map((environment) => (
                  <option key={environment.id} value={environment.id}>
                    {environment.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="rounded-lg border border-dashed border-border bg-muted/40 p-4">
              <div className="mb-2 flex items-center justify-between">
                <p className="text-sm font-semibold">Session context</p>
                <Badge variant="secondary">{selectedSession ? "Existing session" : "New session"}</Badge>
              </div>
              <p className="text-sm text-muted-foreground">
                {selectedSession
                  ? `Working in "${selectedSession.title}" and storing the full conversation locally.`
                  : "Your first message will create a new persisted chat session automatically."}
              </p>
            </div>
          </div>
          <Textarea
            className="min-h-36"
            placeholder="Ask something like: Why are payment pods restarting in prod after the last release?"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
          />
          <div className="flex flex-wrap gap-3">
            <Button onClick={() => void handleSend()} disabled={isSending}>
              {isSending ? "Sending..." : "Send to Qwen"}
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => {
                setSelectedSessionId(null);
                setMessages([]);
                setStatusMessage("Starting a fresh chat session.");
              }}
            >
              New Session
            </Button>
          </div>
          {statusMessage ? <p className="text-sm text-muted-foreground">{statusMessage}</p> : null}
          <div className="rounded-lg border border-dashed border-border bg-muted/40 p-5">
            <div className="mb-4 flex items-center justify-between">
              <p className="text-sm font-semibold">Assistant conversation</p>
              <Badge variant="secondary">Persisted</Badge>
            </div>
            <div className="space-y-3">
              {messages.length === 0 ? (
                <p className="text-sm leading-6 text-muted-foreground">
                  No messages yet. The first prompt will create a chat session, store the exchange,
                  and call your configured Qwen profile for this environment.
                </p>
              ) : (
                messages.map((message) => (
                  <div
                    className="rounded-lg border bg-background/80 p-4"
                    key={message.id}
                  >
                    <div className="mb-2 flex items-center justify-between">
                      <Badge variant={message.role === "assistant" ? "default" : "outline"}>
                        {message.role}
                      </Badge>
                      <span className="text-xs text-muted-foreground">{message.createdAt}</span>
                    </div>
                    <p className="whitespace-pre-wrap text-sm leading-6 text-foreground">
                      {message.content}
                    </p>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </SectionCard>
      <SectionCard
        eyebrow="First Slice"
        title="Prompt and Tool Flow"
        description="The app foundation is set up so we can plug in the Qwen API client and typed tool calling next."
      >
        <div className="space-y-4">
          <div>
            <p className="text-sm font-semibold">Available tool catalog</p>
            <ul className="mt-3 space-y-3 text-sm leading-6 text-muted-foreground">
              {toolCatalog.map((tool) => (
                <li key={tool.name}>
                  <span className="font-medium text-foreground">{tool.name}</span>
                  {" · "}
                  {tool.description}
                </li>
              ))}
            </ul>
          </div>
          <Separator />
          <div>
            <p className="mb-3 text-sm font-semibold">Saved sessions</p>
            <div className="space-y-2">
              {sessions.length === 0 ? (
                <p className="text-sm text-muted-foreground">No sessions saved yet.</p>
              ) : (
                sessions.map((session) => (
                  <button
                    className={[
                      "w-full rounded-lg border px-3 py-3 text-left text-sm transition",
                      session.id === selectedSessionId ? "border-primary/30 bg-primary/5" : "bg-muted/20",
                    ].join(" ")}
                    key={session.id}
                    onClick={() => {
                      setSelectedSessionId(session.id);
                      setEnvironmentId(session.environmentId);
                    }}
                    type="button"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <span className="font-medium">{session.title}</span>
                      <Badge variant="outline">{session.environmentId}</Badge>
                    </div>
                    <p className="mt-1 text-xs text-muted-foreground">{session.updatedAt}</p>
                  </button>
                ))
              )}
            </div>
          </div>
        </div>
      </SectionCard>
    </div>
  );
}
