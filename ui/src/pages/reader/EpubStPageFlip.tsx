import { Component, onMount, onCleanup } from 'solid-js';
import { PageFlip } from 'page-flip';

/** StPageFlip `flippingTime` (ms) — must match completion timer in EpubStPageFlip. */
const EPUB_PAGE_FLIP_MS = 960;

function escapeHtmlTitle(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

interface EpubStPageFlipProps {
  dir: 'forward' | 'back';
  outgoingTitle: string;
  incomingTitle: string;
  outgoingHtml: string;
  incomingHtml: string;
  proseClass: string;
  chapterTitleColor: string;
  onComplete: () => void;
}

const EpubStPageFlip: Component<EpubStPageFlipProps> = (props) => {
  let host: HTMLDivElement | undefined;
  let pf: PageFlip | undefined;
  let doneTimer: ReturnType<typeof setTimeout> | undefined;
  let completed = false;

  const finish = () => {
    if (completed) return;
    completed = true;
    if (doneTimer !== undefined) clearTimeout(doneTimer);
    props.onComplete();
  };

  onMount(() => {
    if (!host) {
      finish();
      return;
    }

    const p0 = document.createElement('div');
    p0.dataset.density = 'soft';
    const p1 = document.createElement('div');
    p1.dataset.density = 'soft';

    const hStyle = `color: ${props.chapterTitleColor}; letter-spacing: -0.02em;`;
    const wrap = (title: string, html: string) =>
      `<h1 class="text-2xl font-bold mb-8" style="${hStyle}">${escapeHtmlTitle(title)}</h1><div class="prose max-w-none ${props.proseClass}">${html}</div>`;

    if (props.dir === 'forward') {
      p0.innerHTML = wrap(props.outgoingTitle, props.outgoingHtml);
      p1.innerHTML = wrap(props.incomingTitle, props.incomingHtml);
    } else {
      p0.innerHTML = wrap(props.incomingTitle, props.incomingHtml);
      p1.innerHTML = wrap(props.outgoingTitle, props.outgoingHtml);
    }

    try {
      pf = new PageFlip(host, {
        width: 520,
        height: 720,
        size: 'stretch',
        minWidth: 280,
        maxWidth: 960,
        minHeight: 420,
        maxHeight: 2400,
        flippingTime: EPUB_PAGE_FLIP_MS,
        usePortrait: true,
        maxShadowOpacity: 0.48,
        drawShadow: true,
        showPageCorners: false,
        useMouseEvents: false,
        disableFlipByClick: true,
        mobileScrollSupport: false,
        autoSize: true,
        showCover: false,
        startPage: props.dir === 'forward' ? 0 : 1,
        startZIndex: 0,
      });

      pf.loadFromHTML([p0, p1]);

      requestAnimationFrame(() => {
        if (!pf) return;
        if (props.dir === 'forward') {
          pf.flipNext('top');
        } else {
          pf.flipPrev('top');
        }
      });

      doneTimer = setTimeout(finish, EPUB_PAGE_FLIP_MS + 120);
    } catch (e) {
      console.error('StPageFlip failed:', e);
      finish();
    }
  });

  onCleanup(() => {
    if (doneTimer !== undefined) clearTimeout(doneTimer);
    if (pf) {
      try {
        pf.destroy();
      } catch {
        /* host may already be detached */
      }
      pf = undefined;
    }
  });

  return (
    <div
      ref={(el) => {
        host = el;
      }}
      class="epub-st-page-flip-host w-full min-h-[65vh] min-w-0"
    />
  );
};

export { EpubStPageFlip };
export type { EpubStPageFlipProps };
