export interface ThemePreset {
  name: string;
  preview: { bg: string; accent: string; text: string };
}

export const THEMES: ThemePreset[] = [
  { name: "Dark Indigo",   preview: { bg: "#0f0f14", accent: "#6366f1", text: "#ffffff" } },
  { name: "Midnight Blue", preview: { bg: "#0a0a1a", accent: "#3b82f6", text: "#e0e0ff" } },
  { name: "Forest",        preview: { bg: "#0a1a0f", accent: "#22c55e", text: "#d0f0d0" } },
  { name: "Sunset",        preview: { bg: "#1a0a0a", accent: "#f97316", text: "#ffe0d0" } },
  { name: "Monochrome",    preview: { bg: "#111111", accent: "#e5e5e5", text: "#ffffff" } },
  { name: "Corporate",     preview: { bg: "#0f1a2a", accent: "#0ea5e9", text: "#e0f0ff" } },
];
