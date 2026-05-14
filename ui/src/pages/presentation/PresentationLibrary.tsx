import { createSignal, onMount, For, Show } from "solid-js";
import { listPresentations } from "../../lib/presentation-api";
import type { DeckSummary } from "../../lib/deck-schema";

interface Props {
  onOpenDeck: (id: string) => void;
  onNewDeck: () => void;
}

export default function PresentationLibrary(props: Props) {
  const [decks, setDecks] = createSignal<DeckSummary[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  onMount(async () => {
    try {
      const list = await listPresentations();
      setDecks(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  });

  return (
    <div class="flex flex-col h-full bg-[#0f0f14] text-white p-8">
      <div class="flex items-center justify-between mb-8">
        <div>
          <h1 class="text-3xl font-bold tracking-tight">Presentations</h1>
          <p class="text-gray-400 mt-1 text-sm">AI-generated cinematic decks</p>
        </div>
        <button
          onClick={props.onNewDeck}
          class="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 rounded-lg text-sm font-medium transition-colors"
        >
          + New Presentation
        </button>
      </div>

      <Show when={loading()}>
        <div class="flex items-center justify-center flex-1 text-gray-500">Loading...</div>
      </Show>

      <Show when={error()}>
        <div class="text-red-400 text-sm">{error()}</div>
      </Show>

      <Show when={!loading() && decks().length === 0}>
        <div class="flex flex-col items-center justify-center flex-1 gap-4 text-center">
          <div class="text-6xl opacity-30">&#9654;</div>
          <h2 class="text-xl font-medium text-gray-300">No presentations yet</h2>
          <p class="text-gray-500 text-sm max-w-sm">
            Paste your notes, upload a document, or drop in a URL and let AI build your deck.
          </p>
          <button
            onClick={props.onNewDeck}
            class="px-6 py-3 bg-indigo-600 hover:bg-indigo-500 rounded-xl text-sm font-medium transition-colors mt-2"
          >
            Create your first presentation
          </button>
        </div>
      </Show>

      <Show when={!loading() && decks().length > 0}>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          <For each={decks()}>
            {(deck) => (
              <button
                onClick={() => props.onOpenDeck(deck.id)}
                class="group text-left bg-[#1c1c24] hover:bg-[#252530] border border-[#2a2a36] hover:border-indigo-500/40 rounded-xl overflow-hidden transition-all"
              >
                <div class="aspect-video bg-[#0f0f14] flex items-center justify-center border-b border-[#2a2a36]">
                  <Show
                    when={deck.thumbnail_data_url}
                    fallback={<span class="text-4xl opacity-20">&#9654;</span>}
                  >
                    <img
                      src={deck.thumbnail_data_url!}
                      alt={deck.title}
                      class="w-full h-full object-cover"
                    />
                  </Show>
                </div>
                <div class="p-3">
                  <p class="font-medium text-sm truncate">{deck.title}</p>
                  <p class="text-xs text-gray-500 mt-0.5">
                    {deck.slide_count} slides &middot; {new Date(deck.updated_at).toLocaleDateString()}
                  </p>
                </div>
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
