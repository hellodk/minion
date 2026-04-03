declare module 'page-flip' {
  export class PageFlip {
    constructor(element: HTMLElement, settings: Record<string, string | number | boolean>);
    loadFromHTML(items: HTMLElement[] | NodeListOf<HTMLElement>): void;
    destroy(): void;
    flipNext(corner?: string): void;
    flipPrev(corner?: string): void;
    on(event: string, handler: (e: { data: unknown; object: PageFlip }) => void): PageFlip;
    off(event: string): void;
  }
}
