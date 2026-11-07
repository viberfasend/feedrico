// Token values in pixels for code that must compute geometry (virtualized Layouts).
// Re-reads when density or theme switches, so sizes always match the CSS.

class TokenRuntime {
  version = $state(0);

  constructor() {
    if (typeof document === 'undefined') return;
    new MutationObserver(() => this.version++).observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-density', 'data-theme'],
    });
  }

  /** A token's computed value as a number, e.g. px('--cards-gap') → 16. */
  px(name: string): number {
    void this.version;
    return parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name)) || 0;
  }
}

export const tokens = new TokenRuntime();
