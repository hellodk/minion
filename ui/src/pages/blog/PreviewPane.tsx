import { Component, createEffect, onCleanup } from 'solid-js';
import DOMPurify, { type Config as DOMPurifyConfig } from 'dompurify';

interface PreviewPaneProps {
  html: string;
}

const DOMPURIFY_CONFIG: DOMPurifyConfig = {
  USE_PROFILES: { html: true },
  ADD_TAGS: [
    'svg', 'path', 'circle', 'rect', 'line', 'polyline', 'polygon',
    'text', 'g', 'defs', 'clipPath', 'use', 'image', 'foreignObject',
    'ellipse', 'tspan', 'marker', 'linearGradient', 'radialGradient', 'stop',
  ],
  ADD_ATTR: [
    'viewBox', 'xmlns', 'd', 'fill', 'stroke', 'stroke-width', 'stroke-linecap',
    'stroke-linejoin', 'transform', 'cx', 'cy', 'r', 'rx', 'ry',
    'x', 'y', 'x1', 'y1', 'x2', 'y2', 'width', 'height',
    'points', 'clip-path', 'marker-end', 'marker-start',
    'text-anchor', 'dominant-baseline', 'font-size', 'font-family',
  ],
  FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed', 'form'],
};

let mermaidInitialized = false;

async function applyMermaid(container: HTMLElement): Promise<void> {
  const blocks = container.querySelectorAll('code.language-mermaid');
  if (blocks.length === 0) return;
  try {
    const mermaid = await import('mermaid');
    if (!mermaidInitialized) {
      mermaid.default.initialize({ startOnLoad: false, theme: 'neutral', securityLevel: 'strict' });
      mermaidInitialized = true;
    }
    for (let i = 0; i < blocks.length; i++) {
      const el = blocks[i] as HTMLElement;
      const pre = el.closest('pre') ?? el;
      const graphDef = el.textContent ?? '';
      try {
        const id = `mermaid-${Date.now()}-${i}`;
        const { svg } = await mermaid.default.render(id, graphDef);
        const wrapper = document.createElement('div');
        wrapper.className = 'mermaid-rendered';
        wrapper.innerHTML = DOMPurify.sanitize(svg, {
          USE_PROFILES: { svg: true, svgFilters: true },
          ADD_TAGS: ['foreignObject'],
        });
        pre.replaceWith(wrapper);
      } catch {
        // leave original code block on render failure
      }
    }
  } catch {
    // mermaid not available — leave code blocks as-is
  }
}

function applyHighlight(container: HTMLElement): void {
  const blocks = container.querySelectorAll('pre code:not(.language-mermaid)');
  if (blocks.length === 0) return;
  import('highlight.js').then((hljs) => {
    blocks.forEach((block) => {
      hljs.default.highlightElement(block as HTMLElement);
    });
  }).catch(() => {});
}

const PreviewPane: Component<PreviewPaneProps> = (props) => {
  let containerRef: HTMLDivElement | undefined;

  createEffect(() => {
    const raw = props.html;
    if (!containerRef) return;
    const clean = DOMPurify.sanitize(raw, DOMPURIFY_CONFIG) as unknown as string;
    containerRef.innerHTML = clean;
    applyMermaid(containerRef);
    applyHighlight(containerRef);
  });

  onCleanup(() => {
    if (containerRef) containerRef.innerHTML = '';
  });

  return (
    <div
      ref={containerRef}
      class="prose prose-slate max-w-none h-full overflow-y-auto px-6 py-4
             prose-headings:font-bold prose-headings:text-slate-900
             prose-code:bg-slate-100 prose-code:px-1 prose-code:rounded
             prose-pre:bg-slate-900 prose-pre:text-slate-100
             prose-a:text-sky-600 prose-table:border-collapse
             prose-th:border prose-th:border-slate-300 prose-th:p-2 prose-th:bg-slate-50
             prose-td:border prose-td:border-slate-200 prose-td:p-2"
    />
  );
};

export default PreviewPane;
