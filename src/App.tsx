import { useEffect, useState } from "react";
import {
  Bot,
  ClipboardList,
  Database,
  ServerCog,
  ShieldCheck,
  ShieldEllipsis,
} from "lucide-react";
import { ApprovalsPage } from "./features/approvals/ApprovalsPage";
import { ChatPage } from "./features/chat/ChatPage";
import { InvestigationsPage } from "./features/investigations/InvestigationsPage";
import { ResourcesPage } from "./features/resources/ResourcesPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { getAppHealth, listEnvironments } from "./lib/tauri";
import { Badge } from "./components/ui/badge";
import type {
  AppHealth,
  AppSection,
  EnvironmentProfile,
  NavigationItem,
} from "./types/domain";

const navigationItems: NavigationItem[] = [
  {
    id: "chat",
    label: "Chat",
    description: "LLM troubleshooting cockpit",
    icon: Bot,
  },
  {
    id: "resources",
    label: "Resources",
    description: "Kubernetes, servers, logs, and configs",
    icon: ServerCog,
  },
  {
    id: "investigations",
    label: "Investigations",
    description: "Timeline and reports",
    icon: ClipboardList,
  },
  {
    id: "approvals",
    label: "Approvals",
    description: "Controlled operations",
    icon: ShieldEllipsis,
  },
  {
    id: "settings",
    label: "Settings",
    description: "Runtime health and profiles",
    icon: ShieldCheck,
  },
];

export function App() {
  const [activeSection, setActiveSection] = useState<AppSection>("resources");
  const [appHealth, setAppHealth] = useState<AppHealth | null>(null);
  const [environments, setEnvironments] = useState<EnvironmentProfile[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  async function bootstrap() {
    try {
      const [health, envs] = await Promise.all([getAppHealth(), listEnvironments()]);
      setAppHealth(health);
      setEnvironments(envs);
      setLoadError(null);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown bootstrap error";
      setLoadError(message);
    }
  }

  useEffect(() => {
    void bootstrap();
  }, []);

  const activeItem = navigationItems.find((item) => item.id === activeSection) ?? navigationItems[0];
  const environmentCount = environments.length;
  const enabledLabels = [
    environments.some((environment) => environment.kubernetesEnabled) ? "Kubernetes" : null,
    environments.some((environment) => environment.elkEnabled) ? "ELK" : null,
    environments.some((environment) => environment.sshEnabled) ? "SSH" : null,
    environments.some((environment) => environment.nacosEnabled) ? "Nacos" : null,
  ].filter(Boolean) as string[];

  return (
    <div className="min-h-screen bg-background">
      <div className="surface-grid min-h-screen">
        <div className="mx-auto grid min-h-screen max-w-[1600px] gap-6 p-6 xl:grid-cols-[290px_minmax(0,1fr)]">
          <aside className="flex flex-col gap-5 rounded-[2rem] border border-white/45 bg-card/75 p-5 shadow-[0_28px_90px_-42px_rgba(14,116,144,0.55)] backdrop-blur-xl">
            <div className="space-y-4">
              <div className="inline-flex h-11 w-11 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-[0_14px_38px_-16px_rgba(14,116,144,0.9)]">
                <Database className="h-5 w-5" />
              </div>
              <div className="space-y-2">
                <p className="text-xs font-medium uppercase tracking-[0.28em] text-muted-foreground">
                  ADBCHelper
                </p>
                <h1 className="font-serif text-3xl leading-tight">Desktop Ops Copilot</h1>
                <p className="text-sm leading-6 text-muted-foreground">
                  A local-first desktop app for Kubernetes, ELK, SSH, Nacos, Redis, and LLM-guided
                  investigations.
                </p>
              </div>
            </div>
            <nav aria-label="Primary" className="grid gap-2">
              {navigationItems.map((item) => {
                const Icon = item.icon;

                return (
                  <button
                    className={[
                      "flex items-start gap-3 rounded-2xl border px-4 py-3 text-left transition duration-200",
                      item.id === activeSection
                        ? "border-cyan-400/40 bg-gradient-to-r from-cyan-500/15 via-sky-500/10 to-amber-400/15 shadow-[0_18px_44px_-28px_rgba(14,116,144,0.9)]"
                        : "border-transparent hover:border-cyan-900/10 hover:bg-white/50",
                    ].join(" ")}
                    key={item.id}
                    onClick={() => setActiveSection(item.id)}
                    type="button"
                  >
                    <span className="mt-0.5 rounded-xl bg-white/70 p-2 shadow-sm ring-1 ring-white/60">
                      <Icon className="h-4 w-4 text-foreground/90" />
                    </span>
                    <span className="block">
                      <span className="block text-sm font-medium">{item.label}</span>
                      <span className="mt-1 block text-xs leading-5 text-muted-foreground">
                        {item.description}
                      </span>
                    </span>
                  </button>
                );
              })}
            </nav>
          </aside>
          <main className="flex min-h-[calc(100vh-3rem)] flex-col gap-6">
            <header className="overflow-hidden rounded-[2rem] border border-white/45 bg-card/72 p-6 shadow-[0_30px_90px_-42px_rgba(15,23,42,0.45)] backdrop-blur-xl">
              <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-cyan-400/70 to-transparent" />
              <div className="relative flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
                <div className="space-y-3">
                  <Badge className="border-0 bg-cyan-500/12 text-cyan-900 hover:bg-cyan-500/12">
                    {activeItem.label}
                  </Badge>
                  <div className="space-y-2">
                    <h2 className="font-serif text-4xl leading-tight">{activeItem.description}</h2>
                    <p className="max-w-3xl text-sm leading-6 text-muted-foreground">
                      Work directly with the live tools that are already wired into the desktop app.
                    </p>
                  </div>
                </div>
                <div className="flex flex-col gap-3 lg:items-end">
                  <div className="inline-flex items-center gap-2 rounded-full border border-white/60 bg-white/70 px-4 py-2 text-sm text-muted-foreground shadow-sm">
                    <span
                      className={[
                        "h-2.5 w-2.5 rounded-full",
                        loadError ? "bg-rose-500" : "bg-emerald-500",
                      ].join(" ")}
                    />
                    {loadError
                      ? `Bootstrap issue: ${loadError}`
                      : appHealth
                        ? `Runtime ready · SQLite ${appHealth.databaseReady ? "initialized" : "pending"}`
                        : "Bootstrapping runtime..."}
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <Badge variant="secondary" className="bg-amber-400/16 text-amber-950">
                      {environmentCount} environment{environmentCount === 1 ? "" : "s"}
                    </Badge>
                    {enabledLabels.slice(0, 4).map((label) => (
                      <Badge key={label} variant="secondary" className="bg-sky-500/10 text-sky-950">
                        {label}
                      </Badge>
                    ))}
                  </div>
                </div>
              </div>
            </header>
            {activeSection === "chat" ? <ChatPage /> : null}
            {activeSection === "resources" ? <ResourcesPage environments={environments} /> : null}
            {activeSection === "investigations" ? <InvestigationsPage /> : null}
            {activeSection === "approvals" ? <ApprovalsPage environments={environments} /> : null}
            {activeSection === "settings" ? (
              <SettingsPage appHealth={appHealth} environments={environments} onRefreshEnvironments={bootstrap} />
            ) : null}
          </main>
        </div>
      </div>
    </div>
  );
}

function MilestoneRow({ phase, title, body }: { phase: string; title: string; body: string }) {
  return (
    <div className="rounded-xl border bg-muted/30 p-4">
      <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">{phase}</p>
      <h3 className="mt-2 text-base font-semibold">{title}</h3>
      <p className="mt-1 text-sm leading-6 text-muted-foreground">{body}</p>
    </div>
  );
}
