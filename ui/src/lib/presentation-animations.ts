import type { AnimPhase, TransitionKind, Direction } from "./deck-schema";

export const KEYFRAMES_CSS = `
@keyframes fade-in        { from{opacity:0} to{opacity:1} }
@keyframes fade-exit      { from{opacity:1} to{opacity:0} }
@keyframes slide-in-left  { from{opacity:0;transform:translateX(-40px)} to{opacity:1;transform:translateX(0)} }
@keyframes slide-in-right { from{opacity:0;transform:translateX(40px)}  to{opacity:1;transform:translateX(0)} }
@keyframes slide-in-up    { from{opacity:0;transform:translateY(40px)}  to{opacity:1;transform:translateY(0)} }
@keyframes slide-in-down  { from{opacity:0;transform:translateY(-40px)} to{opacity:1;transform:translateY(0)} }
@keyframes zoom-in-anim   { from{opacity:0;transform:scale(0.8)}   to{opacity:1;transform:scale(1)} }
@keyframes scale-up-anim  { from{opacity:0;transform:scale(0.6)}   to{opacity:1;transform:scale(1)} }
@keyframes spring-in-anim { from{opacity:0;transform:scale(0.75)}  to{opacity:1;transform:scale(1)} }
@keyframes blur-reveal-anim { from{opacity:0;filter:blur(8px)} to{opacity:1;filter:blur(0)} }
@keyframes zoom-exit  { from{opacity:1;transform:scale(1)}   to{opacity:0;transform:scale(1.15)} }
@keyframes zoom-enter { from{opacity:0;transform:scale(0.9)} to{opacity:1;transform:scale(1)} }
@keyframes slide-exit-left   { from{opacity:1;transform:translateX(0)}   to{opacity:0;transform:translateX(-60px)} }
@keyframes slide-exit-right  { from{opacity:1;transform:translateX(0)}   to{opacity:0;transform:translateX(60px)} }
@keyframes slide-enter-left  { from{opacity:0;transform:translateX(60px)}  to{opacity:1;transform:translateX(0)} }
@keyframes slide-enter-right { from{opacity:0;transform:translateX(-60px)} to{opacity:1;transform:translateX(0)} }
`;

const STYLE_TAG_ID = "minion-anim-keyframes";

export function injectKeyframes(): void {
  if (document.getElementById(STYLE_TAG_ID)) return;
  const style = document.createElement("style");
  style.id = STYLE_TAG_ID;
  style.textContent = KEYFRAMES_CSS;
  document.head.appendChild(style);
}

export function animationStyle(phase: AnimPhase | undefined): string {
  if (!phase) return "";
  const { delay_ms: d, duration_ms: dur, effect } = phase;
  const std    = "cubic-bezier(0.4,0,0.2,1)";
  const spring = "cubic-bezier(0.34,1.56,0.64,1)";
  switch (effect.kind) {
    case "fade":        return `fade-in ${dur}ms ${std} ${d}ms both`;
    case "slide_in": {
      const m: Record<string,string> = {
        left:"slide-in-left", right:"slide-in-right", up:"slide-in-up", down:"slide-in-down"
      };
      return `${m[(effect as {kind:"slide_in";direction:string}).direction] ?? "slide-in-left"} ${dur}ms ${std} ${d}ms both`;
    }
    case "zoom_in":     return `zoom-in-anim ${dur}ms ${std} ${d}ms both`;
    case "scale_up":    return `scale-up-anim ${dur}ms ${std} ${d}ms both`;
    case "blur_reveal": return `blur-reveal-anim ${dur}ms ${std} ${d}ms both`;
    case "spring":      return `spring-in-anim ${dur}ms ${spring} ${d}ms both`;
    default:            return `fade-in ${dur}ms ${std} ${d}ms both`;
  }
}

export interface TransitionStyles { exiting: string; entering: string }

export function transitionStyles(
  kind: TransitionKind, direction: Direction | undefined, duration_ms: number
): TransitionStyles {
  const dur  = duration_ms > 0 ? duration_ms : 300;
  const ease = "cubic-bezier(0.4,0,0.2,1)";
  switch (kind) {
    case "fade":
      return { exiting: `fade-exit ${dur}ms ${ease} both`, entering: `fade-in ${dur}ms ${ease} both` };
    case "push": {
      const ex = direction === "right" ? "slide-exit-right" : "slide-exit-left";
      const en = direction === "right" ? "slide-enter-right" : "slide-enter-left";
      return { exiting: `${ex} ${dur}ms ${ease} both`, entering: `${en} ${dur}ms ${ease} both` };
    }
    case "fly": {
      const en = direction === "right" ? "slide-enter-right" : "slide-enter-left";
      return { exiting: `fade-exit ${dur}ms ${ease} both`, entering: `${en} ${dur}ms ${ease} both` };
    }
    default:
      return { exiting: `zoom-exit ${dur}ms ${ease} both`, entering: `zoom-enter ${dur}ms ${ease} both` };
  }
}
