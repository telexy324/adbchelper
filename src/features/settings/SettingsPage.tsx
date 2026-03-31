import type { AppHealth, EnvironmentProfile } from "../../types/domain";
import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";

interface SettingsPageProps {
  appHealth: AppHealth | null;
  environments: EnvironmentProfile[];
}

export function SettingsPage({ appHealth, environments }: SettingsPageProps) {
  return (
    <div className="grid gap-6 xl:grid-cols-[1.15fr_0.85fr]">
      <SectionCard
        eyebrow="Foundation"
        title="App Runtime"
        description="The first implementation focuses on making the desktop runtime stable before we layer in Qwen orchestration and ops integrations."
      >
        {appHealth ? (
          <dl className="grid gap-4 md:grid-cols-2">
            <div className="rounded-lg border bg-muted/30 p-4">
              <dt className="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">App</dt>
              <dd className="mt-2 text-sm text-foreground">{appHealth.appName}</dd>
            </div>
            <div className="rounded-lg border bg-muted/30 p-4">
              <dt className="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">Version</dt>
              <dd className="mt-2 text-sm text-foreground">{appHealth.version}</dd>
            </div>
            <div className="rounded-lg border bg-muted/30 p-4">
              <dt className="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">Database</dt>
              <dd className="mt-2 text-sm text-foreground">
                {appHealth.databaseReady ? "Ready" : "Unavailable"}
              </dd>
            </div>
            <div className="rounded-lg border bg-muted/30 p-4">
              <dt className="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">Storage Path</dt>
              <dd className="mt-2 break-all text-sm text-foreground">{appHealth.storagePath}</dd>
            </div>
          </dl>
        ) : (
          <p className="text-sm text-muted-foreground">Loading runtime details...</p>
        )}
      </SectionCard>
      <SectionCard
        eyebrow="Environment Profiles"
        title="Default Environments"
        description="These seed profiles are stored locally so the app already has a predictable shape while we add real connection forms."
      >
        <ul className="space-y-3">
          {environments.map((environment) => (
            <li className="flex items-center justify-between gap-3 rounded-lg border bg-muted/30 p-3" key={environment.id}>
              <span className="text-sm text-foreground">
                {environment.name} with Kubernetes, ELK, SSH, Nacos, and Redis toggles
              </span>
              <Badge variant={environment.kind === "prod" ? "danger" : environment.kind === "test" ? "warning" : "success"}>
                {environment.kind}
              </Badge>
            </li>
          ))}
        </ul>
      </SectionCard>
    </div>
  );
}
