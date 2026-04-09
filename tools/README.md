Place local helper binaries here when packaging the desktop app.

Windows Kubernetes packaging:
- drop `kubectl.exe` into this folder
- the app will prefer a custom `kubectl path` from Settings first
- if no custom path is set, it will look for bundled `tools/kubectl.exe`
- if that is also missing, it falls back to `kubectl` from the system PATH

This folder is bundled into packaged builds through `src-tauri/tauri.conf.json`.
The actual executable is ignored by git through `.gitignore`.
