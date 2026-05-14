import { createSignal, Switch, Match, Show } from "solid-js";
import PresentationLibrary from "./presentation/PresentationLibrary";
import CreationStudio from "./presentation/CreationStudio";
import DeckWorkspace from "./presentation/DeckWorkspace";

type View = "library" | "studio" | "workspace";

export default function PresentationPage() {
  const [view, setView] = createSignal<View>("library");
  const [activeDeckId, setActiveDeckId] = createSignal<string | null>(null);
  const [initialSessionId, setInitialSessionId] = createSignal<string | undefined>(undefined);

  return (
    <div class="h-full w-full">
      <Switch>
        <Match when={view() === "library"}>
          <PresentationLibrary
            onOpenDeck={(id) => { setActiveDeckId(id); setInitialSessionId(undefined); setView("workspace"); }}
            onNewDeck={() => setView("studio")} />
        </Match>
        <Match when={view() === "studio"}>
          <CreationStudio
            onBack={() => setView("library")}
            onGenerated={(sid) => { setInitialSessionId(sid); setActiveDeckId(null); setView("workspace"); }} />
        </Match>
        <Match when={view() === "workspace"}>
          <Show when={activeDeckId() || initialSessionId()}
            fallback={
              <div class="flex items-center justify-center h-full bg-[#0f0f14]">
                <button onClick={() => setView("library")} class="px-4 py-2 bg-[#1c1c24] rounded-lg text-sm text-white">
                  &#8592; Back to Library
                </button>
              </div>
            }>
            <DeckWorkspace
              deckId={activeDeckId() ?? ""}
              onBack={() => setView("library")}
              initialSessionId={initialSessionId()} />
          </Show>
        </Match>
      </Switch>
    </div>
  );
}
