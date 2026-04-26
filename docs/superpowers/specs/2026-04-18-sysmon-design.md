# System Monitor — Design Spec
_Date: 2026-04-18_

## Overview

A real-time system health monitoring module for MINION. Collects CPU, RAM, disk, GPU, and process metrics every 5 seconds, stores 30 days of history in SQLite, surfaces alerts when configurable thresholds are crossed, and optionally calls an LLM to correlate events and produce RCA. LLM features degrade silently when no endpoint is available.

---

## Architecture

### New files

| File | Responsibility |
|---|---|
| `src-tauri/src/sysmon_collect.rs` | Metric collection via `sysinfo` + optional `nvml-wrapper` |
| `src-tauri/src/sysmon_commands.rs` | All `#[tauri::command]` functions |
| `src-tauri/src/sysmon_analysis.rs` | LLM prompt builder, RCA trigger logic, correlation |
| `ui/src/pages/Sysmon.tsx` | Main page — Timeline-First layout, light theme |
| `ui/src/pages/sysmon/ProcessTab.tsx` | Sortable process table, zombies highlighted red |
| `ui/src/pages/sysmon/DiskTab.tsx` | Per-mount usage bars + top-directory breakdown |
| `ui/src/pages/sysmon/AnalysisTab.tsx` | LLM analysis history + Deep Dive panel |
| `ui/src/pages/sysmon/SettingsTab.tsx` | Per-metric threshold configuration |

### Modified files

| File | Change |
|---|---|
| `crates/minion-db/src/migrations.rs` | Migration 015 — 4 new tables |
| `src-tauri/Cargo.toml` | Add `sysinfo`, `nvml-wrapper` (optional feature) |
| `src-tauri/src/lib.rs` | Declare module, spawn poller, register commands |

---

## Background Worker

`spawn_sysmon_poller(state, db, app_handle)` — spawned in `lib.rs` setup alongside `spawn_scheduled_publisher`.

**5-second tick loop:**
1. Call `sysmon_collect::snapshot()` → `SystemSnapshot`
2. Insert row into `sysmon_snapshots`
3. Every 30 s: snapshot top-20 processes into `sysmon_processes`
4. Check each metric against stored thresholds
5. If threshold crossed: insert `sysmon_alerts` row, emit `sysmon-alert` Tauri event
6. If alert fired and last auto-analysis was >2 min ago: queue async LLM call (debounced)
7. Emit `sysmon-snapshot` Tauri event with latest `SystemSnapshot`
8. Every midnight: prune `sysmon_snapshots` and `sysmon_processes` older than 30 days

---

## Data Model (Migration 015)

### `sysmon_snapshots`
```sql
id           TEXT PRIMARY KEY
sampled_at   TEXT NOT NULL           -- ISO-8601 UTC
cpu_pct      REAL NOT NULL
ram_used_mb  INTEGER NOT NULL
ram_total_mb INTEGER NOT NULL
swap_used_mb INTEGER NOT NULL
load_avg_1   REAL                    -- NULL on Windows
disks_json   TEXT NOT NULL           -- [{mount, used_gb, total_gb, read_bps, write_bps}]
gpus_json    TEXT NOT NULL           -- [{name, util_pct, vram_used_mb, vram_total_mb, temp_c}]
net_json     TEXT NOT NULL           -- [{iface, rx_bps, tx_bps}]
```
~200 bytes/row × 17,280 rows/day ≈ 3.5 MB/day → ~100 MB/30 days.

### `sysmon_processes`
```sql
id          TEXT PRIMARY KEY
sampled_at  TEXT NOT NULL
pid         INTEGER NOT NULL
name        TEXT NOT NULL
cpu_pct     REAL NOT NULL
ram_mb      INTEGER NOT NULL
status      TEXT NOT NULL           -- 'running' | 'sleeping' | 'zombie' | 'stopped'
user_name   TEXT
```
Pruned to last 200 samples (~top-20 × 200 = 4,000 rows max).

### `sysmon_alerts`
```sql
id           TEXT PRIMARY KEY
fired_at     TEXT NOT NULL
metric       TEXT NOT NULL          -- 'cpu' | 'ram' | 'disk' | 'gpu' | 'zombie'
value        REAL NOT NULL
threshold    REAL NOT NULL
severity     TEXT NOT NULL          -- 'warn' | 'critical'
detail       TEXT                   -- mount path, process name, pid, etc.
resolved_at  TEXT                   -- NULL = still active
```

### `sysmon_analyses`
```sql
id            TEXT PRIMARY KEY
created_at    TEXT NOT NULL
trigger       TEXT NOT NULL          -- 'auto' | 'manual'
alert_id      TEXT REFERENCES sysmon_alerts(id) ON DELETE SET NULL
question      TEXT                   -- user question for manual deep dives
context_json  TEXT NOT NULL          -- snapshot window sent to LLM
response      TEXT NOT NULL          -- LLM markdown response
```

---

## Tauri Commands

| Command | Description |
|---|---|
| `sysmon_get_current` | Latest snapshot + current process list |
| `sysmon_get_history(metric, hours)` | Time-series array for sparklines (max 720 h) |
| `sysmon_list_alerts(limit)` | Recent alerts, newest first |
| `sysmon_resolve_alert(id)` | Manually mark alert resolved |
| `sysmon_list_processes(sample_id?)` | Process snapshot (latest or specific) |
| `sysmon_kill_process(pid)` | SIGKILL a zombie or runaway process |
| `sysmon_get_disk_breakdown(path)` | Top-20 subdirs by size under `path` |
| `sysmon_run_analysis(question?)` | Manual deep-dive LLM call |
| `sysmon_list_analyses(limit)` | Cached LLM analysis history |
| `sysmon_get_settings` | Current threshold config |
| `sysmon_save_settings(settings)` | Persist threshold overrides |

---

## Tauri Events (frontend listeners)

| Event | Payload | Cadence |
|---|---|---|
| `sysmon-snapshot` | `SystemSnapshot` | Every 5 s |
| `sysmon-alert` | `{ id, metric, value, threshold, severity, detail }` | On breach |
| `sysmon-analysis-ready` | `{ id, trigger, response }` | When LLM completes |

---

## LLM Integration

### Graceful degradation
- If no LLM endpoint is configured → skip all LLM calls silently, show "No LLM endpoint configured" in the analysis panel.
- If LLM call fails (network error, timeout, non-2xx) → log `warn`, do not surface error to user, do not retry automatically.
- Manual "Deep Dive" button shows a spinner; on failure shows "Analysis unavailable — check LLM settings."

### Auto-analysis trigger
- Fires when an alert is inserted AND last `sysmon_analyses` row with `trigger = 'auto'` is >2 minutes old (debounce).
- Context window: all `sysmon_snapshots` from the past 5 minutes + all active alerts.

### Manual deep dive
- Context window: all `sysmon_snapshots` from the past 60 minutes + all alerts from same window + optional user question.

### Prompt structure
```
System: You are a system reliability expert. Analyse the metrics below and provide a concise RCA.
        Focus on correlations between CPU, RAM, disk I/O, and process events.
        Be specific: name the likely cause, its effect, and one actionable fix if warranted.
        If no issue is present, say so briefly.

User:   [metric summary table]
        [alert list]
        [question if manual]
```
Response stored as-is (markdown). Frontend renders it.

---

## Default Thresholds

| Metric | Warn | Critical |
|---|---|---|
| CPU % | 75 | 90 |
| RAM % | 80 | 92 |
| Disk % (any mount) | 80 | 90 |
| GPU utilisation % | 85 | 95 |
| Zombie processes | any | — |

All overridable per-metric in Settings.

---

## Frontend — Timeline-First Layout (Light Theme)

```
┌─────────────────────────────────────────────────────┐
│  CPU ▃▅▇▅▄  RAM ▅▆▇▇▆  Disk ▄▄▅▅▆  GPU ▁▂▂▃▄       │  ← sparkline header (60 min)
├─────────────────────────────────────────────────────┤
│  Event Timeline                                     │
│  ● 14:32  CPU spike 89% — cargo build               │
│  ● 14:31  Disk I/O burst — 480 MB/s                 │
│  ● 14:28  Zombie detected — pid 4821                │
├─────────────────────────────────────────────────────┤
│  🤖 LLM Insight                    [Deep Dive →]    │
│  cargo build → disk thrash → CPU spike → zombie     │
├──────────┬──────────┬──────────────┬────────────────┤
│ Processes│   Disk   │   Analyses   │   Settings     │  ← tabs
└──────────┴──────────┴──────────────┴────────────────┘
```

- Sparkline header updates live via `sysmon-snapshot` event (no polling)
- Timeline is a scrollable feed; new events prepended, alerts show severity colour
- LLM panel shows latest analysis; "Deep Dive" opens a modal with optional question input
- Process tab: sortable by CPU/RAM, zombies highlighted red with one-click kill
- Disk tab: per-mount horizontal bars + expandable top-dirs tree
- Analyses tab: full history of LLM responses
- Settings tab: threshold sliders per metric, save button

---

## Dependencies to Add

```toml
# src-tauri/Cargo.toml
sysinfo = "0.30"
nvml-wrapper = { version = "0.10", optional = true }

[features]
nvidia = ["nvml-wrapper"]
```

GPU collection falls back gracefully when `nvml-wrapper` init fails (no NVIDIA driver).

---

## Out of Scope

- Network topology / remote host monitoring
- Container / Docker stats
- Alerting via email or push notification (shown in-app only)
- ML-based anomaly detection (LLM RCA only)
