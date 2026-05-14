// ui/src/store/deck-store.ts
import { createStore } from "solid-js/store";
import type { Deck } from "../lib/deck-schema";
import type { DeckPatch } from "../lib/deck-patch";
import { applyPatch as applyDeckPatch } from "../lib/deck-patch";

const MAX_HISTORY = 50;

export interface DeckStoreState { deck: Deck | null }

export interface DeckStoreActions {
  setDeck: (deck: Deck | null) => void;
  applyPatch: (patch: DeckPatch) => void;
  undo: () => void;
  redo: () => void;
  canUndo: () => boolean;
  canRedo: () => boolean;
}

export function createDeckStore(): [DeckStoreState, DeckStoreActions] {
  const [store, setStore] = createStore<DeckStoreState>({ deck: null });
  let undoStack: Deck[] = [];
  let redoStack: Deck[] = [];

  const actions: DeckStoreActions = {
    setDeck(deck) {
      undoStack = [];
      redoStack = [];
      setStore("deck", deck);
    },
    applyPatch(patch) {
      const current = store.deck;
      if (!current) return;
      undoStack = [...undoStack.slice(-(MAX_HISTORY - 1)), current];
      redoStack = [];
      setStore("deck", applyDeckPatch(current, patch));
    },
    undo() {
      const prev = undoStack.at(-1);
      if (!prev) return;
      if (store.deck) redoStack = [...redoStack, store.deck];
      undoStack = undoStack.slice(0, -1);
      setStore("deck", prev);
    },
    redo() {
      const next = redoStack.at(-1);
      if (!next) return;
      if (store.deck) undoStack = [...undoStack, store.deck];
      redoStack = redoStack.slice(0, -1);
      setStore("deck", next);
    },
    canUndo: () => undoStack.length > 0,
    canRedo: () => redoStack.length > 0,
  };

  return [store, actions];
}
