import { useEffect, useState } from "react";
import {
  ArrowRight,
  Bot,
  ClipboardList,
  Database,
  LayoutDashboard,
  ServerCog,
  ShieldCheck,
} from "lucide-react";
import { ChatPage } from "./features/chat/ChatPage";
import { InvestigationsPage } from "./features/investigations/InvestigationsPage";
import { ResourcesPage } from "./features/resources/ResourcesPage";
import { SettingsPage } from "./features/settings/SettingsPage";
import { getAppHealth, listEnvironments } from "./lib/tauri";
import { Button } from "./components/ui/button";
import { Badge } from "./components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./components/ui/card";
import type {
  AppHealth,
  AppSection,
  EnvironmentProfile,
  NavigationItem,
} from "./types/domain";

const navigationItems: NavigationItem[] = [
  {
    id: "overview",
    label: "Overview",
    description: "Roadmap and system foundation",
    icon: LayoutDashboard,
  },
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
    id: "settings",
    label: "Settings",
    description: "Runtime health and profiles",
    icon: ShieldCheck,
  },
];

export function App() {
  const [activeSection, setActiveSection] = useState<AppSection>("overview");
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

  return (
    <div className="min-h-screen bg-background">
      <div className="surface-grid min-h-screen">
        <div className="mx-auto grid min-h-screen max-w-[1600px] gap-6 p-6 xl:grid-cols-[290px_minmax(0,1fr)]">
          <aside className="flex flex-col gap-5 rounded-3xl border bg-card/80 p-5 shadow-sm backdrop-blur">
            <div className="space-y-4">
              <div className="inline-flex h-11 w-11 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-sm">
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
                      "flex items-start gap-3 rounded-xl border px-4 py-3 text-left transition",
                      item.id === activeSection
                        ? "border-primary/20 bg-primary/5 shadow-sm"
                        : "border-transparent hover:border-border hover:bg-muted/60",
                    ].join(" ")}
                    key={item.id}
                    onClick={() => setActiveSection(item.id)}
                    type="button"
                  >
                    <span className="mt-0.5 rounded-md bg-muted p-2">
                      <Icon className="h-4 w-4 text-foreground" />
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
            <Card className="mt-auto border-dashed bg-muted/40">
              <CardHeader className="pb-3">
                <CardTitle className="text-base">Current phase</CardTitle>
                <CardDescription>Week 1 foundation is live and ready for Qwen integration.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <Badge variant="secondary">10-week roadmap</Badge>
                <p className="text-sm leading-6 text-muted-foreground">
                  Next we can wire environment forms, model settings, and typed tool execution.
                </p>
              </CardContent>
            </Card>
          </aside>
          <main className="flex min-h-[calc(100vh-3rem)] flex-col gap-6">
            <header className="rounded-3xl border bg-card/85 p-6 shadow-sm backdrop-blur">
              <div className="flex flex-col gap-6 lg:flex-row lg:items-end lg:justify-between">
                <div className="space-y-3">
                  <Badge variant="outline">10-week MVP kickoff</Badge>
                  <div className="space-y-2">
                    <h2 className="font-serif text-4xl leading-tight">
                      Foundation for the full desktop app starts here
                    </h2>
                    <p className="max-w-3xl text-sm leading-6 text-muted-foreground">
                      The shell, runtime bridge, local database, and seeded environment model are in
                      place so we can build the real ops workflows on top.
                    </p>
                  </div>
                </div>
                <div className="flex flex-col gap-3">
                  <div className="inline-flex items-center gap-2 rounded-full border bg-background px-4 py-2 text-sm text-muted-foreground">
                    <span className="h-2.5 w-2.5 rounded-full bg-emerald-500" />
                    {loadError
                      ? `Bootstrap issue: ${loadError}`
                      : appHealth
                        ? `Runtime ready · SQLite ${appHealth.databaseReady ? "initialized" : "pending"}`
                        : "Bootstrapping runtime..."}
                  </div>
                  <div className="flex flex-wrap gap-3">
                    <Button>Continue Week 2 <ArrowRight className="h-4 w-4" /></Button>
                    <Button variant="outline">Open Roadmap</Button>
                  </div>
                </div>
              </div>
            </header>
            {activeSection === "overview" ? <OverviewPage environments={environments} /> : null}
            {activeSection === "chat" ? <ChatPage /> : null}
            {activeSection === "resources" ? <ResourcesPage environments={environments} /> : null}
            {activeSection === "investigations" ? <InvestigationsPage /> : null}
            {activeSection === "settings" ? (
              <SettingsPage appHealth={appHealth} environments={environments} onRefreshEnvironments={bootstrap} />
            ) : null}
          </main>
        </div>
      </div>
    </div>
  );
}

function OverviewPage({ environments }: { environments: EnvironmentProfile[] }) {
  return (
    <div className="grid gap-6">
      <Card className="overflow-hidden">
        <CardHeader className="gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="space-y-2">
            <p className="text-xs font-medium uppercase tracking-[0.24em] text-muted-foreground">
              What this scaffold gives us
            </p>
            <CardTitle className="font-serif text-3xl">
              Week 1 of the 10-week roadmap is now represented in code
            </CardTitle>
            <CardDescription className="max-w-3xl text-sm leading-6">
              The app already has a desktop shell, local runtime bridge, database bootstrapping, and
              seeded environments that map to your real ops systems.
            </CardDescription>
          </div>
          <div className="grid grid-cols-2 gap-3 text-sm">
            <MetricCard label="Screens" value="5" />
            <MetricCard label="Environments" value={String(environments.length)} />
            <MetricCard label="Runtime" value="Tauri" />
            <MetricCard label="Storage" value="SQLite" />
          </div>
        </CardHeader>
      </Card>
      <div className="grid gap-6 xl:grid-cols-[1.1fr_0.9fr]">
        <Card>
          <CardHeader>
            <CardTitle className="font-serif text-2xl">Current scope</CardTitle>
            <CardDescription>These are the core assets already in place for the first milestone.</CardDescription>
          </CardHeader>
          <CardContent>
            <ul className="space-y-3 text-sm leading-6 text-muted-foreground">
              <li>Desktop shell with dedicated pages for chat, resources, investigations, and settings</li>
              <li>Tauri backend foundation with app health and local SQLite initialization</li>
              <li>Seed environment profiles so we can build connection forms and adapters next</li>
            </ul>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="font-serif text-2xl">Roadmap bands</CardTitle>
            <CardDescription>The product is organized into the same development slices we discussed.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <MilestoneRow phase="Weeks 2-3" title="Environment Profiles and Qwen Chat" body="Credential management, settings forms, model client, and tool orchestration" />
            <MilestoneRow phase="Weeks 4-7" title="Integrations" body="Kubernetes, ELK, SSH diagnostics, and Nacos config diff" />
            <MilestoneRow phase="Weeks 8-10" title="Investigation and Safety" body="Report generation, approval flows, hardening, and packaging" />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function MetricCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border bg-muted/40 px-4 py-3">
      <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">{label}</p>
      <p className="mt-2 text-lg font-semibold">{value}</p>
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
