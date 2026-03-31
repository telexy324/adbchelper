import { useEffect, useMemo, useState } from "react";
import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";
import { listInvestigationEvidence, listInvestigations } from "../../lib/tauri";
import type { InvestigationEvidence, InvestigationSummary } from "../../types/domain";

export function InvestigationsPage() {
  const [investigations, setInvestigations] = useState<InvestigationSummary[]>([]);
  const [selectedInvestigationId, setSelectedInvestigationId] = useState<string | null>(null);
  const [evidence, setEvidence] = useState<InvestigationEvidence[]>([]);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);

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
    async function loadEvidence() {
      if (!selectedInvestigationId) {
        setEvidence([]);
        return;
      }

      try {
        const saved = await listInvestigationEvidence(selectedInvestigationId);
        setEvidence(saved);
      } catch (error) {
        setStatusMessage(error instanceof Error ? error.message : "Failed to load evidence.");
      }
    }

    void loadEvidence();
  }, [selectedInvestigationId]);

  const selectedInvestigation = useMemo(
    () => investigations.find((investigation) => investigation.id === selectedInvestigationId) ?? null,
    [investigations, selectedInvestigationId],
  );

  return (
    <div className="grid gap-6 xl:grid-cols-[0.9fr_1.1fr]">
      <SectionCard
        eyebrow="Week 8 Foundation"
        title="Investigation Workspace"
        description="Evidence is now beginning to land here. Nacos drift attachments are persisted and visible as the first saved investigation stream."
      >
        {statusMessage ? <p className="text-sm text-muted-foreground">{statusMessage}</p> : null}
        <div className="space-y-3">
          {investigations.length === 0 ? (
            <div className="rounded-lg border border-dashed border-border bg-muted/40 p-5 text-sm text-muted-foreground">
              No investigations saved yet. Use the Week 7 Nacos panel to attach drift evidence here.
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
      <SectionCard
        eyebrow="Evidence Timeline"
        title={selectedInvestigation ? selectedInvestigation.title : "Saved Evidence"}
        description="Each attachment becomes a durable evidence record. This is the bridge from read-only diagnosis into report-ready investigations."
      >
        {selectedInvestigation ? (
          <div className="rounded-xl border bg-muted/20 p-4">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="secondary">{selectedInvestigation.environmentId}</Badge>
              <Badge variant="outline">{evidence.length} evidence item(s)</Badge>
            </div>
            <p className="mt-3 text-sm leading-6 text-muted-foreground">
              Created {selectedInvestigation.createdAt}. Updated {selectedInvestigation.updatedAt}.
            </p>
          </div>
        ) : null}
        <div className="space-y-4">
          {selectedInvestigation === null ? (
            <p className="text-sm text-muted-foreground">Choose an investigation to inspect saved evidence.</p>
          ) : evidence.length === 0 ? (
            <p className="text-sm text-muted-foreground">No evidence saved yet for this investigation.</p>
          ) : (
            evidence.map((item) => (
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
                <pre className="mt-3 max-h-72 overflow-auto rounded-lg bg-muted/30 p-3 text-xs leading-6 text-muted-foreground">
                  {prettyContent(item.contentJson)}
                </pre>
              </article>
            ))
          )}
        </div>
      </SectionCard>
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
