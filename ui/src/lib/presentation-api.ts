// ui/src/lib/presentation-api.ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DeckSummary, Deck, GenerationConfig } from "./deck-schema";
import type { DeckPatch } from "./deck-patch";

export async function listPresentations(): Promise<DeckSummary[]> {
  return invoke<DeckSummary[]>("list_presentations");
}

export async function getDeck(id: string): Promise<Deck> {
  return invoke<Deck>("get_deck", { id });
}

export async function saveDeckPatch(id: string, patches: DeckPatch[]): Promise<void> {
  return invoke<void>("save_deck_patch", { id, patches });
}

export interface InputSource {
  kind: "text" | "file_path" | "url" | "git_url";
  content: string;
}

export async function startGeneration(
  inputs: InputSource[],
  config: GenerationConfig,
): Promise<string> {
  return invoke<string>("start_presentation_generation", { inputs, config });
}

export async function interruptGeneration(
  sessionId: string,
  afterAgent: string,
  instruction: string,
): Promise<void> {
  return invoke<void>("interrupt_generation", {
    sessionId,
    afterAgent,
    instruction,
  });
}

export type ExportFormat = "pptx" | "pdf" | "html" | "speaker_notes_pdf";

export async function exportPresentation(
  id: string,
  format: ExportFormat,
  outputPath: string,
): Promise<{ file_size_bytes: number; path: string }> {
  return invoke("export_presentation", { id, format, outputPath });
}

export type AgentName =
  | "research" | "storyteller" | "slide_planner" | "visual" | "design_critic";

export type AgentEvent =
  | { seq: number; agent: AgentName; kind: "started" }
  | { seq: number; agent: AgentName; kind: "progress"; data: string }
  | { seq: number; agent: AgentName; kind: "slide_ready"; slide_index: number; patch: DeckPatch }
  | { seq: number; agent: AgentName; kind: "completed" }
  | { seq: number; agent: AgentName; kind: "error"; message: string; recoverable: boolean }
  | { seq: number; kind: "stream_complete"; deck_id: string }
  | { seq: number; kind: "stream_error"; message: string };

export function listenToAgentEvents(
  sessionId: string,
  onEvent: (e: AgentEvent) => void,
): Promise<UnlistenFn> {
  return listen<AgentEvent>(`presentation://agent-event/${sessionId}`, e => {
    onEvent(e.payload);
  });
}
