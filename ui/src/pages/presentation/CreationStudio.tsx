import { createSignal, Show, Switch, Match, For } from "solid-js";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { startGeneration, type InputSource } from "../../lib/presentation-api";
import type { GenerationConfig } from "../../lib/deck-schema";
import { THEMES } from "../../lib/themes";

type Tab = "text" | "files" | "url" | "git";
type Audience = "Engineering" | "Executive" | "Investor" | "General";
type Tone = "Authoritative" | "Conversational" | "Technical" | "Inspirational";

const AUDIENCES: Audience[] = ["Engineering", "Executive", "Investor", "General"];
const TONES: Tone[] = ["Authoritative", "Conversational", "Technical", "Inspirational"];
const LANGS = [
  { code: "en-US", label: "English (US)" }, { code: "fr-FR", label: "French" },
  { code: "de-DE", label: "German" },       { code: "es-ES", label: "Spanish" },
  { code: "ja-JP", label: "Japanese" },     { code: "zh-CN", label: "Chinese (Simplified)" },
  { code: "ar-SA", label: "Arabic" },
];

interface Props { onGenerated: (sessionId: string) => void; onBack: () => void }

export default function CreationStudio(props: Props) {
  const [tab, setTab] = createSignal<Tab>("text");
  const [text, setText] = createSignal("");
  const [paths, setPaths] = createSignal<string[]>([]);
  const [url, setUrl] = createSignal("");
  const [git, setGit] = createSignal("");
  const [audience, setAudience] = createSignal<Audience>("General");
  const [tone, setTone] = createSignal<Tone>("Conversational");
  const [lang, setLang] = createSignal("en-US");
  const [theme, setTheme] = createSignal<string>("Dark Indigo");
  const [busy, setBusy] = createSignal(false);
  const [err, setErr] = createSignal<string | null>(null);

  const hasInput = () => {
    switch (tab()) {
      case "text": return text().trim().length > 0;
      case "files": return paths().length > 0;
      case "url": return url().trim().length > 0;
      case "git": return git().trim().length > 0;
    }
  };

  const buildInputs = (): InputSource[] => {
    switch (tab()) {
      case "text": return [{ kind: "text", content: text() }];
      case "files": return paths().map(p => ({ kind: "file_path" as const, content: p }));
      case "url": return [{ kind: "url", content: url() }];
      case "git": return [{ kind: "git_url", content: git() }];
    }
  };

  const generate = async () => {
    if (!hasInput() || busy()) return;
    setErr(null); setBusy(true);
    try {
      const config: GenerationConfig = {
        audience: audience(), tone: tone(), language: lang(),
        theme_name: theme(),
        presentation_context: "live_talk",
      };
      props.onGenerated(await startGeneration(buildInputs(), config));
    } catch (e) { setErr(String(e)); } finally { setBusy(false); }
  };

  const tabCls = (t: Tab) =>
    `px-4 py-2 text-sm font-medium rounded-t-lg transition-colors ${tab() === t
      ? "bg-[#1c1c24] text-white border-b-2 border-indigo-500"
      : "text-gray-400 hover:text-white"}`;

  const chip = (on: boolean) =>
    `px-3 py-1 rounded-full text-xs font-medium border cursor-pointer transition-colors ${on
      ? "bg-indigo-600 border-indigo-500 text-white"
      : "bg-[#1c1c24] border-[#2a2a36] text-gray-400 hover:border-indigo-500/60"}`;

  return (
    <div class="flex flex-col h-full bg-[#0f0f14] text-white p-8 max-w-3xl mx-auto overflow-y-auto">
      <div class="flex items-center gap-4 mb-8">
        <button onClick={props.onBack} class="text-gray-400 hover:text-white text-sm">&#8592; Back</button>
        <h1 class="text-2xl font-bold">New Presentation</h1>
      </div>

      {/* Tabs */}
      <div class="flex gap-1 border-b border-[#2a2a36]">
        {(["text", "files", "url", "git"] as Tab[]).map(t =>
          <button class={tabCls(t)} onClick={() => setTab(t)}>
            {t[0].toUpperCase() + t.slice(1)}
          </button>
        )}
      </div>

      {/* Tab content */}
      <div class="bg-[#1c1c24] rounded-b-xl rounded-tr-xl p-4 mb-6 border border-[#2a2a36] border-t-0">
        <Switch>
          <Match when={tab() === "text"}>
            <textarea
              class="w-full bg-transparent text-sm text-gray-200 placeholder-gray-600 resize-none outline-none"
              rows={12} placeholder="Paste notes, outline, or raw text…"
              value={text()} onInput={e => setText(e.currentTarget.value)}
            />
          </Match>
          <Match when={tab() === "files"}>
            <button
              class="px-4 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium"
              onClick={async () => {
                const selected = await openFileDialog({
                  multiple: true,
                  filters: [{ name: "Supported files", extensions: ["pdf","docx","md","xlsx","png","jpg","jpeg"] }],
                });
                if (selected) {
                  const files = Array.isArray(selected) ? selected : [selected];
                  setPaths(files);
                }
              }}
            >
              Choose Files…
            </button>
            <Show when={paths().length > 0}>
              <ul class="mt-2 text-xs text-gray-400 list-disc list-inside space-y-0.5">
                <For each={paths()}>{p => <li class="truncate">{p}</li>}</For>
              </ul>
            </Show>
          </Match>
          <Match when={tab() === "url"}>
            <input type="url"
              class="w-full bg-[#0f0f14] border border-[#2a2a36] rounded-lg px-3 py-2 text-sm text-gray-200 outline-none focus:border-indigo-500"
              placeholder="https://example.com/report" value={url()}
              onInput={e => setUrl(e.currentTarget.value)}
            />
            <p class="mt-2 text-xs text-amber-500/80">&#9888; Only fetch URLs you own or trust. Minion fetches server-side.</p>
          </Match>
          <Match when={tab() === "git"}>
            <input type="text"
              class="w-full bg-[#0f0f14] border border-[#2a2a36] rounded-lg px-3 py-2 text-sm text-gray-200 outline-none focus:border-indigo-500"
              placeholder="https://github.com/org/repo" value={git()}
              onInput={e => setGit(e.currentTarget.value)}
            />
          </Match>
        </Switch>
      </div>

      {/* Audience */}
      <div class="mb-4">
        <p class="text-xs text-gray-500 uppercase tracking-wider mb-2">Audience</p>
        <div class="flex gap-2 flex-wrap">
          <For each={AUDIENCES}>{a =>
            <button class={chip(audience() === a)} onClick={() => setAudience(a)}>{a}</button>
          }</For>
        </div>
      </div>

      {/* Tone */}
      <div class="mb-4">
        <p class="text-xs text-gray-500 uppercase tracking-wider mb-2">Tone</p>
        <div class="flex gap-2 flex-wrap">
          <For each={TONES}>{t =>
            <button class={chip(tone() === t)} onClick={() => setTone(t)}>{t}</button>
          }</For>
        </div>
      </div>

      {/* Language */}
      <div class="mb-6">
        <p class="text-xs text-gray-500 uppercase tracking-wider mb-2">Language</p>
        <select
          class="bg-[#1c1c24] border border-[#2a2a36] rounded-lg px-3 py-2 text-sm text-gray-200 outline-none"
          value={lang()} onChange={e => setLang(e.currentTarget.value)}>
          <For each={LANGS}>{l => <option value={l.code}>{l.label}</option>}</For>
        </select>
      </div>

      {/* Theme */}
      <div class="mb-6">
        <p class="text-xs text-gray-500 uppercase tracking-wider mb-3">Theme</p>
        <div class="grid grid-cols-3 gap-3">
          <For each={THEMES}>{(t) => (
            <button
              onClick={() => setTheme(t.name)}
              class={`flex flex-col items-center gap-1.5 rounded-lg p-1 border-2 transition-colors ${
                theme() === t.name
                  ? "border-indigo-500"
                  : "border-transparent hover:border-[#3a3a48]"
              }`}
            >
              <div
                class="w-20 h-[50px] rounded-md flex-shrink-0 relative overflow-hidden"
                style={{ "background-color": t.preview.bg }}
              >
                <div class="absolute bottom-0 left-0 right-0 h-[6px]"
                  style={{ "background-color": t.preview.accent }} />
                <div class="absolute top-2 left-2 flex flex-col gap-1">
                  <div class="h-1.5 w-10 rounded-full opacity-70"
                    style={{ "background-color": t.preview.text }} />
                  <div class="h-1 w-7 rounded-full opacity-40"
                    style={{ "background-color": t.preview.text }} />
                </div>
              </div>
              <span class="text-[10px] text-gray-400 leading-none text-center">{t.name}</span>
            </button>
          )}</For>
        </div>
      </div>

      <Show when={err()}><p class="text-red-400 text-sm mb-3">{err()}</p></Show>

      <button
        onClick={generate}
        disabled={!hasInput() || busy()}
        class="w-full py-3 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 disabled:cursor-not-allowed rounded-xl text-sm font-semibold transition-colors"
      >
        {busy() ? "Generating…" : "Generate Presentation"}
      </button>
    </div>
  );
}
