# AI Building-Blocks — a Primer for the MINION Roadmap

*A field guide to the concepts we keep hitting while designing the Health
Vault, Blog, and future modules. Short on hype, long on what actually
works in a desktop app that runs on one machine.*

---

## 1. RAG — Retrieval-Augmented Generation

### What it is

A classic LLM prompt has one input: the user's question. A RAG pipeline
has two: the question **plus** a handful of relevant chunks fetched from
a document corpus at query time. The LLM sees the chunks inline and
grounds its answer in them.

```
question ─▶ embed ─▶ vector search ─▶ top-K chunks ─▶ LLM prompt ─▶ answer
```

### Why it matters

- **Freshness**: no retraining needed when documents change.
- **Provenance**: the retrieved chunks can be surfaced as citations.
- **Size**: works on corpora too big to fit in a context window.

### Trade-offs

| Cost | Mitigation |
|------|------------|
| Retrieval quality bounds answer quality | Invest in good chunking + metadata filters before fancier rerankers |
| Chunks ≠ understanding | Keep chunk size sensible (300–800 tokens) and include neighbouring context |
| Hallucinations on empty retrieval | Instruct the model to say "I don't have that in the documents" |

### Concrete MINION integrations

- **Health Vault AI Analysis (shipped)**: the timeline-brief builder in
  `health_analysis.rs` is a curated summary, not retrieval — but the
  same patient corpus is the right substrate for RAG-style Q&A once we
  add document-level embeddings.
- **Reader**: semantic search across imported books/PDFs is a natural
  RAG fit; we already have `tantivy` for lexical search, so adding a
  local embedding model (e.g. `all-MiniLM-L6-v2` through `minion-llm`)
  covers the "semantic" half.
- **Blog**: writing assistant that grounds drafts in your own prior
  posts ("tone-match" or "avoid repetition").

### First-step MVP

Start with a single crate (`minion-rag`) that wraps:

1. Chunker (markdown-aware; don't split mid-code-fence)
2. Embedder (delegate to `minion-llm`; Ollama exposes `/api/embeddings`)
3. Vector store (SQLite + `sqlite-vec` or `libsql-vector`; an on-device
   brute-force cosine search over 10k vectors is <50ms — no need for
   FAISS yet)
4. Pipeline (`search(query) -> Vec<Chunk>`)

---

## 2. Agentic Workflows

### What it is

An "agent" is an LLM with a tool-call loop. Instead of returning
a text answer, it can ask the runtime to do things — read a file, query
a DB, call an API, ask another model. The runtime executes the tool,
hands the result back, and the model decides whether it's done.

```
[system] You have these tools: read_file, search_labs, send_email …
[user] What changed between my last two blood panels?
[assistant tool_call: search_labs(canonical_name="HbA1c")]
[tool result: [{date:…,value:…}, …]]
[assistant] "Your HbA1c dropped 1.2 points over six months…"
```

### Why it matters

LLMs stop being "predict the next word" and become **executable
workflows** — they can do multi-step reasoning grounded in your data.

### Key patterns

| Pattern | One-line definition | When to reach for it |
|---------|---------------------|----------------------|
| Single-turn tool use | Model calls one tool to answer | Lookup / Q&A |
| ReAct loop | Model reasons, acts, observes, repeats | Unknown steps ahead of time |
| Plan → Execute | Separate planner model sketches steps; executor runs them | Long, expensive workflows |
| Self-reflection | Model critiques its own output, retries | High-stakes outputs |
| Guarded tool use | Every tool call goes through a policy layer | Anything that writes or spends |

### Trade-offs

- **Cost explodes fast** — a 5-step ReAct loop is 5× the tokens of a
  direct answer.
- **Non-determinism** — the same input can take different paths.
- **Security** — a tool-using agent has more blast radius than a chat
  bot. Default to read-only tools, require human approval for writes.

### Concrete MINION integrations

- **Health Vault review queue**: instead of the user confirming each
  extracted lab test, an agent could auto-merge "clearly correct" rows
  and only surface ambiguous ones (tool: `save_lab_test`, `merge_entity`).
- **Blog publishing**: an agent that picks a canonical URL, generates
  platform-tailored summaries, and kicks off auto-publish calls — all
  gated on a final "approve" button.

### First-step MVP

- Define tools as typed functions with JSON Schema (`minion-llm` already
  lets you pass system prompts; add a `tools: Vec<ToolSpec>` field).
- Keep the loop inside **one** Rust async function; don't let the
  agent spawn processes or shell out.
- Log every tool call + result to `audit_log` — both for debugging and
  because users will want it.

---

## 3. Agentic AI vs. "Agentic Agents"

The terminology is messy. Here's how I'll use it in MINION docs from
now on:

| Term | Meaning |
|------|---------|
| **Agentic workflow** | A deterministic pipeline where the LLM calls tools in a defined structure (RAG counts) |
| **Agent** | One LLM + one tool-loop, scoped to a single goal |
| **Multi-agent system** | Two or more agents with distinct roles communicating (often through a shared memory or a router) |
| **Agentic AI** | Marketing term; usually means "agent" or "multi-agent system" |

Multi-agent systems are seductive but often overkill. Most real-world
tasks are a single agent with a good toolset. Reach for multi-agent
when:

- Roles are genuinely different (writer ≠ critic)
- Concurrency matters (research N sources in parallel)
- The tool catalog is too big for one system prompt

### Concrete MINION integrations (longer horizon)

- **Health analysis** could use a 2-agent setup: a **retriever** agent
  that builds a timeline brief from the DB, and a **reasoner** agent
  that produces the analysis. Today both live in the same function;
  splitting them would let us swap the reasoner (local → cloud) without
  touching retrieval.
- **Blog SEO**: writer agent drafts; critic agent scores against SEO
  rubric; a coordinator retries on low scores.

---

## 4. MCP — Model Context Protocol

### What it is

A standard protocol (JSON-RPC over stdio, SSE, or WebSocket) that lets
LLM clients discover and call tools hosted in a separate process. It
was introduced by Anthropic and is rapidly becoming the *de facto*
interop layer for agent tools.

```
Claude Code/LLM ◀──MCP──▶ mcp-filesystem     (tools: read_file, …)
                ◀──MCP──▶ mcp-github         (tools: list_issues, …)
                ◀──MCP──▶ mcp-minion         (tools: health_query_labs, …)
```

### Why it matters

- Tools written once work across LLM clients (Claude Code, Claude
  desktop, Cursor, Zed, generic CLI agents).
- Clean process separation — the LLM never touches your filesystem
  directly; it goes through an authored MCP server that enforces
  permissions.
- Discovery: clients ask the server "what tools do you expose?" at
  startup; no hand-written glue.

### Trade-offs

- Still young; the spec is evolving (as of early 2026 there are several
  authentication drafts).
- Per-server process overhead; not great for 50 tools that each need
  their own binary.

### Concrete MINION integrations

- **MCP server for MINION itself**: expose a carefully curated set of
  tools (`health.query_labs`, `blog.publish`, `reader.search`). External
  LLMs (Claude Code on your laptop, or your own model) could then reason
  over MINION data without MINION embedding an LLM.
- The reverse — MINION as an MCP **client** — is useful for plugging
  *other* tool servers into MINION's built-in AI features (e.g., a
  filesystem server to let the Health agent read a folder).

### First-step MVP

1. Ship a thin `minion-mcp` crate that implements the stdio transport
   and the `tools/list` + `tools/call` methods.
2. Start with three read-only tools: `list_patients`, `timeline_get`,
   `analyze`. Each is a pass-through to the existing Tauri command.
3. Document how to wire it into Claude Code via `~/.claude.json`.

---

## 5. n8n-style Workflow Automation

### What it is

n8n is a node-based workflow engine — draw a graph of "nodes" (triggers,
actions, conditions, loops) and the engine runs them when the trigger
fires. Think of it as Zapier with self-hosting and LLM-friendly nodes.

```
[Cron: daily 7am] → [Fetch new lab PDFs from email]
                  → [Ingest via MINION API]
                  → [Run analysis]
                  → [If anomaly: Slack message]
```

### Why it matters for MINION

Because MINION has a clean HTTP/Tauri surface, it slots into any n8n
workflow as a **data source** (emit events, read records) or a **sink**
(accept incoming documents). This is how "an app on my desk" becomes
"a service my other tools can orchestrate".

### Concrete MINION integrations

- **Blog Custom Webhook** platform (already in the v2 plan): any n8n
  workflow that ends in an HTTP node can receive our publish payloads —
  no platform-specific integration needed.
- **Health incoming**: an n8n flow that watches an IMAP folder for
  lab-PDF emails, downloads attachments, and POSTs them to a MINION
  ingestion webhook.
- **Export of analyses**: after a weekly AI analysis runs, n8n can
  format + email the summary to the patient.

### First-step MVP (no n8n integration code yet)

- Design the Blog v2 "Custom Webhook" platform so it takes a user URL
  and POSTs `{post: …, html: …, markdown: …}` on publish.
- Emit a small, stable event schema from the event bus (`minion-core`)
  so a future HTTP-bridge plugin can forward them to n8n.

---

## 6. How these fit together in MINION

```
┌───────────────────────────────────────────────────────────────┐
│ MINION desktop app                                            │
│                                                               │
│  ┌────────────┐    ┌───────────────┐    ┌────────────────┐   │
│  │ SolidJS UI │◀──▶│ Tauri command │◀──▶│ Rust modules    │   │
│  └────────────┘    └───────────────┘    └────────┬────────┘   │
│                                                   │           │
│            ┌──────────────────────────────────────┼────────┐  │
│            ▼                  ▼                   ▼        │  │
│      ┌──────────┐      ┌───────────┐      ┌────────────┐  │  │
│      │ minion-  │      │ minion-   │      │ minion-rag │  │  │
│      │ llm      │      │ crypto    │      │  (future)  │  │  │
│      └────┬─────┘      └───────────┘      └─────┬──────┘  │  │
│           │                                      │         │  │
│           ▼                                      ▼         │  │
│      [Ollama / OpenAI /              [SQLite + embeddings] │  │
│       Anthropic / Gemini]                                  │  │
│                                                            │  │
│      ┌────────────────────┐   ┌────────────────────┐      │  │
│      │ minion-mcp (future)│──▶│ Claude Code, etc.   │      │  │
│      └────────────────────┘   └────────────────────┘      │  │
└───────────────────────────────────────────────────────────────┘
                         ▲
                         │ HTTP / Webhook
                         │
                    ┌─────────┐
                    │  n8n    │  (external workflow automation)
                    └─────────┘
```

- **RAG** lives inside `minion-rag` and is consumed by Health /
  Reader / Blog.
- **Agents** live in feature crates (`health_analysis`, future
  `blog_agent`) and use `minion-llm` as the transport + RAG as the
  grounding layer.
- **MCP** turns MINION into a tool server others can orchestrate.
- **n8n** lives outside MINION and talks to it through HTTP + webhooks.

---

## 7. What we're *not* building (for now)

- **On-device fine-tuning / LoRA** — not worth the engineering cost for
  a single-user app. Prompt engineering + RAG covers 95% of cases.
- **Vector DB service** — libsql/sqlite-vec in-process is plenty up to
  10^5–10^6 vectors on modest hardware.
- **Multi-agent framework (AutoGen/LangGraph style)** — premature
  abstraction. Start with single-agent loops inside Rust async
  functions.
- **Real-time audio/video agents** — not aligned with MINION's "local
  files" center of gravity.

---

## 8. Glossary

- **Embedding** — a vector of floats (typically 384–1536 dims) that
  represents the meaning of a piece of text. Similar texts produce
  similar vectors.
- **Chunk** — a contiguous sub-string of a document, usually a few
  hundred tokens, treated as a single indexing unit for RAG.
- **Tool call** — a structured JSON output from an LLM that names a
  function and its arguments; the runtime executes the function.
- **Context window** — the maximum number of tokens an LLM can see at
  once (8k → 2M+ depending on model).
- **Grounding** — constraining an LLM's output by including authoritative
  source material in the prompt.

---

*Document owner: MINION roadmap. Update alongside each new AI feature
module. See `docs/BLOG_MODULE_V2.md` for the nearest concrete plan and
`docs/HEALTH_MODULE.md` for the shipped reference.*
