import { useEffect, useState, type ReactNode } from "react";
import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { runSshDiagnostics, searchLogs } from "../../lib/tauri";
import type {
  EnvironmentProfile,
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
  const [logFilters, setLogFilters] = useState<LogSearchInput>({
    environmentId: environments[0]?.id ?? "dev",
    service: "",
    pod: "",
    keyword: "",
    traceId: "",
    timeRange: "1h",
  });
  const [sshFilters, setSshFilters] = useState<SshDiagnosticsInput>({
    environmentId: environments[0]?.id ?? "dev",
    host: "",
    commandPreset: "system_overview",
    logPath: "",
  });
  const [logResults, setLogResults] = useState<LogSearchResponse | null>(null);
  const [sshResults, setSshResults] = useState<SshDiagnosticsResponse | null>(null);
  const [logStatusMessage, setLogStatusMessage] = useState<string | null>(null);
  const [sshStatusMessage, setSshStatusMessage] = useState<string | null>(null);
  const [isLogLoading, setIsLogLoading] = useState(false);
  const [isSshLoading, setIsSshLoading] = useState(false);

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

  const activeLogEnvironment =
    environments.find((environment) => environment.id === logFilters.environmentId) ?? environments[0] ?? null;
  const activeSshEnvironment =
    environments.find((environment) => environment.id === sshFilters.environmentId) ?? environments[0] ?? null;

  return (
    <div className="grid gap-6">
      <SectionCard
        eyebrow="Week 5"
        title="Log Analysis Workbench"
        description="Search logs with the same filters from the roadmap, cluster repeated failures, and shape evidence for chat and investigations."
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
            <Input
              value={logFilters.service}
              onChange={(event) => updateLogFilter("service", event.target.value)}
            />
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
            <Input
              value={logFilters.traceId}
              onChange={(event) => updateLogFilter("traceId", event.target.value)}
            />
          </Field>
        </div>
        <div className="flex flex-wrap gap-3">
          <Button onClick={() => void runSearch(logFilters)} disabled={isLogLoading}>
            {isLogLoading ? "Searching..." : "Search Logs"}
          </Button>
          <Button
            variant="outline"
            onClick={() => {
              const nextFilters: LogSearchInput = {
                environmentId: activeLogEnvironment?.id ?? environments[0]?.id ?? "dev",
                service: "",
                pod: "",
                keyword: "",
                traceId: "",
                timeRange: "1h",
              };
              setFilters(nextFilters);
              void runSearch(nextFilters);
            }}
          >
            Reset Filters
          </Button>
        </div>
        {logStatusMessage ? <p className="text-sm text-muted-foreground">{logStatusMessage}</p> : null}
        {logResults ? (
          <div className="space-y-4">
            <div className="rounded-xl border bg-muted/30 p-4">
              <div className="mb-3 flex flex-wrap items-center gap-2">
                <Badge variant="secondary">{logResults.adapterMode}</Badge>
                <Badge variant="outline">{logResults.entries.length} events</Badge>
                <Badge variant="outline">{logResults.clusters.length} clusters</Badge>
              </div>
              <p className="text-sm font-semibold text-foreground">{logResults.summary.headline}</p>
              <p className="mt-2 text-xs leading-5 text-muted-foreground">{logResults.executedQuery}</p>
            </div>
            <div className="grid gap-4 lg:grid-cols-2">
              <div className="rounded-xl border bg-background/80 p-4">
                <p className="text-sm font-semibold">Likely causes</p>
                <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
                  {logResults.summary.likelyCauses.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
              <div className="rounded-xl border bg-background/80 p-4">
                <p className="text-sm font-semibold">Recommended next steps</p>
                <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
                  {logResults.summary.recommendedNextSteps.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
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
          </div>
        ) : null}
      </SectionCard>
      <div className="grid gap-6 xl:grid-cols-[1.05fr_0.95fr]">
        <SectionCard
          eyebrow="Week 6"
          title="SSH Diagnostics Workbench"
          description="Run read-only host diagnostics through approved presets, inspect host health, and pull log evidence without opening the door to arbitrary shell access."
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
                onChange={(event) =>
                  updateSshFilter("commandPreset", event.target.value as SshCommandPreset)
                }
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
          <div className="flex flex-wrap gap-3">
            <Button onClick={() => void runSsh(sshFilters)} disabled={isSshLoading}>
              {isSshLoading ? "Running..." : "Run Diagnostics"}
            </Button>
            <Button
              variant="outline"
              onClick={() => {
                const nextFilters: SshDiagnosticsInput = {
                  environmentId: activeSshEnvironment?.id ?? environments[0]?.id ?? "dev",
                  host: "",
                  commandPreset: "system_overview",
                  logPath: "",
                };
                setSshFilters(nextFilters);
                void runSsh(nextFilters);
              }}
            >
              Reset Diagnostics
            </Button>
          </div>
          {sshStatusMessage ? <p className="text-sm text-muted-foreground">{sshStatusMessage}</p> : null}
          {sshResults ? (
            <div className="space-y-4">
              <div className="rounded-xl border bg-muted/30 p-4">
                <div className="mb-3 flex flex-wrap items-center gap-2">
                  <Badge variant="secondary">{sshResults.adapterMode}</Badge>
                  <Badge variant="outline">{sshResults.targetHost}</Badge>
                  <Badge variant="outline">{sshResults.commandPreset}</Badge>
                </div>
                <p className="text-sm font-semibold text-foreground">{sshResults.summaryHeadline}</p>
                <p className="mt-2 text-xs leading-5 text-muted-foreground">{sshResults.executedCommand}</p>
              </div>
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
                        <Badge variant={line.level === "ERROR" ? "danger" : line.level === "WARN" ? "warning" : "outline"}>
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
            </div>
          ) : null}
        </SectionCard>
        <SectionCard
          eyebrow="Readiness"
          title="Environment Coverage"
          description="This keeps the broader roadmap visible while the Week 5 and Week 6 slices become real working surfaces."
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
            <div className="rounded-xl border border-dashed bg-background/70 p-4">
              <p className="text-sm font-semibold">What Week 6 adds</p>
              <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
                <li>SSH host diagnostics through a command whitelist instead of arbitrary commands</li>
                <li>Host health summaries for CPU, memory, disk, and listener checks</li>
                <li>Server log viewing for app and Nginx troubleshooting</li>
              </ul>
            </div>
            <div className="rounded-xl border border-dashed bg-background/70 p-4">
              <p className="text-sm font-semibold">Still next</p>
              <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
                <li>Real SSH transport and profile-based authentication</li>
                <li>Link SSH evidence directly into chat and investigations</li>
                <li>Cross-signal pivots between host diagnostics, ELK logs, and Kubernetes state</li>
              </ul>
            </div>
            {sshResults ? (
              <div className="rounded-xl border border-dashed bg-background/70 p-4">
                <p className="text-sm font-semibold">Approved command list</p>
                <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
                  {sshResults.allowedCommands.map((command) => (
                    <li key={command}>{command}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            {sshResults ? (
              <div className="rounded-xl border border-dashed bg-background/70 p-4">
                <p className="text-sm font-semibold">Recommended follow-up</p>
                <ul className="mt-3 space-y-2 text-sm leading-6 text-muted-foreground">
                  {sshResults.recommendedActions.map((action) => (
                    <li key={action}>{action}</li>
                  ))}
                </ul>
              </div>
            ) : null}
          </div>
        </SectionCard>
      </div>
    </div>
  );

  function updateLogFilter<Key extends keyof LogSearchInput>(key: Key, value: LogSearchInput[Key]) {
    setLogFilters((current) => ({
      ...current,
      [key]: value,
    }));
  }

  function updateSshFilter<Key extends keyof SshDiagnosticsInput>(
    key: Key,
    value: SshDiagnosticsInput[Key],
  ) {
    setSshFilters((current) => ({
      ...current,
      [key]: value,
    }));
  }
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
