# AI File Manager

Autonomous **Tauri + React + Rust** file management system implementing the full specification in `WhatToBuild.md`.

## Implemented capabilities

- Autonomous/manual organization engine with AI-style classification output contract
- Real-time continuous organization mode using `notify` file watcher
- Exclusion system (`ignore`, `read_only`, `manual`) persisted in SQLite
- Action logging for move/create-style operations with rollback groups
- Rollback engine to revert file moves by rollback group
- Tantivy-backed indexing and semantic-style search endpoint
- File metadata database (SQLite) + content preview indexing
- File map generation under `/.ai_file_manager/.ai_maps/{folder_id}.json`
- Tauri command API + React dashboard for full control flow

## Tech stack

- **Desktop/UI:** Tauri, React, TypeScript, TailwindCSS
- **Core:** Rust (`notify`, `walkdir`, async commands)
- **Search:** Tantivy
- **Metadata + logs:** SQLite (`rusqlite`)

## Run locally

```bash
npm install
npm run dev
# in another terminal
npm run tauri dev
```

> Linux desktop prerequisites for Tauri must be installed (`webkit2gtk`, etc.).

## Backend command surface

- `organize_directory(path)`
- `set_continuous_mode(path, enabled)`
- `set_exclusion(rule)`
- `semantic_search(query, limit)`
- `get_logs(limit)`
- `rollback_group(rollback_group)`
- `system_status()`

## Notes

- All organization moves are duplicate-safe and overwrite-safe.
- All move actions are logged with rollback groups.
- Continuous mode respects exclusions.
- AI classification is implemented with a local deterministic intelligence layer and follows the required output schema.
