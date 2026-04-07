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
  deleteConnectionProfile,
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

type ProfileFormState = {
  id?: string;
  environmentId: string;
  profileType: ConnectionProfileType;
  name: string;
  endpoint: string;
  username: string;
  defaultScope: string;
  notes: string;
  secretValue: string;
  kubeconfigPath: string;
  kubeContext: string;
  sshHost: string;
  sshPort: string;
  sshAuthMode: "password" | "key" | "agent";
  sshPrivateKeyPath: string;
  elkIndexPattern: string;
  elkSpace: string;
  nacosNamespaceId: string;
  nacosGroup: string;
  nacosApiVersion: "v1" | "v2";
  nacosAuthMode: "basic" | "accessToken";
  redisDatabase: string;
  redisTlsEnabled: boolean;
  redisSlowlogLimit: string;
  qwenModel: string;
  qwenBasePath: string;
};

const emptyProfile = (): ProfileFormState => ({
  environmentId: "dev",
  profileType: "kubernetes",
  name: "",
  endpoint: "",
  username: "",
  defaultScope: "",
  notes: "",
  secretValue: "",
  kubeconfigPath: "",
  kubeContext: "",
  sshHost: "",
  sshPort: "22",
  sshAuthMode: "password",
  sshPrivateKeyPath: "",
  elkIndexPattern: "",
  elkSpace: "",
  nacosNamespaceId: "",
  nacosGroup: "DEFAULT_GROUP",
  nacosApiVersion: "v1",
  nacosAuthMode: "basic",
  redisDatabase: "0",
  redisTlsEnabled: false,
  redisSlowlogLimit: "5",
  qwenModel: "qwen-plus",
  qwenBasePath: "/compatible-mode/v1",
});

export function SettingsPage({
  appHealth,
  environments,
  onRefreshEnvironments,
}: SettingsPageProps) {
  const [environmentDrafts, setEnvironmentDrafts] = useState<UpsertEnvironmentInput[]>([]);
  const [connectionProfiles, setConnectionProfiles] = useState<ConnectionProfile[]>([]);
  const [profileDraft, setProfileDraft] = useState<ProfileFormState>(emptyProfile);
  const [validation, setValidation] = useState<ValidationResult | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [activeProfileId, setActiveProfileId] = useState<string | null>(null);

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
        ...emptyProfile(),
        environmentId: saved.environmentId,
      });
      setActiveProfileId(null);
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

  async function handleDeleteProfile(profileId: string) {
    setIsSaving(true);
    setStatusMessage(null);
    try {
      await deleteConnectionProfile(profileId);
      setConnectionProfiles((current) => current.filter((profile) => profile.id !== profileId));
      if (activeProfileId === profileId) {
        setProfileDraft({
          ...emptyProfile(),
          environmentId: environments[0]?.id ?? "dev",
        });
        setActiveProfileId(null);
        setValidation(null);
      }
      setStatusMessage("Connection profile deleted.");
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to delete profile.");
    } finally {
      setIsSaving(false);
    }
  }

  function handleEditProfile(profile: ConnectionProfile) {
    setProfileDraft(profileToDraft(profile));
    setActiveProfileId(profile.id);
    setValidation(null);
    setStatusMessage(`Editing ${profile.name}.`);
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
            <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-muted/20 p-3">
              <div>
                <p className="text-sm font-medium text-foreground">
                  {activeProfileId ? "Editing existing profile" : "Create a new profile"}
                </p>
                <p className="text-xs text-muted-foreground">
                  {activeProfileId
                    ? "Save will update the selected profile."
                    : "Fill in the fields below to add a new integration profile."}
                </p>
              </div>
              {activeProfileId ? (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    setProfileDraft({
                      ...emptyProfile(),
                      environmentId: environments[0]?.id ?? "dev",
                    });
                    setActiveProfileId(null);
                    setValidation(null);
                    setStatusMessage("Switched back to new profile mode.");
                  }}
                >
                  Cancel Edit
                </Button>
              ) : null}
            </div>
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
                    setProfileDraft(buildNextProfileTypeState(
                      profileDraft,
                      event.target.value as ConnectionProfileType,
                    ))
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
                  placeholder={endpointPlaceholder(profileDraft.profileType)}
                  value={profileDraft.endpoint}
                  onChange={(event) =>
                    setProfileDraft((current) => ({ ...current, endpoint: event.target.value }))
                  }
                />
              </Field>
              {renderTypeSpecificFields(profileDraft, setProfileDraft)}
            </div>
            <Field label="Connection summary">
              <div className="rounded-lg border bg-muted/20 p-4 text-xs leading-6 text-muted-foreground">
                <pre className="whitespace-pre-wrap break-all">
                  {JSON.stringify(composeStructuredConfig(profileDraft), null, 2)}
                </pre>
              </div>
              <p className="text-xs leading-5 text-muted-foreground">
                The app still stores structured config internally, but you no longer need to type raw JSON by hand.
              </p>
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
              <p className="text-xs leading-5 text-muted-foreground">
                Use this for passwords, tokens, or API keys. It is stored in the system keychain, not in SQLite.
              </p>
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
                          <p className="mt-2 break-all text-xs text-muted-foreground">
                            Config: {profile.configJson}
                          </p>
                          <div className="mt-3">
                            <div className="flex flex-wrap gap-2">
                              <Button
                                onClick={() => handleEditProfile(profile)}
                                size="sm"
                                variant="outline"
                                disabled={isSaving}
                              >
                                Edit
                              </Button>
                              <Button
                                onClick={() => void handleClearSecret(profile.id)}
                                size="sm"
                                variant="ghost"
                                disabled={!profile.hasSecret || isSaving}
                              >
                                Clear Secret
                              </Button>
                              <Button
                                onClick={() => void handleDeleteProfile(profile.id)}
                                size="sm"
                                variant="ghost"
                                disabled={isSaving}
                                className="text-rose-700 hover:bg-rose-50 hover:text-rose-800"
                              >
                                Delete
                              </Button>
                            </div>
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

function normalizeProfileDraft(draft: ProfileFormState): UpsertConnectionProfileInput {
  const normalizedEndpoint =
    draft.profileType === "ssh"
      ? draft.sshHost.trim() || draft.endpoint.trim()
      : draft.endpoint.trim();

  return {
    id: draft.id,
    environmentId: draft.environmentId,
    profileType: draft.profileType,
    name: draft.name.trim(),
    endpoint: normalizedEndpoint,
    username: draft.username?.trim() || undefined,
    defaultScope: draft.defaultScope?.trim() || undefined,
    notes: draft.notes?.trim() || undefined,
    configJson: JSON.stringify(composeStructuredConfig(draft)),
    secretValue: draft.secretValue?.trim() || undefined,
  };
}

function composeStructuredConfig(draft: ProfileFormState): Record<string, unknown> {
  switch (draft.profileType) {
    case "kubernetes":
      return {
        kubeconfigPath: draft.kubeconfigPath.trim() || undefined,
        context: draft.kubeContext.trim() || undefined,
      };
    case "ssh":
      return {
        host: draft.sshHost.trim() || undefined,
        port: toNumberOrUndefined(draft.sshPort),
        authMode: draft.sshAuthMode,
        privateKeyPath: draft.sshPrivateKeyPath.trim() || undefined,
      };
    case "elk":
      return {
        indexPattern: draft.elkIndexPattern.trim() || undefined,
        space: draft.elkSpace.trim() || undefined,
      };
    case "nacos":
      return {
        namespaceId: draft.nacosNamespaceId.trim() || undefined,
        group: draft.nacosGroup.trim() || undefined,
        apiVersion: draft.nacosApiVersion,
        authMode: draft.nacosAuthMode,
      };
    case "redis":
      return {
        database: toNumberOrUndefined(draft.redisDatabase),
        tlsEnabled: draft.redisTlsEnabled,
        slowlogLimit: toNumberOrUndefined(draft.redisSlowlogLimit),
      };
    case "qwen":
      return {
        basePath: draft.qwenBasePath.trim() || "/compatible-mode/v1",
      };
    default:
      return {};
  }
}

function endpointPlaceholder(profileType: ConnectionProfileType): string {
  switch (profileType) {
    case "kubernetes":
      return "https://k8s.example.com";
    case "elk":
      return "https://elk.example.com";
    case "ssh":
      return "10.0.0.8";
    case "nacos":
      return "https://nacos.example.com";
    case "redis":
      return "redis.example.com:6379";
    case "qwen":
      return "https://dashscope.aliyuncs.com";
    default:
      return "Endpoint";
  }
}

function buildNextProfileTypeState(
  current: ProfileFormState,
  nextType: ConnectionProfileType,
): ProfileFormState {
  const base = {
    ...current,
    profileType: nextType,
    secretValue: "",
  };

  switch (nextType) {
    case "kubernetes":
      return {
        ...base,
        defaultScope: current.defaultScope || "default",
        username: "",
      };
    case "elk":
      return {
        ...base,
        defaultScope: current.defaultScope || "logs-*",
      };
    case "ssh":
      return {
        ...base,
        username: current.username || "root",
      };
    case "nacos":
      return {
        ...base,
        defaultScope: current.defaultScope || "DEFAULT_GROUP",
      };
    case "redis":
      return {
        ...base,
        defaultScope: current.defaultScope || "0",
      };
    case "qwen":
      return {
        ...base,
        defaultScope: current.defaultScope || "qwen-plus",
      };
    default:
      return base;
  }
}

function renderTypeSpecificFields(
  profileDraft: ProfileFormState,
  setProfileDraft: React.Dispatch<React.SetStateAction<ProfileFormState>>,
) {
  switch (profileDraft.profileType) {
    case "kubernetes":
      return (
        <>
          <Field label="Kubeconfig path">
            <Input
              placeholder="/Users/you/.kube/config"
              value={profileDraft.kubeconfigPath}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, kubeconfigPath: event.target.value }))
              }
            />
          </Field>
          <Field label="Context">
            <Input
              placeholder="prod-cluster"
              value={profileDraft.kubeContext}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, kubeContext: event.target.value }))
              }
            />
          </Field>
          <Field label="Default namespace">
            <Input
              placeholder="default"
              value={profileDraft.defaultScope}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, defaultScope: event.target.value }))
              }
            />
          </Field>
        </>
      );
    case "elk":
      return (
        <>
          <Field label="Username">
            <Input
              placeholder="elastic"
              value={profileDraft.username}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, username: event.target.value }))
              }
            />
          </Field>
          <Field label="Index pattern">
            <Input
              placeholder="logs-*"
              value={profileDraft.elkIndexPattern}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, elkIndexPattern: event.target.value }))
              }
            />
          </Field>
          <Field label="Space or tenant">
            <Input
              placeholder="observability"
              value={profileDraft.elkSpace}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, elkSpace: event.target.value }))
              }
            />
          </Field>
        </>
      );
    case "ssh":
      return (
        <>
          <Field label="Username">
            <Input
              placeholder="ops-user"
              value={profileDraft.username}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, username: event.target.value }))
              }
            />
          </Field>
          <Field label="Host">
            <Input
              placeholder="app-prod-01.internal"
              value={profileDraft.sshHost}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, sshHost: event.target.value }))
              }
            />
          </Field>
          <Field label="Port">
            <Input
              placeholder="22"
              value={profileDraft.sshPort}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, sshPort: event.target.value }))
              }
            />
          </Field>
          <Field label="Auth mode">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={profileDraft.sshAuthMode}
              onChange={(event) =>
                setProfileDraft((current) => ({
                  ...current,
                  sshAuthMode: event.target.value as ProfileFormState["sshAuthMode"],
                }))
              }
            >
              <option value="password">password</option>
              <option value="key">private key</option>
              <option value="agent">ssh agent</option>
            </select>
          </Field>
          {profileDraft.sshAuthMode === "key" ? (
            <Field label="Private key path">
              <Input
                placeholder="~/.ssh/id_ed25519"
                value={profileDraft.sshPrivateKeyPath}
                onChange={(event) =>
                  setProfileDraft((current) => ({ ...current, sshPrivateKeyPath: event.target.value }))
                }
              />
            </Field>
          ) : null}
        </>
      );
    case "nacos":
      return (
        <>
          <Field label="Username">
            <Input
              placeholder="nacos"
              value={profileDraft.username}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, username: event.target.value }))
              }
            />
          </Field>
          <Field label="Namespace ID">
            <Input
              placeholder="public"
              value={profileDraft.nacosNamespaceId}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, nacosNamespaceId: event.target.value }))
              }
            />
          </Field>
          <Field label="Default group">
            <Input
              placeholder="DEFAULT_GROUP"
              value={profileDraft.nacosGroup}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, nacosGroup: event.target.value }))
              }
            />
          </Field>
          <Field label="API version">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={profileDraft.nacosApiVersion}
              onChange={(event) =>
                setProfileDraft((current) => ({
                  ...current,
                  nacosApiVersion: event.target.value as ProfileFormState["nacosApiVersion"],
                }))
              }
            >
              <option value="v1">v1</option>
              <option value="v2">v2</option>
            </select>
          </Field>
          <Field label="Auth mode">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={profileDraft.nacosAuthMode}
              onChange={(event) =>
                setProfileDraft((current) => ({
                  ...current,
                  nacosAuthMode: event.target.value as ProfileFormState["nacosAuthMode"],
                }))
              }
            >
              <option value="basic">basic</option>
              <option value="accessToken">access token</option>
            </select>
          </Field>
        </>
      );
    case "redis":
      return (
        <>
          <Field label="Username">
            <Input
              placeholder="default"
              value={profileDraft.username}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, username: event.target.value }))
              }
            />
          </Field>
          <Field label="Database">
            <Input
              placeholder="0"
              value={profileDraft.redisDatabase}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, redisDatabase: event.target.value }))
              }
            />
          </Field>
          <Field label="Slowlog sample limit">
            <Input
              placeholder="5"
              value={profileDraft.redisSlowlogLimit}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, redisSlowlogLimit: event.target.value }))
              }
            />
          </Field>
          <Field label="Default key prefix">
            <Input
              placeholder="payment:*"
              value={profileDraft.defaultScope}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, defaultScope: event.target.value }))
              }
            />
          </Field>
          <ToggleField
            checked={profileDraft.redisTlsEnabled}
            label="TLS enabled"
            onChange={(checked) =>
              setProfileDraft((current) => ({ ...current, redisTlsEnabled: checked }))
            }
          />
        </>
      );
    case "qwen":
      return (
        <>
          <Field label="Model">
            <Input
              placeholder="qwen-plus"
              value={profileDraft.qwenModel}
              onChange={(event) =>
                setProfileDraft((current) => ({
                  ...current,
                  qwenModel: event.target.value,
                  defaultScope: event.target.value,
                }))
              }
            />
          </Field>
          <Field label="Base path">
            <Input
              placeholder="/compatible-mode/v1"
              value={profileDraft.qwenBasePath}
              onChange={(event) =>
                setProfileDraft((current) => ({ ...current, qwenBasePath: event.target.value }))
              }
            />
          </Field>
        </>
      );
    default:
      return null;
  }
}

function toNumberOrUndefined(value: string): number | undefined {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function profileToDraft(profile: ConnectionProfile): ProfileFormState {
  const rawConfig = tryParseConfig(profile.configJson);

  return {
    ...emptyProfile(),
    id: profile.id,
    environmentId: profile.environmentId,
    profileType: profile.profileType as ConnectionProfileType,
    name: profile.name,
    endpoint: profile.endpoint,
    username: profile.username ?? "",
    defaultScope: profile.defaultScope ?? "",
    notes: profile.notes ?? "",
    secretValue: "",
    kubeconfigPath: stringValue(rawConfig.kubeconfigPath),
    kubeContext: stringValue(rawConfig.context),
    sshHost: stringValue(rawConfig.host) || profile.endpoint,
    sshPort: stringValue(rawConfig.port) || "22",
    sshAuthMode: sshAuthModeValue(rawConfig.authMode),
    sshPrivateKeyPath: stringValue(rawConfig.privateKeyPath),
    elkIndexPattern: stringValue(rawConfig.indexPattern),
    elkSpace: stringValue(rawConfig.space),
    nacosNamespaceId: stringValue(rawConfig.namespaceId),
    nacosGroup: stringValue(rawConfig.group) || "DEFAULT_GROUP",
    nacosApiVersion: nacosApiVersionValue(rawConfig.apiVersion),
    nacosAuthMode: nacosAuthModeValue(rawConfig.authMode),
    redisDatabase: stringValue(rawConfig.database) || "0",
    redisTlsEnabled: booleanValue(rawConfig.tlsEnabled),
    redisSlowlogLimit: stringValue(rawConfig.slowlogLimit) || "5",
    qwenModel: profile.defaultScope || "qwen-plus",
    qwenBasePath: stringValue(rawConfig.basePath) || "/compatible-mode/v1",
  };
}

function tryParseConfig(configJson: string): Record<string, unknown> {
  try {
    const parsed = JSON.parse(configJson) as Record<string, unknown>;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

function stringValue(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number") {
    return String(value);
  }
  return "";
}

function booleanValue(value: unknown): boolean {
  return value === true;
}

function sshAuthModeValue(value: unknown): ProfileFormState["sshAuthMode"] {
  return value === "key" || value === "agent" ? value : "password";
}

function nacosApiVersionValue(value: unknown): ProfileFormState["nacosApiVersion"] {
  return value === "v2" ? "v2" : "v1";
}

function nacosAuthModeValue(value: unknown): ProfileFormState["nacosAuthMode"] {
  return value === "accessToken" ? "accessToken" : "basic";
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
