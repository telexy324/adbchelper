import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";

export function InvestigationsPage() {
  return (
    <div className="grid gap-6 xl:grid-cols-[1.1fr_0.9fr]">
      <SectionCard
        eyebrow="Week 8"
        title="Investigation Workspace"
        description="Saved troubleshooting sessions, evidence timelines, and exportable incident reports will live here."
      >
        <div className="rounded-lg border border-dashed border-border bg-muted/40 p-5">
          <div className="mb-4 flex items-center justify-between">
            <p className="text-sm font-semibold">Planned structure</p>
            <Badge variant="secondary">Report-ready</Badge>
          </div>
          <ul className="space-y-3 text-sm leading-6 text-muted-foreground">
            <li>Investigation summary with root-cause notes</li>
            <li>Evidence collected from pod events, logs, and config diffs</li>
            <li>Markdown and HTML report export</li>
          </ul>
        </div>
      </SectionCard>
      <SectionCard
        eyebrow="Report Output"
        title="Office Work Acceleration"
        description="This module is designed to turn diagnosis into a report without rewriting the same summary every day."
      >
        <ol className="space-y-3 text-sm leading-6 text-muted-foreground">
          <li>Issue summary and impact scope</li>
          <li>Timeline of deployments, errors, and restarts</li>
          <li>Mitigation, follow-up actions, and report export</li>
        </ol>
      </SectionCard>
    </div>
  );
}
