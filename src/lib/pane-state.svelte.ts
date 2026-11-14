// Machine-local pane sizes (spec: "What stays on the machine"). Not stored in the Library:
// they depend on this screen. Values are preferences; panes.ts derives what is actually used.
import { parseStoredPanes } from './panes';

const KEY = 'feedrico.panes';

class PaneState {
  /** null means "the token default". */
  sidebar = $state<number | null>(null);
  list = $state<number | null>(null);
  slideOver = $state<number | null>(null);
  sidebarHidden = $state(false);

  constructor() {
    if (typeof localStorage === 'undefined') return;
    const stored = parseStoredPanes(localStorage.getItem(KEY));
    this.sidebar = stored.sidebar;
    this.list = stored.list;
    this.slideOver = stored.slideOver;
    this.sidebarHidden = stored.sidebarHidden;
  }

  save() {
    if (typeof localStorage === 'undefined') return;
    localStorage.setItem(
      KEY,
      JSON.stringify({
        sidebar: this.sidebar,
        list: this.list,
        slideOver: this.slideOver,
        sidebarHidden: this.sidebarHidden,
      }),
    );
  }

  set(pane: 'sidebar' | 'list' | 'slideOver', px: number | null) {
    this[pane] = px === null ? null : Math.round(px);
    if (pane === 'sidebar') this.sidebarHidden = false;
    this.save();
  }

  toggleSidebar() {
    this.sidebarHidden = !this.sidebarHidden;
    this.save();
  }
}

export const panes = new PaneState();
