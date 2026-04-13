import { useEffect, useMemo, useState } from "react";
import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Textarea } from "../../components/ui/textarea";
import {
  generateInvestigationReport,
  getInvestigationDetail,
  listInvestigations,
  sendChatMessage,
} from "../../lib/tauri";
import type {
  ChatMessage,
  InvestigationDetail,
  InvestigationReport,
  InvestigationSummary,
} from "../../types/domain";

export function InvestigationsPage() {
  const [investigations, setInvestigations] = useState<InvestigationSummary[]>([]);
  const [selectedInvestigationId, setSelectedInvestigationId] = useState<string | null>(null);
  const [detail, setDetail] = useState<InvestigationDetail | null>(null);
  const [report, setReport] = useState<InvestigationReport | null>(null);
  const [reportView, setReportView] = useState<"markdown" | "html">("markdown");
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [analysisPrompt, setAnalysisPrompt] = useState("");
  const [analysisMessages, setAnalysisMessages] = useState<ChatMessage[]>([]);
  const [analysisSessionId, setAnalysisSessionId] = useState<string | null>(null);
  const [isAskingQwen, setIsAskingQwen] = useState(false);
  const [selectedEvidenceIds, setSelectedEvidenceIds] = useState<string[]>([]);

  useEffect(() => {
    async function loadInvestigations() {
      try {
        const saved = await listInvestigations();
        setInvestigations(saved);
        if (saved.length > 0) {
          setSelectedInvestigationId(saved[0].id);
        }
      } catch (error) {
        setStatusMessage(
          error instanceof Error ? error.message : "Failed to load investigation workspace.",
        );
      }
    }

    void loadInvestigations();
  }, []);

  useEffect(() => {
    async function loadDetail() {
      if (!selectedInvestigationId) {
        setDetail(null);
        setReport(null);
        setAnalysisMessages([]);
        setAnalysisSessionId(null);
        setSelectedEvidenceIds([]);
        return;
      }

      try {
        const nextDetail = await getInvestigationDetail(selectedInvestigationId);
        setDetail(nextDetail);
        setReport(null);
        setSelectedEvidenceIds(nextDetail.evidence.map((item) => item.id));
      } catch (error) {
        setStatusMessage(error instanceof Error ? error.message : "Failed to load investigation detail.");
      }
    }

    void loadDetail();
  }, [selectedInvestigationId]);

  const selectedInvestigation = useMemo(
    () => investigations.find((investigation) => investigation.id === selectedInvestigationId) ?? null,
    [investigations, selectedInvestigationId],
  );

  async function handleGenerateReport() {
    if (!selectedInvestigationId) {
      return;
    }

    setIsGenerating(true);
    setStatusMessage(null);
    try {
      const nextReport = await generateInvestigationReport({ investigationId: selectedInvestigationId });
      setReport(nextReport);
      setStatusMessage(`Generated Markdown and HTML report for "${nextReport.investigation.title}".`);
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to generate report.");
    } finally {
      setIsGenerating(false);
    }
  }

  async function handleAskQwen() {
    if (!selectedInvestigation || !analysisPrompt.trim()) {
      return;
    }

    setIsAskingQwen(true);
    setStatusMessage(null);
    try {
      const response = await sendChatMessage({
        sessionId: analysisSessionId ?? undefined,
        environmentId: selectedInvestigation.environmentId,
        investigationId: selectedInvestigation.id,
        selectedEvidenceIds,
        content: analysisPrompt.trim(),
      });
      setAnalysisSessionId(response.session.id);
      setAnalysisMessages(response.messages);
      setAnalysisPrompt("");
      setStatusMessage(`Qwen analyzed investigation "${selectedInvestigation.title}".`);
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to analyze investigation with Qwen.");
    } finally {
      setIsAskingQwen(false);
    }
  }

  function toggleEvidenceSelection(evidenceId: string, checked: boolean) {
    setSelectedEvidenceIds((current) => {
      if (checked) {
        return current.includes(evidenceId) ? current : [...current, evidenceId];
      }
      return current.filter((item) => item !== evidenceId);
    });
  }

  return (
    <div className="grid gap-6 xl:grid-cols-[0.88fr_1.12fr]">
      <SectionCard
        eyebrow="Week 8"
        title="Investigation Workspace"
        description="Saved evidence now rolls into a report-oriented workspace with timeline generation and export-ready Markdown and HTML."
      >
        {statusMessage ? <p className="text-sm text-muted-foreground">{statusMessage}</p> : null}
        <div className="space-y-3">
          {investigations.length === 0 ? (
            <div className="rounded-lg border border-dashed border-border bg-muted/40 p-5 text-sm text-muted-foreground">
              No investigations saved yet. Attach evidence from logs, SSH, or Nacos to start a report.
            </div>
          ) : (
            investigations.map((investigation) => (
              <button
                className={[
                  "w-full rounded-xl border px-4 py-4 text-left transition",
                  investigation.id === selectedInvestigationId
                    ? "border-primary/30 bg-primary/5"
                    : "bg-muted/20",
                ].join(" ")}
                key={investigation.id}
                onClick={() => setSelectedInvestigationId(investigation.id)}
                type="button"
              >
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <span className="font-medium">{investigation.title}</span>
                  <Badge variant="outline">{investigation.environmentId}</Badge>
                </div>
                <p className="mt-2 text-xs text-muted-foreground">
                  {investigation.updatedAt} · {investigation.status}
                </p>
              </button>
            ))
          )}
        </div>
      </SectionCard>

      <div className="grid gap-6">
        <SectionCard
          eyebrow="Timeline"
          title={selectedInvestigation ? selectedInvestigation.title : "Saved Evidence"}
          description="The timeline is generated from attached evidence so incident context can be read in chronological order."
        >
          {detail?.investigation ? (
            <div className="rounded-xl border bg-muted/20 p-4">
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="secondary">{detail.investigation.environmentId}</Badge>
                <Badge variant="outline">{detail.evidence.length} evidence item(s)</Badge>
                <Badge variant="outline">{detail.timeline.length} timeline event(s)</Badge>
              </div>
              <p className="mt-3 text-sm leading-6 text-muted-foreground">
                Created {detail.investigation.createdAt}. Updated {detail.investigation.updatedAt}.
              </p>
            </div>
          ) : null}
          <div className="space-y-4">
            {detail === null ? (
              <p className="text-sm text-muted-foreground">Choose an investigation to inspect saved evidence.</p>
            ) : (
              detail.timeline.map((event) => (
                <article className="rounded-xl border bg-background/80 p-4" key={event.id}>
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div className="space-y-1">
                      <p className="text-sm font-semibold">{event.title}</p>
                      <p className="text-xs text-muted-foreground">
                        {event.timestamp} · {event.sourceType}
                      </p>
                    </div>
                    <Badge variant="secondary">{event.sourceType}</Badge>
                  </div>
                  <p className="mt-3 text-sm leading-6 text-muted-foreground">{event.detail}</p>
                </article>
              ))
            )}
          </div>
        </SectionCard>

        <SectionCard
          eyebrow="Correlation"
          title="Cross-Source Insights"
          description="Saved evidence is compared across sources so likely relationships show up before you read the full raw payloads."
        >
          <div className="space-y-4">
            {detail === null ? (
              <p className="text-sm text-muted-foreground">Choose an investigation to inspect correlations.</p>
            ) : detail.correlations.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No cross-source correlations detected yet. Save logs, Kubernetes events, SSH, and config drift into the same investigation to enrich this view.
              </p>
            ) : (
              detail.correlations.map((item) => (
                <article className="rounded-xl border bg-background/80 p-4" key={item.id}>
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <p className="text-sm font-semibold">{item.title}</p>
                    <Badge variant={item.confidence === "high" ? "success" : "warning"}>
                      {item.confidence}
                    </Badge>
                  </div>
                  <p className="mt-3 text-sm leading-6 text-muted-foreground">{item.detail}</p>
                  <p className="mt-2 text-xs text-muted-foreground">
                    Linked evidence: {item.linkedEvidenceIds.join(", ")}
                  </p>
                </article>
              ))
            )}
          </div>
        </SectionCard>

        <SectionCard
          eyebrow="Evidence"
          title="Evidence Collection"
          description="Each saved item remains visible as source material for the generated report."
        >
          <div className="space-y-4">
            {detail === null ? (
              <p className="text-sm text-muted-foreground">Choose an investigation to inspect evidence.</p>
            ) : detail.evidence.length === 0 ? (
              <p className="text-sm text-muted-foreground">No evidence saved yet for this investigation.</p>
            ) : (
              detail.evidence.map((item) => (
                <article className="rounded-xl border bg-background/80 p-4" key={item.id}>
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div className="space-y-1">
                      <p className="text-sm font-semibold">{item.title}</p>
                      <p className="text-xs text-muted-foreground">
                        {item.evidenceType} · {item.createdAt}
                      </p>
                    </div>
                    <Badge variant="secondary">{item.evidenceType}</Badge>
                  </div>
                  <p className="mt-3 text-sm leading-6 text-muted-foreground">{item.summary}</p>
                  <pre className="mt-3 max-h-56 overflow-auto rounded-lg bg-muted/30 p-3 text-xs leading-6 text-muted-foreground">
                    {prettyContent(item.contentJson)}
                  </pre>
                </article>
              ))
            )}
          </div>
        </SectionCard>

        <SectionCard
          eyebrow="Qwen Analysis"
          title="Ask Qwen About This Investigation"
          description="Send the selected investigation's saved evidence, timeline, and cross-source correlations to Qwen for synthesis."
        >
          {!selectedInvestigation ? (
            <p className="text-sm text-muted-foreground">
              Choose an investigation first, then ask Qwen to summarize root cause, impact, or next steps.
            </p>
          ) : (
            <div className="space-y-4">
              <div className="rounded-xl border bg-muted/20 p-4">
                <div className="flex flex-wrap items-center gap-2">
                  <Badge variant="secondary">{selectedInvestigation.environmentId}</Badge>
                  <Badge variant="outline">{detail?.evidence.length ?? 0} evidence item(s)</Badge>
                  <Badge variant="outline">{analysisSessionId ? "Existing Qwen session" : "New Qwen session"}</Badge>
                </div>
                <p className="mt-3 text-sm text-muted-foreground">
                  Choose the saved evidence you want Qwen to use. Only selected items are sent as investigation context, with payloads trimmed for model input size.
                </p>
              </div>
              <div className="rounded-xl border bg-background/70 p-4">
                <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
                  <p className="text-sm font-semibold">Evidence selection</p>
                  <div className="flex flex-wrap gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setSelectedEvidenceIds(detail?.evidence.map((item) => item.id) ?? [])}
                      disabled={!detail || detail.evidence.length === 0}
                    >
                      Select All
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setSelectedEvidenceIds([])}
                      disabled={selectedEvidenceIds.length === 0}
                    >
                      Clear
                    </Button>
                  </div>
                </div>
                {detail === null || detail.evidence.length === 0 ? (
                  <p className="text-sm text-muted-foreground">No saved evidence is available for this investigation yet.</p>
                ) : (
                  <div className="space-y-2">
                    {detail.evidence.map((item) => (
                      <label
                        key={item.id}
                        className="flex items-start gap-3 rounded-lg border bg-muted/20 px-3 py-3 text-sm"
                      >
                        <input
                          type="checkbox"
                          checked={selectedEvidenceIds.includes(item.id)}
                          onChange={(event) => toggleEvidenceSelection(item.id, event.target.checked)}
                          className="mt-1"
                        />
                        <div className="min-w-0 flex-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="font-medium text-foreground">{item.title}</span>
                            <Badge variant="outline">{item.evidenceType}</Badge>
                          </div>
                          <p className="mt-1 text-xs text-muted-foreground">{item.createdAt}</p>
                          <p className="mt-2 line-clamp-2 text-sm text-muted-foreground">{item.summary}</p>
                        </div>
                      </label>
                    ))}
                  </div>
                )}
              </div>
              <Textarea
                className="min-h-28"
                placeholder="Ask something like: summarize the likely root cause from this evidence and tell me what to verify next."
                value={analysisPrompt}
                onChange={(event) => setAnalysisPrompt(event.target.value)}
              />
              <div className="flex flex-wrap gap-3">
                <Button
                  onClick={() => void handleAskQwen()}
                  disabled={isAskingQwen || !analysisPrompt.trim() || selectedEvidenceIds.length === 0}
                >
                  {isAskingQwen ? "Sending..." : "Send Evidence To Qwen"}
                </Button>
                <Button
                  variant="outline"
                  onClick={() => {
                    setAnalysisSessionId(null);
                    setAnalysisMessages([]);
                    setStatusMessage("Started a fresh investigation analysis session.");
                  }}
                  disabled={isAskingQwen}
                >
                  New Analysis Session
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                {selectedEvidenceIds.length} evidence item(s) selected for the next Qwen request.
              </p>
              <div className="space-y-3">
                {analysisMessages.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    No analysis messages yet. The first question will send the selected investigation context to Qwen.
                  </p>
                ) : (
                  analysisMessages.map((message) => (
                    <article className="rounded-xl border bg-background/80 p-4" key={message.id}>
                      <div className="flex items-center justify-between gap-3">
                        <Badge variant={message.role === "assistant" ? "default" : "outline"}>
                          {message.role}
                        </Badge>
                        <span className="text-xs text-muted-foreground">{message.createdAt}</span>
                      </div>
                      <p className="mt-3 whitespace-pre-wrap text-sm leading-6 text-foreground">
                        {message.content}
                      </p>
                    </article>
                  ))
                )}
              </div>
            </div>
          )}
        </SectionCard>

        <SectionCard
          eyebrow="Report Export"
          title="Markdown And HTML"
          description="Generate a compact incident-style report from the saved investigation timeline and evidence."
        >
          <div className="flex flex-wrap gap-3">
            <button
              className={[
                "rounded-full border px-4 py-2 text-sm transition",
                reportView === "markdown" ? "border-primary/30 bg-primary/5" : "bg-muted/20",
              ].join(" ")}
              onClick={() => setReportView("markdown")}
              type="button"
            >
              Markdown
            </button>
            <button
              className={[
                "rounded-full border px-4 py-2 text-sm transition",
                reportView === "html" ? "border-primary/30 bg-primary/5" : "bg-muted/20",
              ].join(" ")}
              onClick={() => setReportView("html")}
              type="button"
            >
              HTML
            </button>
            <button
              className="rounded-full border bg-background px-4 py-2 text-sm transition hover:bg-accent"
              onClick={() => void handleGenerateReport()}
              disabled={!selectedInvestigationId || isGenerating}
              type="button"
            >
              {isGenerating ? "Generating..." : "Generate Report"}
            </button>
          </div>
          {report ? (
            <pre className="max-h-[32rem] overflow-auto rounded-xl border bg-muted/20 p-4 text-xs leading-6 text-muted-foreground">
              {reportView === "markdown" ? report.markdown : report.html}
            </pre>
          ) : (
            <p className="text-sm text-muted-foreground">
              Generate a report to preview the Markdown and HTML output for this investigation.
            </p>
          )}
        </SectionCard>
      </div>
    </div>
  );
}

function prettyContent(contentJson: string): string {
  try {
    return JSON.stringify(JSON.parse(contentJson), null, 2);
  } catch {
    return contentJson;
  }
}
