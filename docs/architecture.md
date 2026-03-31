# ADBCHelper Architecture

This repository now contains the Week 1 foundation for the 10-week desktop ops copilot roadmap:

- `React + TypeScript` frontend for the desktop shell and core pages
- `Tauri + Rust` backend for local runtime commands and future privileged tool adapters
- `SQLite` bootstrap for local application state
- seed environment profiles that mirror the intended Kubernetes, ELK, SSH, Nacos, and Redis integrations

Next implementation slices:

1. environment connection management and secrets
2. Qwen-backed chat orchestration
3. Kubernetes read-only adapters
4. ELK, SSH, and Nacos integrations
5. investigation timelines, report export, and approvals
