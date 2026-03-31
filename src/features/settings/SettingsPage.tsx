import { useEffect, useMemo, useState } from "react";
import type {
  AppHealth,
  ConnectionProfile,
  ConnectionProfileType,
  EnvironmentProfile,
  UpsertConnectionProfileInput,
  UpsertEnvironmentInput,
  ValidationResult,
} from "../../types/domain";
import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Textarea } from "../../components/ui/textarea";
import {
  clearConnectionProfileSecret,
  listConnectionProfiles,
  saveConnectionProfile,
  saveEnvironment,
  validateConnectionProfile,
} from "../../lib/tauri";

interface SettingsPageProps {
  appHealth: AppHealth | null;
  environments: EnvironmentProfile[];
  onRefreshEnvironments: () => Promise<void>;
}

const profileTypeOptions: ConnectionProfileType[] = [
  "kubernetes",
  "elk",
  "ssh",
  "nacos",
  "redis",
  "qwen",
];

const emptyProfile: UpsertConnectionProfileInput = {
  environmentId: "dev",
  profileType: "kubernetes",
  name: "",
  endpoint: "",
  username: "",
  defaultScope: "",
  notes: "",
  configJson: "{}",
  secretValue: "",
};

export function SettingsPage({
  appHealth,
  environments,
  onRefreshEnvironments,
}: SettingsPageProps) {
  const [environmentDrafts, setEnvironmentDrafts] = useState<UpsertEnvironmentInput[]>([]);
  const [connectionProfiles, setConnectionProfiles] = useState<ConnectionProfile[]>([]);
  const [profileDraft, setProfileDraft] = useState<UpsertConnectionProfileInput>(emptyProfile);
  const [validation, setValidation] = useState<ValidationResult | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    setEnvironmentDrafts(environments.map((environment) => ({ ...environment })));
  }, [environments]);

  useEffect(() => {
    if (environments.length > 0) {
      setProfileDraft((current) => ({
        ...current,
        environmentId: current.environmentId || environments[0].id,
      }));
    }
  }, [environments]);

  useEffect(() => {
    async function loadProfiles() {
      try {
        const profiles = await listConnectionProfiles();
        setConnectionProfiles(profiles);
      } catch (error) {
        const message = error instanceof Error ? error.message : "Failed to load profiles";
        setStatusMessage(message);
      }
    }

    void loadProfiles();
  }, []);

  const groupedProfiles = useMemo(() => {
    return environmentDrafts.map((environment) => ({
      environment,
      profiles: connectionProfiles.filter((profile) => profile.environmentId === environment.id),
    }));
  }, [connectionProfiles, environmentDrafts]);

  async function handleEnvironmentSave(environment: UpsertEnvironmentInput) {
    setIsSaving(true);
    setStatusMessage(null);
    try {
      await saveEnvironment(environment);
      await onRefreshEnvironments();
      setStatusMessage(`Saved ${environment.name}.`);
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to save environment.");
    } finally {
      setIsSaving(false);
    }
  }

  async function handleValidateProfile() {
    setStatusMessage(null);
    try {
      const result = await validateConnectionProfile(normalizeProfileDraft(profileDraft));
      setValidation(result);
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Validation failed.");
    }
  }

  async function handleSaveProfile() {
    setIsSaving(true);
    setStatusMessage(null);
    try {
      const saved = await saveConnectionProfile(normalizeProfileDraft(profileDraft));
      setConnectionProfiles((current) => {
        const next = current.filter((profile) => profile.id !== saved.id);
        return [...next, saved].sort((left, right) => left.name.localeCompare(right.name));
      });
      setProfileDraft({
        ...emptyProfile,
        environmentId: saved.environmentId,
      });
      setValidation({
        ok: true,
        messages: [`Saved connection profile ${saved.name}.`],
      });
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to save profile.");
    } finally {
      setIsSaving(false);
    }
  }

  async function handleClearSecret(profileId: string) {
    setIsSaving(true);
    setStatusMessage(null);
    try {
      await clearConnectionProfileSecret(profileId);
      setConnectionProfiles((current) =>
        current.map((profile) =>
          profile.id === profileId ? { ...profile, hasSecret: false } : profile,
        ),
      );
      setStatusMessage("Secret cleared from system keychain.");
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to clear secret.");
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="grid gap-6">
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
      <div className="grid gap-6 xl:grid-cols-[1fr_1.1fr]">
        <SectionCard
          eyebrow="Environment Profiles"
          title="Environment Toggles"
          description="Update the core environment metadata here. This is the base layer for connection profiles and future access policy."
        >
          <div className="space-y-4">
            {environmentDrafts.map((environment, index) => (
              <div className="rounded-xl border bg-muted/30 p-4" key={environment.id}>
                <div className="mb-4 flex items-center justify-between">
                  <div>
                    <p className="font-medium text-foreground">{environment.name}</p>
                    <p className="text-sm text-muted-foreground">Environment ID: {environment.id}</p>
                  </div>
                  <Badge
                    variant={
                      environment.kind === "prod"
                        ? "danger"
                        : environment.kind === "test"
                          ? "warning"
                          : "success"
                    }
                  >
                    {environment.kind}
                  </Badge>
                </div>
                <div className="grid gap-4 md:grid-cols-2">
                  <Field label="Display name">
                    <Input
                      value={environment.name}
                      onChange={(event) =>
                        updateEnvironmentDraft(index, "name", event.target.value, setEnvironmentDrafts)
                      }
                    />
                  </Field>
                  <Field label="Kind">
                    <select
                      className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
                      value={environment.kind}
                      onChange={(event) =>
                        updateEnvironmentDraft(
                          index,
                          "kind",
                          event.target.value as EnvironmentProfile["kind"],
                          setEnvironmentDrafts,
                        )
                      }
                    >
                      <option value="dev">dev</option>
                      <option value="test">test</option>
                      <option value="prod">prod</option>
                    </select>
                  </Field>
                </div>
                <div className="mt-4 grid gap-3 sm:grid-cols-2">
                  <ToggleField
                    checked={environment.kubernetesEnabled}
                    label="Kubernetes"
                    onChange={(checked) =>
                      updateEnvironmentDraft(index, "kubernetesEnabled", checked, setEnvironmentDrafts)
                    }
                  />
                  <ToggleField
                    checked={environment.elkEnabled}
                    label="ELK"
                    onChange={(checked) =>
                      updateEnvironmentDraft(index, "elkEnabled", checked, setEnvironmentDrafts)
                    }
                  />
                  <ToggleField
                    checked={environment.sshEnabled}
                    label="SSH"
                    onChange={(checked) =>
                      updateEnvironmentDraft(index, "sshEnabled", checked, setEnvironmentDrafts)
                    }
                  />
                  <ToggleField
                    checked={environment.nacosEnabled}
                    label="Nacos"
                    onChange={(checked) =>
                      updateEnvironmentDraft(index, "nacosEnabled", checked, setEnvironmentDrafts)
                    }
                  />
                  <ToggleField
                    checked={environment.redisEnabled}
                    label="Redis"
                    onChange={(checked) =>
                      updateEnvironmentDraft(index, "redisEnabled", checked, setEnvironmentDrafts)
                    }
                  />
                </div>
                <div className="mt-4">
                  <Button onClick={() => void handleEnvironmentSave(environment)} size="sm" disabled={isSaving}>
                    Save Environment
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </SectionCard>
        <SectionCard
          eyebrow="Connection Profiles"
          title="Profile Registry"
          description="Store profile metadata in SQLite and keep secrets in the system keychain. Validation here is local-first and safe."
        >
          <div className="space-y-5">
            <div className="grid gap-4 md:grid-cols-2">
              <Field label="Environment">
                <select
                  className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
                  value={profileDraft.environmentId}
                  onChange={(event) =>
                    setProfileDraft((current) => ({ ...current, environmentId: event.target.value }))
                  }
                >
                  {environments.map((environment) => (
                    <option key={environment.id} value={environment.id}>
                      {environment.name}
                    </option>
                  ))}
                </select>
              </Field>
              <Field label="Profile type">
                <select
                  className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
                  value={profileDraft.profileType}
                  onChange={(event) =>
                    setProfileDraft((current) => ({
                      ...current,
                      profileType: event.target.value as ConnectionProfileType,
                    }))
                  }
                >
                  {profileTypeOptions.map((option) => (
                    <option key={option} value={option}>
                      {option}
                    </option>
                  ))}
                </select>
              </Field>
              <Field label="Profile name">
                <Input
                  placeholder="Prod Kubernetes API"
                  value={profileDraft.name}
                  onChange={(event) =>
                    setProfileDraft((current) => ({ ...current, name: event.target.value }))
                  }
                />
              </Field>
              <Field label="Endpoint or host">
                <Input
                  placeholder="https://k8s.example.com or 10.0.0.8:22"
                  value={profileDraft.endpoint}
                  onChange={(event) =>
                    setProfileDraft((current) => ({ ...current, endpoint: event.target.value }))
                  }
                />
              </Field>
              <Field label="Username">
                <Input
                  placeholder="ops-user"
                  value={profileDraft.username}
                  onChange={(event) =>
                    setProfileDraft((current) => ({ ...current, username: event.target.value }))
                  }
                />
              </Field>
              <Field label="Default scope">
                <Input
                  placeholder="namespace, index pattern, or config group"
                  value={profileDraft.defaultScope}
                  onChange={(event) =>
                    setProfileDraft((current) => ({ ...current, defaultScope: event.target.value }))
                  }
                />
              </Field>
            </div>
            <Field label="Extra JSON">
              <Textarea
                className="min-h-28"
                value={profileDraft.configJson}
                onChange={(event) =>
                  setProfileDraft((current) => ({ ...current, configJson: event.target.value }))
                }
              />
              {profileDraft.profileType === "ssh" ? (
                <p className="text-xs leading-5 text-muted-foreground">
                  SSH supports <code>{"{\"authMode\":\"agent\"}"}</code> or{" "}
                  <code>{"{\"authMode\":\"key\",\"privateKeyPath\":\"~/.ssh/id_ed25519\",\"port\":22}"}</code>.
                  Password auth is not wired yet in execution mode.
                </p>
              ) : null}
            </Field>
            <Field label="Notes">
              <Textarea
                className="min-h-24"
                value={profileDraft.notes}
                onChange={(event) =>
                  setProfileDraft((current) => ({ ...current, notes: event.target.value }))
                }
              />
            </Field>
            <Field label="Secret value">
              <Input
                type="password"
                placeholder="API key, password, or token"
                value={profileDraft.secretValue}
                onChange={(event) =>
                  setProfileDraft((current) => ({ ...current, secretValue: event.target.value }))
                }
              />
            </Field>
            <div className="flex flex-wrap gap-3">
              <Button onClick={() => void handleValidateProfile()} variant="outline" disabled={isSaving}>
                Validate Profile
              </Button>
              <Button onClick={() => void handleSaveProfile()} disabled={isSaving}>
                Save Profile
              </Button>
            </div>
            {validation ? (
              <div className="rounded-lg border bg-muted/30 p-4">
                <p className="text-sm font-medium text-foreground">
                  {validation.ok ? "Validation passed" : "Validation needs attention"}
                </p>
                <ul className="mt-2 space-y-2 text-sm text-muted-foreground">
                  {validation.messages.map((message) => (
                    <li key={message}>{message}</li>
                  ))}
                </ul>
              </div>
            ) : null}
            {statusMessage ? <p className="text-sm text-muted-foreground">{statusMessage}</p> : null}
            <div className="space-y-4">
              {groupedProfiles.map(({ environment, profiles }) => (
                <div key={environment.id}>
                  <div className="mb-3 flex items-center gap-2">
                    <p className="text-sm font-semibold">{environment.name}</p>
                    <Badge variant="outline">{profiles.length} profiles</Badge>
                  </div>
                  <div className="space-y-3">
                    {profiles.length === 0 ? (
                      <div className="rounded-lg border border-dashed bg-muted/20 p-3 text-sm text-muted-foreground">
                        No profiles saved yet.
                      </div>
                    ) : (
                      profiles.map((profile) => (
                        <div className="rounded-lg border bg-muted/20 p-4" key={profile.id}>
                          <div className="flex flex-wrap items-start justify-between gap-3">
                            <div>
                              <p className="font-medium text-foreground">{profile.name}</p>
                              <p className="text-sm text-muted-foreground">
                                {profile.profileType} · {profile.endpoint || "no endpoint"}
                              </p>
                            </div>
                            <div className="flex flex-wrap gap-2">
                              <Badge variant="secondary">{profile.profileType}</Badge>
                              <Badge variant={profile.hasSecret ? "success" : "outline"}>
                                {profile.hasSecret ? "Secret stored" : "No secret"}
                              </Badge>
                            </div>
                          </div>
                          <p className="mt-3 text-sm text-muted-foreground">
                            Scope: {profile.defaultScope || "n/a"} · User: {profile.username || "n/a"}
                          </p>
                          <div className="mt-3">
                            <Button
                              onClick={() => void handleClearSecret(profile.id)}
                              size="sm"
                              variant="ghost"
                              disabled={!profile.hasSecret || isSaving}
                            >
                              Clear Secret
                            </Button>
                          </div>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </SectionCard>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="space-y-2">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

function ToggleField({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-3 rounded-lg border bg-background px-3 py-2 text-sm">
      <input checked={checked} onChange={(event) => onChange(event.target.checked)} type="checkbox" />
      <span>{label}</span>
    </label>
  );
}

function normalizeProfileDraft(draft: UpsertConnectionProfileInput): UpsertConnectionProfileInput {
  return {
    ...draft,
    username: draft.username?.trim() || undefined,
    defaultScope: draft.defaultScope?.trim() || undefined,
    notes: draft.notes?.trim() || undefined,
    configJson: draft.configJson?.trim() || "{}",
    secretValue: draft.secretValue?.trim() || undefined,
  };
}

function updateEnvironmentDraft(
  index: number,
  key: keyof UpsertEnvironmentInput,
  value: string | boolean,
  setter: React.Dispatch<React.SetStateAction<UpsertEnvironmentInput[]>>,
) {
  setter((current) =>
    current.map((environment, currentIndex) =>
      currentIndex === index ? { ...environment, [key]: value } : environment,
    ),
  );
}
