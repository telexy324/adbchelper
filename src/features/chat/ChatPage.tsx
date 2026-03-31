import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Separator } from "../../components/ui/separator";

export function ChatPage() {
  return (
    <div className="grid gap-6 xl:grid-cols-[1.2fr_0.8fr]">
      <SectionCard
        eyebrow="Week 3"
        title="Chat Orchestrator"
        description="This panel is where Qwen-backed troubleshooting, tool traces, and guided next steps will land."
      >
        <div className="rounded-lg border border-dashed border-border bg-muted/40 p-5">
          <div className="mb-4 flex items-center justify-between">
            <p className="text-sm font-semibold">Planned capabilities</p>
            <Badge variant="secondary">Qwen-ready</Badge>
          </div>
          <ul className="space-y-3 text-sm leading-6 text-muted-foreground">
            <li>Natural-language troubleshooting across Kubernetes, ELK, SSH, and Nacos</li>
            <li>Tool execution evidence panel with request and result summaries</li>
            <li>Session save into investigations and incident reports</li>
          </ul>
        </div>
      </SectionCard>
      <SectionCard
        eyebrow="First Slice"
        title="Prompt and Tool Flow"
        description="The app foundation is set up so we can plug in the Qwen API client and typed tool calling next."
      >
        <div className="space-y-4">
          <ol className="space-y-3 text-sm leading-6 text-muted-foreground">
            <li>Frontend chat workspace and session history</li>
            <li>Rust orchestrator for tool planning and execution</li>
            <li>Audit logging for assistant actions</li>
          </ol>
          <Separator />
          <div className="flex flex-wrap gap-3">
            <Button size="sm">Connect Qwen API</Button>
            <Button size="sm" variant="outline">
              Register Tool Schemas
            </Button>
          </div>
        </div>
      </SectionCard>
    </div>
  );
}
