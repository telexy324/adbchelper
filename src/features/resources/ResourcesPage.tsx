import { useEffect, useState, type ReactNode } from "react";
import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import {
  attachToolEvidence,
  compareNacosConfig,
  listKubernetesEvents,
  listChatSessions,
  listInvestigations,
  runSshDiagnostics,
  saveInvestigationEvidence,
  searchLogs,
} from "../../lib/tauri";
import type {
  ChatSession,
  CompareNacosConfigInput,
  CompareNacosConfigResponse,
  EnvironmentProfile,
  InvestigationSummary,
  ListKubernetesEventsInput,
  ListKubernetesEventsResponse,
  LogSearchInput,
  LogSearchResponse,
  LogTimeRange,
  SshCommandPreset,
  SshDiagnosticsInput,
  SshDiagnosticsResponse,
} from "../../types/domain";

interface ResourcesPageProps {
  environments: EnvironmentProfile[];
}

const timeRangeOptions: LogTimeRange[] = ["15m", "1h", "6h", "24h"];
const sshCommandOptions: { value: SshCommandPreset; label: string }[] = [
  { value: "system_overview", label: "System overview" },
  { value: "check_process_ports", label: "Process and ports" },
  { value: "tail_app_log", label: "Tail application log" },
  { value: "tail_nginx_error", label: "Tail Nginx error log" },
];

export function ResourcesPage({ environments }: ResourcesPageProps) {
  const defaultEnvironmentId = environments[0]?.id ?? "dev";

  const [logFilters, setLogFilters] = useState<LogSearchInput>({
    environmentId: defaultEnvironmentId,
    service: "",
    pod: "",
    keyword: "",
    traceId: "",
    timeRange: "1h",
  });
  const [sshFilters, setSshFilters] = useState<SshDiagnosticsInput>({
    environmentId: defaultEnvironmentId,
    host: "",
    commandPreset: "system_overview",
    logPath: "",
  });
  const [kubernetesFilters, setKubernetesFilters] = useState<ListKubernetesEventsInput>({
    environmentId: defaultEnvironmentId,
    namespace: "default",
    involvedObject: "",
    reason: "",
  });
  const [nacosFilters, setNacosFilters] = useState<CompareNacosConfigInput>({
    sourceEnvironmentId: environments[1]?.id ?? defaultEnvironmentId,
    targetEnvironmentId: environments[2]?.id ?? defaultEnvironmentId,
    dataId: "payment-service.yaml",
    group: "DEFAULT_GROUP",
    namespaceId: "",
  });
  const [logResults, setLogResults] = useState<LogSearchResponse | null>(null);
  const [sshResults, setSshResults] = useState<SshDiagnosticsResponse | null>(null);
  const [kubernetesResults, setKubernetesResults] = useState<ListKubernetesEventsResponse | null>(null);
  const [nacosResults, setNacosResults] = useState<CompareNacosConfigResponse | null>(null);
  const [chatSessions, setChatSessions] = useState<ChatSession[]>([]);
  const [investigations, setInvestigations] = useState<InvestigationSummary[]>([]);
  const [attachChatSessionId, setAttachChatSessionId] = useState<string>("new");
  const [attachInvestigationId, setAttachInvestigationId] = useState<string>("new");
  const [investigationTitle, setInvestigationTitle] = useState("Nacos drift investigation");
  const [logStatusMessage, setLogStatusMessage] = useState<string | null>(null);
  const [sshStatusMessage, setSshStatusMessage] = useState<string | null>(null);
  const [kubernetesStatusMessage, setKubernetesStatusMessage] = useState<string | null>(null);
  const [nacosStatusMessage, setNacosStatusMessage] = useState<string | null>(null);
  const [attachStatusMessage, setAttachStatusMessage] = useState<string | null>(null);
  const [isLogLoading, setIsLogLoading] = useState(false);
  const [isSshLoading, setIsSshLoading] = useState(false);
  const [isKubernetesLoading, setIsKubernetesLoading] = useState(false);
  const [isNacosLoading, setIsNacosLoading] = useState(false);
  const [isAttachingChat, setIsAttachingChat] = useState(false);
  const [isAttachingInvestigation, setIsAttachingInvestigation] = useState(false);

  useEffect(() => {
    if (environments.length === 0) {
      return;
    }

    setLogFilters((current) => ({
      ...current,
      environmentId: current.environmentId || environments[0].id,
    }));
    setSshFilters((current) => ({
      ...current,
      environmentId: current.environmentId || environments[0].id,
    }));
    setKubernetesFilters((current) => ({
      ...current,
      environmentId: current.environmentId || environments[0].id,
    }));
    setNacosFilters((current) => ({
      ...current,
      sourceEnvironmentId: current.sourceEnvironmentId || environments[0].id,
      targetEnvironmentId:
        current.targetEnvironmentId || environments[environments.length - 1]?.id || environments[0].id,
    }));
  }, [environments]);

  useEffect(() => {
    if (environments.length === 0) {
      return;
    }

    void runSearch({
      environmentId: logFilters.environmentId || environments[0].id,
      service: logFilters.service,
      pod: logFilters.pod,
      keyword: logFilters.keyword,
      traceId: logFilters.traceId,
      timeRange: logFilters.timeRange,
    });
    void runSsh({
      environmentId: sshFilters.environmentId || environments[0].id,
      host: sshFilters.host,
      commandPreset: sshFilters.commandPreset,
      logPath: sshFilters.logPath,
    });
    void runKubernetesEvents({
      environmentId: kubernetesFilters.environmentId || environments[0].id,
      namespace: kubernetesFilters.namespace,
      involvedObject: kubernetesFilters.involvedObject,
      reason: kubernetesFilters.reason,
    });
  }, []);

  useEffect(() => {
    void refreshEvidenceTargets();
  }, []);

  async function runSearch(input: LogSearchInput) {
    setIsLogLoading(true);
    setLogStatusMessage(null);
    try {
      const response = await searchLogs(input);
      setLogResults(response);
      setLogStatusMessage(`Loaded ${response.entries.length} events from ${response.adapterMode}.`);
    } catch (error) {
      setLogStatusMessage(error instanceof Error ? error.message : "Failed to search logs.");
    } finally {
      setIsLogLoading(false);
    }
  }

  async function runSsh(input: SshDiagnosticsInput) {
    setIsSshLoading(true);
    setSshStatusMessage(null);
    try {
      const response = await runSshDiagnostics(input);
      setSshResults(response);
      setSshStatusMessage(`Loaded SSH diagnostics from ${response.adapterMode}.`);
    } catch (error) {
      setSshStatusMessage(error instanceof Error ? error.message : "Failed to run SSH diagnostics.");
    } finally {
      setIsSshLoading(false);
    }
  }

  async function runKubernetesEvents(input: ListKubernetesEventsInput) {
    setIsKubernetesLoading(true);
    setKubernetesStatusMessage(null);
    try {
      const response = await listKubernetesEvents(normalizeKubernetesInput(input));
      setKubernetesResults(response);
      setKubernetesStatusMessage(`Loaded ${response.events.length} Kubernetes events from ${response.adapterMode}.`);
    } catch (error) {
      setKubernetesStatusMessage(
        error instanceof Error ? error.message : "Failed to load Kubernetes events.",
      );
    } finally {
      setIsKubernetesLoading(false);
    }
  }

  async function runNacosCompare(input: CompareNacosConfigInput) {
    setIsNacosLoading(true);
    setNacosStatusMessage(null);
    try {
      const response = await compareNacosConfig(normalizeNacosInput(input));
      setNacosResults(response);
      setNacosStatusMessage(
        `Compared ${response.sourceEnvironmentId} against ${response.targetEnvironmentId} via ${response.adapterMode}.`,
      );
    } catch (error) {
      setNacosStatusMessage(
        error instanceof Error ? error.message : "Failed to compare Nacos configuration.",
      );
    } finally {
      setIsNacosLoading(false);
    }
  }

  async function refreshEvidenceTargets() {
    try {
      const [sessions, savedInvestigations] = await Promise.all([
        listChatSessions(),
        listInvestigations(),
      ]);
      setChatSessions(sessions);
      setInvestigations(savedInvestigations);
    } catch {
      // Keep the resource tools usable even if the evidence sidebars cannot refresh yet.
    }
  }

  async function handleAttachToChat() {
    if (!nacosResults) {
      return;
    }

    setIsAttachingChat(true);
    setAttachStatusMessage(null);
    try {
      const title = `Nacos drift ${nacosResults.dataId}`;
      const response = await attachToolEvidence({
        sessionId: attachChatSessionId === "new" ? undefined : attachChatSessionId,
        environmentId: nacosResults.targetEnvironmentId,
        title,
        toolName: "compare_nacos_config",
        content: formatNacosEvidenceMarkdown(nacosResults),
      });
      setAttachChatSessionId(response.session.id);
      setAttachStatusMessage(`Attached drift evidence to chat session "${response.session.title}".`);
      await refreshEvidenceTargets();
    } catch (error) {
      setAttachStatusMessage(error instanceof Error ? error.message : "Failed to attach drift to chat.");
    } finally {
      setIsAttachingChat(false);
    }
  }

  async function handleAttachToInvestigation() {
    if (!nacosResults) {
      return;
    }

    setIsAttachingInvestigation(true);
    setAttachStatusMessage(null);
    try {
      const response = await saveInvestigationEvidence({
        investigationId: attachInvestigationId === "new" ? undefined : attachInvestigationId,
        title: attachInvestigationId === "new" ? investigationTitle.trim() || "Nacos drift investigation" : undefined,
        environmentId: nacosResults.targetEnvironmentId,
        evidenceType: "nacos_diff",
        evidenceTitle: `${nacosResults.dataId} drift`,
        summary: nacosResults.summary.headline,
        contentJson: JSON.stringify(nacosResults),
      });
      setAttachInvestigationId(response.investigation.id);
      setInvestigationTitle(response.investigation.title);
      setAttachStatusMessage(`Saved drift evidence to investigation "${response.investigation.title}".`);
      await refreshEvidenceTargets();
    } catch (error) {
      setAttachStatusMessage(
        error instanceof Error ? error.message : "Failed to attach drift to investigation.",
      );
    } finally {
      setIsAttachingInvestigation(false);
    }
  }

  async function handleSaveEvidenceToInvestigation(options: {
    environmentId: string;
    evidenceType: string;
    title: string;
    summary: string;
    contentJson: string;
  }) {
    setIsAttachingInvestigation(true);
    setAttachStatusMessage(null);
    try {
      const response = await saveInvestigationEvidence({
        investigationId: attachInvestigationId === "new" ? undefined : attachInvestigationId,
        title:
          attachInvestigationId === "new"
            ? investigationTitle.trim() || "Cross-source investigation"
            : undefined,
        environmentId: options.environmentId,
        evidenceType: options.evidenceType,
        evidenceTitle: options.title,
        summary: options.summary,
        contentJson: options.contentJson,
      });
      setAttachInvestigationId(response.investigation.id);
      setInvestigationTitle(response.investigation.title);
      setAttachStatusMessage(`Saved evidence to investigation "${response.investigation.title}".`);
      await refreshEvidenceTargets();
    } catch (error) {
      setAttachStatusMessage(error instanceof Error ? error.message : "Failed to save investigation evidence.");
    } finally {
      setIsAttachingInvestigation(false);
    }
  }

  return (
    <div className="grid gap-6">
      <SectionCard
        eyebrow="Operations Hub"
        title="Read-Only Investigation Surfaces"
        description="The roadmap slices now live together here: Kubernetes events, logs, SSH, and Nacos all feed the same investigation flow."
      >
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <SummaryTile
            label="Week 4"
            title="Kubernetes Events"
            body="Inspect namespace warnings and workload events, then carry them into investigations."
          />
          <SummaryTile
            label="Week 5"
            title="ELK Log Analysis"
            body="Search by service, pod, keyword, traceId, and time range with clustered failures."
          />
          <SummaryTile
            label="Week 6"
            title="SSH Diagnostics"
            body="Run approved host checks and review server-side evidence without arbitrary shell access."
          />
          <SummaryTile
            label="Week 7"
            title="Nacos Config Diff"
            body="Compare environment configuration drift and surface likely impact before rollout or incident response."
          />
        </div>
      </SectionCard>

      <SectionCard
        eyebrow="Week 4"
        title="Kubernetes Events Workbench"
        description="Inspect namespace events and save rollout or pod-failure signals into the active investigation."
      >
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <Field label="Environment">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={kubernetesFilters.environmentId}
              onChange={(event) => updateKubernetesFilter("environmentId", event.target.value)}
            >
              {environments.map((environment) => (
                <option key={environment.id} value={environment.id}>
                  {environment.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Namespace">
            <Input
              value={kubernetesFilters.namespace}
              onChange={(event) => updateKubernetesFilter("namespace", event.target.value)}
            />
          </Field>
          <Field label="Workload filter">
            <Input
              placeholder="payment-api"
              value={kubernetesFilters.involvedObject}
              onChange={(event) => updateKubernetesFilter("involvedObject", event.target.value)}
            />
          </Field>
          <Field label="Reason filter">
            <Input
              placeholder="BackOff, Failed"
              value={kubernetesFilters.reason}
              onChange={(event) => updateKubernetesFilter("reason", event.target.value)}
            />
          </Field>
        </div>
        <ActionRow
          primaryLabel={isKubernetesLoading ? "Loading..." : "Load Events"}
          onPrimary={() => void runKubernetesEvents(kubernetesFilters)}
          primaryDisabled={isKubernetesLoading}
          onReset={() => {
            const nextFilters: ListKubernetesEventsInput = {
              environmentId: kubernetesFilters.environmentId || defaultEnvironmentId,
              namespace: "default",
              involvedObject: "",
              reason: "",
            };
            setKubernetesFilters(nextFilters);
            void runKubernetesEvents(nextFilters);
          }}
          resetLabel="Reset Events"
        />
        {kubernetesStatusMessage ? <p className="text-sm text-muted-foreground">{kubernetesStatusMessage}</p> : null}
        {kubernetesResults ? (
          <div className="space-y-4">
            <ResultHeader
              badges={[
                { text: kubernetesResults.adapterMode, variant: "secondary" },
                { text: `${kubernetesResults.events.length} events`, variant: "outline" },
                { text: kubernetesResults.namespace, variant: "outline" },
              ]}
              headline={kubernetesResults.summary.headline}
              caption={kubernetesResults.querySummary}
            />
            <div className="grid gap-4 lg:grid-cols-2">
              <SimpleListCard title="Likely impact" items={kubernetesResults.summary.likelyImpact} />
              <SimpleListCard
                title="Recommended next steps"
                items={kubernetesResults.summary.recommendedNextSteps}
              />
            </div>
            <div className="space-y-3">
              {kubernetesResults.events.map((event) => (
                <article className="rounded-xl border bg-muted/20 p-4" key={event.id}>
                  <div className="flex flex-wrap items-center gap-2">
                    <Badge variant={event.level === "Warning" ? "warning" : "outline"}>
                      {event.level}
                    </Badge>
                    <span className="text-sm font-semibold">
                      {event.kind}/{event.name}
                    </span>
                    <span className="text-xs text-muted-foreground">{event.reason}</span>
                  </div>
                  <p className="mt-2 text-sm text-muted-foreground">{event.message}</p>
                  <p className="mt-2 text-xs text-muted-foreground">{event.eventTime}</p>
                </article>
              ))}
            </div>
            <Button
              onClick={() =>
                void handleSaveEvidenceToInvestigation({
                  environmentId: kubernetesResults.environmentId,
                  evidenceType: "kubernetes_events",
                  title: `Kubernetes events ${kubernetesResults.namespace}`,
                  summary: kubernetesResults.summary.headline,
                  contentJson: JSON.stringify(kubernetesResults),
                })
              }
              disabled={isAttachingInvestigation}
            >
              {isAttachingInvestigation ? "Saving..." : "Attach Events To Investigation"}
            </Button>
          </div>
        ) : null}
      </SectionCard>

      <SectionCard
        eyebrow="Week 5"
        title="Log Analysis Workbench"
        description="Search logs, cluster repeated failures, and shape evidence for chat and investigations."
      >
        <div className="grid gap-4 md:grid-cols-2">
          <Field label="Environment">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={logFilters.environmentId}
              onChange={(event) => updateLogFilter("environmentId", event.target.value)}
            >
              {environments.map((environment) => (
                <option key={environment.id} value={environment.id}>
                  {environment.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Time range">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={logFilters.timeRange}
              onChange={(event) => updateLogFilter("timeRange", event.target.value as LogTimeRange)}
            >
              {timeRangeOptions.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Service">
            <Input value={logFilters.service} onChange={(event) => updateLogFilter("service", event.target.value)} />
          </Field>
          <Field label="Pod">
            <Input value={logFilters.pod} onChange={(event) => updateLogFilter("pod", event.target.value)} />
          </Field>
          <Field label="Keyword">
            <Input
              placeholder="timeout, 502, nacos..."
              value={logFilters.keyword}
              onChange={(event) => updateLogFilter("keyword", event.target.value)}
            />
          </Field>
          <Field label="Trace ID">
            <Input value={logFilters.traceId} onChange={(event) => updateLogFilter("traceId", event.target.value)} />
          </Field>
        </div>
        <ActionRow
          primaryLabel={isLogLoading ? "Searching..." : "Search Logs"}
          onPrimary={() => void runSearch(logFilters)}
          primaryDisabled={isLogLoading}
          onReset={() => {
            const nextFilters: LogSearchInput = {
              environmentId: logFilters.environmentId || defaultEnvironmentId,
              service: "",
              pod: "",
              keyword: "",
              traceId: "",
              timeRange: "1h",
            };
            setLogFilters(nextFilters);
            void runSearch(nextFilters);
          }}
          resetLabel="Reset Filters"
        />
        {logStatusMessage ? <p className="text-sm text-muted-foreground">{logStatusMessage}</p> : null}
        {logResults ? (
          <div className="space-y-4">
            <ResultHeader
              badges={[
                { text: logResults.adapterMode, variant: "secondary" },
                { text: `${logResults.entries.length} events`, variant: "outline" },
                { text: `${logResults.clusters.length} clusters`, variant: "outline" },
              ]}
              headline={logResults.summary.headline}
              caption={logResults.executedQuery}
            />
            <div className="grid gap-4 lg:grid-cols-2">
              <SimpleListCard title="Likely causes" items={logResults.summary.likelyCauses} />
              <SimpleListCard title="Recommended next steps" items={logResults.summary.recommendedNextSteps} />
            </div>
            <div className="space-y-3">
              <p className="text-sm font-semibold">Error clusters</p>
              {logResults.clusters.length === 0 ? (
                <p className="text-sm text-muted-foreground">No clusters for the current filters.</p>
              ) : (
                logResults.clusters.map((cluster) => (
                  <article className="rounded-xl border bg-muted/20 p-4" key={cluster.id}>
                    <div className="flex flex-wrap items-center gap-2">
                      <Badge variant={cluster.level === "ERROR" ? "danger" : "warning"}>
                        {cluster.level}
                      </Badge>
                      <span className="text-sm font-semibold">{cluster.label}</span>
                      <span className="text-xs text-muted-foreground">{cluster.count} hits</span>
                    </div>
                    <p className="mt-2 text-sm text-muted-foreground">{cluster.exampleMessage}</p>
                    <p className="mt-2 text-xs text-muted-foreground">
                      Services: {cluster.services.join(", ")}
                      {cluster.traceId ? ` · Sample traceId: ${cluster.traceId}` : ""}
                    </p>
                  </article>
                ))
              )}
            </div>
            <Button
              onClick={() =>
                void handleSaveEvidenceToInvestigation({
                  environmentId: logResults.environmentId,
                  evidenceType: "log_search",
                  title: `Log search ${logResults.environmentId}`,
                  summary: logResults.summary.headline,
                  contentJson: JSON.stringify(logResults),
                })
              }
              disabled={isAttachingInvestigation}
            >
              {isAttachingInvestigation ? "Saving..." : "Attach Logs To Investigation"}
            </Button>
          </div>
        ) : null}
      </SectionCard>

      <div className="grid gap-6 xl:grid-cols-[1.05fr_0.95fr]">
        <SectionCard
          eyebrow="Week 6"
          title="SSH Diagnostics Workbench"
          description="Run approved read-only diagnostics, inspect host health, and review server-side evidence."
        >
          <div className="grid gap-4 md:grid-cols-2">
            <Field label="Environment">
              <select
                className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
                value={sshFilters.environmentId}
                onChange={(event) => updateSshFilter("environmentId", event.target.value)}
              >
                {environments.map((environment) => (
                  <option key={environment.id} value={environment.id}>
                    {environment.name}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Command preset">
              <select
                className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
                value={sshFilters.commandPreset}
                onChange={(event) => updateSshFilter("commandPreset", event.target.value as SshCommandPreset)}
              >
                {sshCommandOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Host override">
              <Input
                placeholder="app-prod-01.internal"
                value={sshFilters.host}
                onChange={(event) => updateSshFilter("host", event.target.value)}
              />
            </Field>
            <Field label="Log path override">
              <Input
                placeholder="/var/log/app/application.log"
                value={sshFilters.logPath}
                onChange={(event) => updateSshFilter("logPath", event.target.value)}
              />
            </Field>
          </div>
          <ActionRow
            primaryLabel={isSshLoading ? "Running..." : "Run Diagnostics"}
            onPrimary={() => void runSsh(sshFilters)}
            primaryDisabled={isSshLoading}
            onReset={() => {
              const nextFilters: SshDiagnosticsInput = {
                environmentId: sshFilters.environmentId || defaultEnvironmentId,
                host: "",
                commandPreset: "system_overview",
                logPath: "",
              };
              setSshFilters(nextFilters);
              void runSsh(nextFilters);
            }}
            resetLabel="Reset Diagnostics"
          />
          {sshStatusMessage ? <p className="text-sm text-muted-foreground">{sshStatusMessage}</p> : null}
          {sshResults ? (
            <div className="space-y-4">
              <ResultHeader
                badges={[
                  { text: sshResults.adapterMode, variant: "secondary" },
                  { text: sshResults.targetHost, variant: "outline" },
                  { text: sshResults.commandPreset, variant: "outline" },
                ]}
                headline={sshResults.summaryHeadline}
                caption={sshResults.executedCommand}
              />
              <div className="grid gap-4 md:grid-cols-2">
                {sshResults.healthSummary.map((metric) => (
                  <article className="rounded-xl border bg-background/80 p-4" key={metric.label}>
                    <div className="flex items-center justify-between gap-3">
                      <p className="text-sm font-semibold">{metric.label}</p>
                      <Badge variant={metric.status === "warning" ? "warning" : "success"}>
                        {metric.status}
                      </Badge>
                    </div>
                    <p className="mt-3 text-2xl font-semibold">{metric.value}</p>
                    <p className="mt-2 text-sm leading-6 text-muted-foreground">{metric.detail}</p>
                  </article>
                ))}
              </div>
              <div className="rounded-xl border bg-background/80 p-4">
                <p className="text-sm font-semibold">Server log viewer</p>
                <div className="mt-3 space-y-3">
                  {sshResults.logLines.map((line) => (
                    <article className="rounded-lg border bg-muted/20 p-3" key={`${line.timestamp}-${line.message}`}>
                      <div className="flex flex-wrap items-center gap-2">
                        <Badge
                          variant={
                            line.level === "ERROR"
                              ? "danger"
                              : line.level === "WARN"
                                ? "warning"
                                : "outline"
                          }
                        >
                          {line.level}
                        </Badge>
                        <span className="text-xs text-muted-foreground">{line.timestamp}</span>
                        <span className="text-xs text-muted-foreground">{line.source}</span>
                      </div>
                      <p className="mt-2 text-sm leading-6 text-muted-foreground">{line.message}</p>
                    </article>
                  ))}
                </div>
              </div>
              <Button
                onClick={() =>
                  void handleSaveEvidenceToInvestigation({
                    environmentId: sshResults.environmentId,
                    evidenceType: "ssh_diagnostics",
                    title: `SSH diagnostics ${sshResults.targetHost}`,
                    summary: sshResults.summaryHeadline,
                    contentJson: JSON.stringify(sshResults),
                  })
                }
                disabled={isAttachingInvestigation}
              >
                {isAttachingInvestigation ? "Saving..." : "Attach SSH Evidence To Investigation"}
              </Button>
            </div>
          ) : null}
        </SectionCard>

        <SectionCard
          eyebrow="Readiness"
          title="Environment Coverage"
          description="This keeps the broader roadmap visible while the read-only integrations grow into full workflows."
        >
          <div className="space-y-4">
            {environments.map((environment) => (
              <article className="rounded-xl border bg-muted/30 p-4" key={environment.id}>
                <header className="mb-3 flex items-center justify-between gap-3">
                  <h3 className="text-base font-semibold">{environment.name}</h3>
                  <EnvironmentBadge kind={environment.kind} />
                </header>
                <p className="text-sm text-muted-foreground">
                  Kubernetes {environment.kubernetesEnabled ? "enabled" : "planned"} · ELK{" "}
                  {environment.elkEnabled ? "enabled" : "planned"} · SSH{" "}
                  {environment.sshEnabled ? "enabled" : "planned"}
                </p>
                <p className="mt-1 text-sm text-muted-foreground">
                  Nacos {environment.nacosEnabled ? "enabled" : "planned"} · Redis{" "}
                  {environment.redisEnabled ? "enabled" : "planned"}
                </p>
              </article>
            ))}
            {sshResults ? (
              <div className="rounded-xl border border-dashed bg-background/70 p-4">
                <p className="text-sm font-semibold">Approved SSH commands</p>
                <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
                  {sshResults.allowedCommands.map((command) => (
                    <li key={command}>{command}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            {nacosResults ? (
              <div className="rounded-xl border border-dashed bg-background/70 p-4">
                <p className="text-sm font-semibold">Nacos drift focus</p>
                <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
                  {nacosResults.summary.likelyImpact.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
            ) : null}
          </div>
        </SectionCard>
      </div>

      <SectionCard
        eyebrow="Week 7"
        title="Nacos Config Diff"
        description="Compare config versions across environments, surface changed keys, and explain what the drift may affect."
      >
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <Field label="Source environment">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={nacosFilters.sourceEnvironmentId}
              onChange={(event) => updateNacosFilter("sourceEnvironmentId", event.target.value)}
            >
              {environments.map((environment) => (
                <option key={environment.id} value={environment.id}>
                  {environment.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Target environment">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={nacosFilters.targetEnvironmentId}
              onChange={(event) => updateNacosFilter("targetEnvironmentId", event.target.value)}
            >
              {environments.map((environment) => (
                <option key={environment.id} value={environment.id}>
                  {environment.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Data ID">
            <Input
              placeholder="payment-service.yaml"
              value={nacosFilters.dataId}
              onChange={(event) => updateNacosFilter("dataId", event.target.value)}
            />
          </Field>
          <Field label="Group">
            <Input
              placeholder="DEFAULT_GROUP"
              value={nacosFilters.group}
              onChange={(event) => updateNacosFilter("group", event.target.value)}
            />
          </Field>
        </div>
        <div className="grid gap-4 md:grid-cols-2">
          <Field label="Namespace override">
            <Input
              placeholder="public"
              value={nacosFilters.namespaceId}
              onChange={(event) => updateNacosFilter("namespaceId", event.target.value)}
            />
          </Field>
          <div className="rounded-xl border border-dashed bg-background/70 p-4 text-sm leading-6 text-muted-foreground">
            Profile-driven HTTP compare uses the saved `nacos` profiles for each environment. If a namespace
            override is empty, the compare uses the profile scope or `namespaceId` from Extra JSON.
          </div>
        </div>
        <ActionRow
          primaryLabel={isNacosLoading ? "Comparing..." : "Compare Config"}
          onPrimary={() => void runNacosCompare(nacosFilters)}
          primaryDisabled={isNacosLoading}
          onReset={() => {
            const nextFilters: CompareNacosConfigInput = {
              sourceEnvironmentId: environments[1]?.id ?? defaultEnvironmentId,
              targetEnvironmentId: environments[2]?.id ?? defaultEnvironmentId,
              dataId: "payment-service.yaml",
              group: "DEFAULT_GROUP",
              namespaceId: "",
            };
            setNacosFilters(nextFilters);
            setNacosResults(null);
            setNacosStatusMessage("Reset Nacos compare filters.");
          }}
          resetLabel="Reset Compare"
        />
        {nacosStatusMessage ? <p className="text-sm text-muted-foreground">{nacosStatusMessage}</p> : null}
        {nacosResults ? (
          <div className="space-y-4">
            <ResultHeader
              badges={[
                { text: nacosResults.adapterMode, variant: "secondary" },
                { text: `${nacosResults.diffEntries.length} diff entries`, variant: "outline" },
                {
                  text: nacosResults.namespaceId ? `namespace ${nacosResults.namespaceId}` : "default namespace",
                  variant: "outline",
                },
              ]}
              headline={nacosResults.summary.headline}
              caption={`${nacosResults.group} / ${nacosResults.dataId}`}
            />
            <div className="grid gap-4 lg:grid-cols-2">
              <SimpleListCard title="Likely impact" items={nacosResults.summary.likelyImpact} />
              <SimpleListCard title="Explanation" items={nacosResults.summary.explanation} />
            </div>
            <div className="grid gap-4 xl:grid-cols-[1.2fr_0.8fr]">
              <div className="space-y-3">
                <p className="text-sm font-semibold">Rich diff review</p>
                {nacosResults.diffEntries.length === 0 ? (
                  <p className="text-sm text-muted-foreground">No drift was detected for this config.</p>
                ) : (
                  nacosResults.diffEntries.map((entry) => (
                    <article className="rounded-xl border bg-muted/20 p-4" key={`${entry.status}-${entry.key}`}>
                      <div className="flex flex-wrap items-center gap-2">
                        <Badge
                          variant={
                            entry.status === "changed"
                              ? "warning"
                              : entry.status === "removed"
                                ? "danger"
                                : entry.status === "added"
                                  ? "success"
                                  : "outline"
                          }
                        >
                          {entry.status}
                        </Badge>
                        <span className="text-sm font-semibold">{entry.key}</span>
                      </div>
                      <div className="mt-3 grid gap-3 lg:grid-cols-2">
                        <RichValuePane
                          label={nacosResults.source.environmentId}
                          value={entry.sourceValue}
                          tone={entry.status === "removed" || entry.status === "changed" ? "source" : "neutral"}
                        />
                        <RichValuePane
                          label={nacosResults.target.environmentId}
                          value={entry.targetValue}
                          tone={entry.status === "added" || entry.status === "changed" ? "target" : "neutral"}
                        />
                      </div>
                    </article>
                  ))
                )}
              </div>
              <div className="space-y-4">
                <ConfigPreviewCard
                  title={nacosResults.source.environmentId}
                  profile={nacosResults.source.profileName}
                  value={nacosResults.source.value}
                />
                <ConfigPreviewCard
                  title={nacosResults.target.environmentId}
                  profile={nacosResults.target.profileName}
                  value={nacosResults.target.value}
                />
              </div>
            </div>
            <div className="space-y-3">
              <p className="text-sm font-semibold">Attach config drift evidence</p>
              <div className="grid gap-4 xl:grid-cols-2">
                <div className="rounded-xl border bg-background/80 p-4">
                  <div className="grid gap-4">
                    <Field label="Chat session">
                      <select
                        className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
                        value={attachChatSessionId}
                        onChange={(event) => setAttachChatSessionId(event.target.value)}
                      >
                        <option value="new">Create new session</option>
                        {chatSessions
                          .filter((session) => session.environmentId === nacosResults.targetEnvironmentId)
                          .map((session) => (
                            <option key={session.id} value={session.id}>
                              {session.title}
                            </option>
                          ))}
                      </select>
                    </Field>
                    <Button onClick={() => void handleAttachToChat()} disabled={isAttachingChat}>
                      {isAttachingChat ? "Attaching..." : "Attach To Chat"}
                    </Button>
                  </div>
                </div>
                <div className="rounded-xl border bg-background/80 p-4">
                  <div className="grid gap-4">
                    <Field label="Investigation">
                      <select
                        className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
                        value={attachInvestigationId}
                        onChange={(event) => setAttachInvestigationId(event.target.value)}
                      >
                        <option value="new">Create new investigation</option>
                        {investigations
                          .filter((investigation) => investigation.environmentId === nacosResults.targetEnvironmentId)
                          .map((investigation) => (
                            <option key={investigation.id} value={investigation.id}>
                              {investigation.title}
                            </option>
                          ))}
                      </select>
                    </Field>
                    {attachInvestigationId === "new" ? (
                      <Field label="New investigation title">
                        <Input
                          value={investigationTitle}
                          onChange={(event) => setInvestigationTitle(event.target.value)}
                        />
                      </Field>
                    ) : null}
                    <Button onClick={() => void handleAttachToInvestigation()} disabled={isAttachingInvestigation}>
                      {isAttachingInvestigation ? "Saving..." : "Attach To Investigation"}
                    </Button>
                  </div>
                </div>
              </div>
              {attachStatusMessage ? <p className="text-sm text-muted-foreground">{attachStatusMessage}</p> : null}
            </div>
          </div>
        ) : null}
      </SectionCard>
    </div>
  );

  function updateLogFilter<Key extends keyof LogSearchInput>(key: Key, value: LogSearchInput[Key]) {
    setLogFilters((current) => ({ ...current, [key]: value }));
  }

  function updateSshFilter<Key extends keyof SshDiagnosticsInput>(
    key: Key,
    value: SshDiagnosticsInput[Key],
  ) {
    setSshFilters((current) => ({ ...current, [key]: value }));
  }

  function updateKubernetesFilter<Key extends keyof ListKubernetesEventsInput>(
    key: Key,
    value: ListKubernetesEventsInput[Key],
  ) {
    setKubernetesFilters((current) => ({ ...current, [key]: value }));
  }

  function updateNacosFilter<Key extends keyof CompareNacosConfigInput>(
    key: Key,
    value: CompareNacosConfigInput[Key],
  ) {
    setNacosFilters((current) => ({ ...current, [key]: value }));
  }
}

function normalizeNacosInput(input: CompareNacosConfigInput): CompareNacosConfigInput {
  return {
    ...input,
    dataId: input.dataId.trim(),
    group: input.group.trim(),
    namespaceId: input.namespaceId?.trim() || undefined,
  };
}

function normalizeKubernetesInput(input: ListKubernetesEventsInput): ListKubernetesEventsInput {
  return {
    ...input,
    namespace: input.namespace.trim() || "default",
    involvedObject: input.involvedObject?.trim() || undefined,
    reason: input.reason?.trim() || undefined,
  };
}

function ActionRow({
  primaryLabel,
  onPrimary,
  primaryDisabled,
  onReset,
  resetLabel,
}: {
  primaryLabel: string;
  onPrimary: () => void;
  primaryDisabled?: boolean;
  onReset: () => void;
  resetLabel: string;
}) {
  return (
    <div className="flex flex-wrap gap-3">
      <Button onClick={onPrimary} disabled={primaryDisabled}>
        {primaryLabel}
      </Button>
      <Button variant="outline" onClick={onReset}>
        {resetLabel}
      </Button>
    </div>
  );
}

function ResultHeader({
  badges,
  headline,
  caption,
}: {
  badges: { text: string; variant: "secondary" | "outline" }[];
  headline: string;
  caption: string;
}) {
  return (
    <div className="rounded-xl border bg-muted/30 p-4">
      <div className="mb-3 flex flex-wrap items-center gap-2">
        {badges.map((badge) => (
          <Badge key={`${badge.variant}-${badge.text}`} variant={badge.variant}>
            {badge.text}
          </Badge>
        ))}
      </div>
      <p className="text-sm font-semibold text-foreground">{headline}</p>
      <p className="mt-2 text-xs leading-5 text-muted-foreground">{caption}</p>
    </div>
  );
}

function SimpleListCard({ title, items }: { title: string; items: string[] }) {
  return (
    <div className="rounded-xl border bg-background/80 p-4">
      <p className="text-sm font-semibold">{title}</p>
      <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  );
}

function ConfigPreviewCard({
  title,
  profile,
  value,
}: {
  title: string;
  profile: string;
  value: string;
}) {
  return (
    <div className="rounded-xl border bg-background/80 p-4">
      <div className="flex flex-wrap items-center gap-2">
        <p className="text-sm font-semibold">{title}</p>
        <Badge variant="outline">{profile}</Badge>
      </div>
      <pre className="mt-3 max-h-64 overflow-auto rounded-lg bg-muted/30 p-3 text-xs leading-6 text-muted-foreground">
        {value}
      </pre>
    </div>
  );
}

function RichValuePane({
  label,
  value,
  tone,
}: {
  label: string;
  value: string | null;
  tone: "source" | "target" | "neutral";
}) {
  const toneClass =
    tone === "source"
      ? "border-rose-200 bg-rose-50/60"
      : tone === "target"
        ? "border-emerald-200 bg-emerald-50/60"
        : "border-border bg-background/70";

  return (
    <div className={`rounded-lg border p-3 ${toneClass}`}>
      <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{label}</p>
      <pre className="mt-2 whitespace-pre-wrap break-all text-xs leading-6 text-foreground">
        {value ?? "(missing)"}
      </pre>
    </div>
  );
}

function formatNacosEvidenceMarkdown(result: CompareNacosConfigResponse): string {
  const topDiffs = result.diffEntries
    .slice(0, 10)
    .map(
      (entry) =>
        `- ${entry.key} [${entry.status}] source=${entry.sourceValue ?? "(missing)"} target=${entry.targetValue ?? "(missing)"}`,
    )
    .join("\n");

  return [
    `Nacos config drift attached for ${result.group}/${result.dataId}.`,
    `Source: ${result.sourceEnvironmentId}`,
    `Target: ${result.targetEnvironmentId}`,
    result.namespaceId ? `Namespace: ${result.namespaceId}` : "Namespace: default",
    "",
    `Summary: ${result.summary.headline}`,
    "",
    "Top drift entries:",
    topDiffs || "- No drift entries",
  ].join("\n");
}

function ValuePane({ label, value }: { label: string; value: string | null }) {
  return (
    <div className="rounded-lg border bg-background/70 p-3">
      <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{label}</p>
      <pre className="mt-2 whitespace-pre-wrap break-all text-xs leading-6 text-muted-foreground">
        {value ?? "(missing)"}
      </pre>
    </div>
  );
}

function SummaryTile({
  label,
  title,
  body,
}: {
  label: string;
  title: string;
  body: string;
}) {
  return (
    <div className="rounded-xl border bg-muted/20 p-4">
      <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{label}</p>
      <p className="mt-2 text-base font-semibold">{title}</p>
      <p className="mt-2 text-sm leading-6 text-muted-foreground">{body}</p>
    </div>
  );
}

function EnvironmentBadge({ kind }: { kind: EnvironmentProfile["kind"] }) {
  const variant = kind === "prod" ? "danger" : kind === "test" ? "warning" : "success";

  return <Badge variant={variant}>{kind}</Badge>;
}

function Field({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <label className="space-y-2">
      <span className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
        {label}
      </span>
      {children}
    </label>
  );
}
