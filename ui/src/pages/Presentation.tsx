import { createSignal, Switch, Match } from "solid-js";
import PresentationLibrary from "./presentation/PresentationLibrary";

type View = "library" | "studio" | "workspace";

export default function PresentationPage() {
  const [view, setView] = createSignal<View>("library");
  const [activeDeckId, setActiveDeckId] = createSignal<string | null>(null);

  return (
    <div class="h-full w-full">
      <Switch>
        <Match when={view() === "library"}>
          <PresentationLibrary
            onOpenDeck={(id) => {
              setActiveDeckId(id);
              setView("workspace");
            }}
            onNewDeck={() => setView("studio")}
          />
        </Match>
        <Match when={view() === "studio"}>
          <div class="flex items-center justify-center h-full text-gray-500 bg-[#0f0f14]">
            <div class="text-center">
              <p class="text-lg font-medium text-white mb-2">Creation Studio</p>
              <p class="text-sm">Coming in Frontend sub-plan</p>
              <button
                onClick={() => setView("library")}
                class="mt-4 px-4 py-2 bg-[#1c1c24] rounded-lg text-sm"
              >
                &#8592; Back
              </button>
            </div>
          </div>
        </Match>
        <Match when={view() === "workspace"}>
          <div class="flex items-center justify-center h-full text-gray-500 bg-[#0f0f14]">
            <div class="text-center">
              <p class="text-lg font-medium text-white mb-2">Deck: {activeDeckId()}</p>
              <p class="text-sm">Workspace coming in Frontend sub-plan</p>
              <button
                onClick={() => setView("library")}
                class="mt-4 px-4 py-2 bg-[#1c1c24] rounded-lg text-sm"
              >
                &#8592; Back to Library
              </button>
            </div>
          </div>
        </Match>
      </Switch>
    </div>
  );
}
