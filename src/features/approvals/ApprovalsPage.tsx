import { useEffect, useState } from "react";
import { SectionCard } from "../../components/SectionCard";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import {
  approveRequest,
  createApprovalRequest,
  executeApprovalRequest,
  listApprovalRequests,
} from "../../lib/tauri";
import type { ApprovalActionType, ApprovalRequest, EnvironmentProfile } from "../../types/domain";

interface ApprovalsPageProps {
  environments: EnvironmentProfile[];
}

const actionOptions: { value: ApprovalActionType; label: string }[] = [
  { value: "restart_pod", label: "Restart pod" },
  { value: "scale_deployment", label: "Scale deployment" },
  { value: "reload_nginx", label: "Reload nginx" },
];

export function ApprovalsPage({ environments }: ApprovalsPageProps) {
  const defaultEnvironmentId = environments[0]?.id ?? "dev";
  const [requests, setRequests] = useState<ApprovalRequest[]>([]);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [draft, setDraft] = useState({
    environmentId: defaultEnvironmentId,
    actionType: "restart_pod" as ApprovalActionType,
    namespace: "default",
    podName: "",
    deploymentName: "",
    replicas: "2",
    host: "",
    rationale: "",
    rollbackHint: "",
  });

  useEffect(() => {
    if (environments.length > 0) {
      setDraft((current) => ({
        ...current,
        environmentId: current.environmentId || environments[0].id,
      }));
    }
  }, [environments]);

  useEffect(() => {
    void refreshRequests();
  }, []);

  async function refreshRequests() {
    try {
      const next = await listApprovalRequests();
      setRequests(next);
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to load approval requests.");
    }
  }

  async function handleCreate() {
    try {
      const payload = buildTargetPayload(draft);
      const request = await createApprovalRequest({
        environmentId: draft.environmentId,
        actionType: draft.actionType,
        targetRef: payload.targetRef,
        targetDetailsJson: JSON.stringify(payload.details),
        rationale: draft.rationale.trim(),
        rollbackHint: draft.rollbackHint.trim() || undefined,
      });
      setStatusMessage(`Created approval request ${request.actionType} for ${request.targetRef}.`);
      await refreshRequests();
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to create approval request.");
    }
  }

  async function handleApprove(approvalId: string) {
    try {
      const request = await approveRequest(approvalId);
      setStatusMessage(`Approved ${request.actionType} for ${request.targetRef}.`);
      await refreshRequests();
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to approve request.");
    }
  }

  async function handleExecute(approvalId: string) {
    try {
      const request = await executeApprovalRequest({ approvalId });
      setStatusMessage(request.executionSummary ?? `Executed ${request.actionType}.`);
      await refreshRequests();
    } catch (error) {
      setStatusMessage(error instanceof Error ? error.message : "Failed to execute request.");
    }
  }

  return (
    <div className="grid gap-6 xl:grid-cols-[0.92fr_1.08fr]">
      <SectionCard
        eyebrow="Week 9"
        title="Approval Center"
        description="Stage risky actions here, classify risk before execution, and require a separate approval step before the app performs a limited write action."
      >
        <div className="grid gap-4 md:grid-cols-2">
          <Field label="Environment">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={draft.environmentId}
              onChange={(event) => setDraft((current) => ({ ...current, environmentId: event.target.value }))}
            >
              {environments.map((environment) => (
                <option key={environment.id} value={environment.id}>
                  {environment.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Action">
            <select
              className="flex h-10 w-full rounded-md border bg-background px-3 py-2 text-sm"
              value={draft.actionType}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  actionType: event.target.value as ApprovalActionType,
                }))
              }
            >
              {actionOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </Field>
          {draft.actionType === "restart_pod" || draft.actionType === "scale_deployment" ? (
            <Field label="Namespace">
              <Input
                value={draft.namespace}
                onChange={(event) => setDraft((current) => ({ ...current, namespace: event.target.value }))}
              />
            </Field>
          ) : null}
          {draft.actionType === "restart_pod" ? (
            <Field label="Pod name">
              <Input
                value={draft.podName}
                onChange={(event) => setDraft((current) => ({ ...current, podName: event.target.value }))}
              />
            </Field>
          ) : null}
          {draft.actionType === "scale_deployment" ? (
            <>
              <Field label="Deployment name">
                <Input
                  value={draft.deploymentName}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, deploymentName: event.target.value }))
                  }
                />
              </Field>
              <Field label="Replica count">
                <Input
                  value={draft.replicas}
                  onChange={(event) => setDraft((current) => ({ ...current, replicas: event.target.value }))}
                />
              </Field>
            </>
          ) : null}
          {draft.actionType === "reload_nginx" ? (
            <Field label="Host">
              <Input
                placeholder="web-prod-01.internal"
                value={draft.host}
                onChange={(event) => setDraft((current) => ({ ...current, host: event.target.value }))}
              />
            </Field>
          ) : null}
        </div>
        <Field label="Rationale">
          <Input
            placeholder="Why is this operation needed?"
            value={draft.rationale}
            onChange={(event) => setDraft((current) => ({ ...current, rationale: event.target.value }))}
          />
        </Field>
        <Field label="Rollback hint">
          <Input
            placeholder="Optional override for rollback guidance"
            value={draft.rollbackHint}
            onChange={(event) => setDraft((current) => ({ ...current, rollbackHint: event.target.value }))}
          />
        </Field>
        <div className="flex flex-wrap gap-3">
          <Button onClick={() => void handleCreate()}>Create Approval Request</Button>
          <Button variant="outline" onClick={() => void refreshRequests()}>
            Refresh
          </Button>
        </div>
        {statusMessage ? <p className="text-sm text-muted-foreground">{statusMessage}</p> : null}
      </SectionCard>

      <SectionCard
        eyebrow="Pending Actions"
        title="Review And Execute"
        description="Approvals move through an explicit state machine: pending, approved, then executed. Risk, rollback hints, and execution output stay visible."
      >
        <div className="space-y-4">
          {requests.length === 0 ? (
            <p className="text-sm text-muted-foreground">No approval requests yet.</p>
          ) : (
            requests.map((request) => (
              <article className="rounded-xl border bg-background/80 p-4" key={request.id}>
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="space-y-1">
                    <p className="text-sm font-semibold">
                      {request.actionType} on {request.targetRef}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {request.environmentId} · {request.createdAt}
                    </p>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <Badge variant={riskVariant(request.riskLevel)}>{request.riskLevel}</Badge>
                    <Badge variant={statusVariant(request.status)}>{request.status}</Badge>
                  </div>
                </div>
                <p className="mt-3 text-sm leading-6 text-muted-foreground">{request.rationale}</p>
                <p className="mt-2 text-xs leading-5 text-muted-foreground">
                  Rollback: {request.rollbackHint}
                </p>
                {request.executionSummary ? (
                  <p className="mt-2 text-xs leading-5 text-muted-foreground">
                    Result: {request.executionSummary}
                  </p>
                ) : null}
                <div className="mt-4 flex flex-wrap gap-3">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => void handleApprove(request.id)}
                    disabled={request.status !== "pending"}
                  >
                    Approve
                  </Button>
                  <Button
                    size="sm"
                    onClick={() => void handleExecute(request.id)}
                    disabled={request.status !== "approved"}
                  >
                    Execute
                  </Button>
                </div>
              </article>
            ))
          )}
        </div>
      </SectionCard>
    </div>
  );
}

function buildTargetPayload(draft: {
  actionType: ApprovalActionType;
  namespace: string;
  podName: string;
  deploymentName: string;
  replicas: string;
  host: string;
}) {
  switch (draft.actionType) {
    case "restart_pod":
      return {
        targetRef: `${draft.namespace}/${draft.podName.trim()}`,
        details: {
          namespace: draft.namespace.trim(),
          podName: draft.podName.trim(),
        },
      };
    case "scale_deployment":
      return {
        targetRef: `${draft.namespace}/${draft.deploymentName.trim()}`,
        details: {
          namespace: draft.namespace.trim(),
          deploymentName: draft.deploymentName.trim(),
          replicas: Number.parseInt(draft.replicas, 10),
        },
      };
    case "reload_nginx":
      return {
        targetRef: draft.host.trim(),
        details: {
          host: draft.host.trim(),
        },
      };
  }
}

function riskVariant(riskLevel: string): "success" | "warning" | "danger" | "outline" {
  if (riskLevel === "critical" || riskLevel === "high") {
    return "danger";
  }
  if (riskLevel === "medium") {
    return "warning";
  }
  return "outline";
}

function statusVariant(status: string): "success" | "warning" | "danger" | "outline" {
  if (status === "executed") {
    return "success";
  }
  if (status === "approved") {
    return "warning";
  }
  return "outline";
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="space-y-2">
      <span className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
        {label}
      </span>
      {children}
    </label>
  );
}
