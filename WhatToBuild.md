# AI FILE MANAGER — SYSTEM SPECIFICATION

## 1. SYSTEM OVERVIEW

The system is an autonomous file management agent that:

- monitors filesystem changes
- indexes all files
- organizes files automatically using AI classification
- supports semantic file search via natural language
- maintains folder structure continuously

User interaction is optional; system runs autonomously by default.

## 2. CORE BEHAVIOR

### 2.1 Autonomous Mode

If enabled:

- continuously monitor selected directories
- automatically organize files into structured folders
- apply classification rules using AI inference
- update index in real-time

### 2.2 Manual Commands (via UI or API)

- trigger organization
- perform AI search
- exclude/include folders
- rollback changes

## 3. FEATURES (EXECUTION LOGIC)

### 3.1 Auto Organization Engine

Trigger:

- right-click OR API call OR scheduled scan

Behavior:

- scan target directory
- classify each file using AI model
- assign category labels
- move file into structured path: `/Category/Subcategory/filename`

Rules:

- avoid overwriting existing files
- detect duplicates before moving
- preserve metadata history of all moves

### 3.2 Continuous Organization Mode

If enabled for folder:

- file system watcher active
- on file create/modify:
  - classify file
  - move immediately if needed
  - update index

Exclude list overrides all operations.

### 3.3 Exclusion System

Folder metadata:

```json
{
  "excluded": true,
  "mode": "ignore | read_only | manual"
}
```

Behavior:

- ignore → no indexing or modification
- read_only → index only, no moves
- manual → only user-triggered actions allowed

### 3.4 AI Semantic Search

Input:

- natural language query

Process:

- convert query → embedding / semantic vector
- search Tantivy index + metadata DB
- rank results by similarity + metadata weight

Output:

- file path
- relevance score
- preview metadata

### 3.5 File Intelligence Engine (AI Layer)

Model input:

- file name
- file type
- extracted text (if applicable)
- folder context

Output:

```json
{
  "category": "",
  "sub_category": "",
  "tags": [],
  "confidence": 0.0
}
```

Supported backends:

- Ollama local LLM
- External API (OpenAI-compatible)

### 3.6 File Map Generator

On folder update:

- generate graph structure:
  - nodes = files/folders
  - edges = relationships (category similarity / history moves)

Store:

- `/.ai_maps/{folder_id}.json`

### 3.7 Action Logging System

Every operation logs:

```json
{
  "timestamp": "",
  "action": "move | delete | create | rename",
  "source": "",
  "destination": "",
  "reason": "AI classification",
  "model_confidence": 0.0
}
```

Logs are used for:

- rollback
- audit trail
- UI dashboard

## 4. SYSTEM ARCHITECTURE

```text
FILE SYSTEM
    ↓
WATCHER (Rust - notify)
    ↓
INDEXER (Tantivy + SQLite)
    ↓
AI CLASSIFIER (llama.cpp / API)
    ↓
RULE ENGINE (organization logic)
    ↓
FILE ACTION EXECUTOR
    ↓
UI LAYER (Tauri + React)
```

## 5. TECH STACK

### 5.1 Desktop Layer

- Tauri
- React
- TypeScript
- TailwindCSS

### 5.2 Core System Layer

- Rust (mandatory)
- Modules:
  - file watcher
  - executor engine
  - rule engine
  - metadata manager

### 5.3 AI Layer

- llama.cpp (local inference)
- optional external LLM API

### 5.4 Search Engine

- Tantivy (primary index)
- SQLite (metadata + logs)

## 6. PERFORMANCE REQUIREMENTS

- file operations must be async
- indexing must be incremental (no full rescans unless forced)
- AI calls must be batch optimized
- system must not block filesystem watcher

## 7. DESIGN CONSTRAINTS

- no blocking UI operations
- no synchronous file scanning on main thread
- all AI operations are fallback-safe
- all file moves must be reversible (transaction-like system)

## 8. OUTPUT CONTRACT FOR AI AGENT

Every AI decision must return:

```json
{
  "action": "move | ignore | tag | delete",
  "confidence": 0.0,
  "reason": "",
  "target_path": ""
}
```
