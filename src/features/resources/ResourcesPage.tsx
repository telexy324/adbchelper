import type { EnvironmentProfile } from "../../types/domain";
import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";

interface ResourcesPageProps {
  environments: EnvironmentProfile[];
}

export function ResourcesPage({ environments }: ResourcesPageProps) {
  return (
    <div className="grid gap-6 xl:grid-cols-[1.1fr_0.9fr]">
      <SectionCard
        eyebrow="Week 4-7"
        title="Resources Hub"
        description="This view will unify the systems you operate every day and make them available to the troubleshooting assistant."
      >
        <div className="grid gap-4">
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
        </div>
      </SectionCard>
      <SectionCard
        eyebrow="Adapters"
        title="Initial Integrations"
        description="The first implementation will prioritize read-only resource access so the assistant can investigate safely."
      >
        <ul className="space-y-3 text-sm leading-6 text-muted-foreground">
          <li>Kubernetes namespace, pod, event, and log inspection</li>
          <li>ELK log search and clustering</li>
          <li>SSH diagnostics for server and Nginx hosts</li>
          <li>Nacos configuration comparison</li>
        </ul>
      </SectionCard>
    </div>
  );
}

function EnvironmentBadge({ kind }: { kind: EnvironmentProfile["kind"] }) {
  const variant = kind === "prod" ? "danger" : kind === "test" ? "warning" : "success";

  return <Badge variant={variant}>{kind}</Badge>;
}
