# ADBCHelper 10-Week MVP Roadmap

This roadmap is the current implementation target for the desktop app. It is written as a reusable project document so you can carry the same delivery plan into other workspaces.

## Goal

By the end of week 10, the app should be able to:

1. connect to environments
2. inspect Kubernetes resources
3. search logs from ELK and servers
4. compare Nacos configs
5. use an LLM to summarize and troubleshoot
6. save investigations and generate reports
7. support a small approval-based action flow

## Week 1: Project Foundation

Build:

1. create `Tauri + React + TypeScript` project
2. set up routing, layout, shared types, and basic state management
3. create Rust command bridge between frontend and backend
4. add SQLite integration
5. define core app models:
   - environment
   - connection profile
   - chat session
   - investigation
6. add app settings page shell

Deliverable:

- desktop app opens
- frontend can call Rust backend
- database initializes on first run

## Week 2: Environment and Credential Management

Build:

1. environment profile management for `dev`, `test`, and `prod`
2. connection profile forms for Kubernetes, ELK, SSH, and Nacos
3. integrate OS keychain for secrets
4. implement backend validation for stored profiles
5. add environment switcher in UI

Deliverable:

- user can create and test environment connections
- secrets are not stored in plain text in SQLite

## Week 3: Chat Framework and LLM Orchestrator

Build:

1. chat page UI
2. chat session persistence
3. Qwen client abstraction
4. tool registry and tool schema system
5. orchestrator loop:
   - receive question
   - select tools
   - execute tool
   - summarize result
6. audit logging for tool calls

Deliverable:

- user can ask a question and receive a model answer
- tool calls are logged and visible

## Week 4: Kubernetes Read-Only Integration

Build:

1. adapter for:
   - list namespaces
   - list pods
   - pod details
   - events
   - logs
2. resources page for Kubernetes
3. pod detail drawer or page
4. AI troubleshooting prompts for pod failures
5. environment-scoped access checks

Deliverable:

- user can inspect workloads and ask questions about pod restarts and health

## Week 5: ELK / Log Analysis

Build:

1. ELK query adapter
2. logs page with filters:
   - service
   - pod
   - keyword
   - traceId
   - time range
3. error clustering and aggregation logic
4. LLM log summarization workflow
5. attach log evidence into chat and investigation flow

Deliverable:

- user can search logs and get AI summaries with likely causes

## Week 6: SSH Server Diagnostics

Build:

1. SSH adapter with saved host profiles
2. command whitelist for read-only diagnostics
3. server logs viewer
4. host health summary for CPU, memory, disk, and process or port checks
5. AI server diagnosis summary

Deliverable:

- app can inspect app servers and Nginx hosts safely

## Week 7: Nacos Config Diff

Build:

1. Nacos connection adapter
2. query configs by `dataId` and `group`
3. environment-to-environment config diff
4. diff viewer UI
5. AI explanation of config differences and likely impact

Deliverable:

- user can compare `test` versus `prod` configs and ask which differences may be risky

## Week 8: Investigations and Report Generation

Build:

1. save investigation from chat
2. evidence collection model
3. timeline generation from:
   - logs
   - pod events
   - config differences
4. report generator in Markdown and HTML
5. investigations list and detail page

Deliverable:

- one-click save from AI session into an incident-style report

## Week 9: Approval Flow and Limited Actions

Build:

1. approval center UI
2. risk classification model
3. pending approval records
4. limited write actions:
   - restart pod
   - scale deployment
   - reload nginx
5. confirm dialog with:
   - target
   - action
   - environment
   - rollback hint
6. audit trail for approvals and executions

Deliverable:

- app supports controlled operations with explicit confirmation

## Week 10: Hardening, Testing, and Release Candidate

Build:

1. improve error handling and retries
2. add timeout and cancellation support
3. add secret masking everywhere
4. add prompt injection protections for logs
5. add role and environment restrictions
6. test production safety behavior
7. package app for target OS
8. write minimal admin and user documentation

Deliverable:

- internal MVP release candidate

## Parallel Workstreams

1. UX cleanup for evidence panels, loading states, and history navigation
2. prompt tuning for better summaries, lower hallucination risk, and explicit uncertainty
3. runbook seeding for pod restarts, Nginx 502s, Redis timeouts, and Nacos drift

## Deferred Until After MVP

1. multi-user backend
2. real-time collaboration
3. autonomous remediation
4. generic shell agent
5. full Kubernetes exec support
6. complex workflow builders
7. plugin marketplace

## Success Criteria

The MVP is successful when it can:

1. explain why a service is failing
2. inspect pods and events
3. search ELK logs and summarize top errors
4. inspect server logs over SSH
5. compare Nacos configs across environments
6. save findings as an investigation report
7. approve a small set of safe actions
