import { getContext, setContext } from "svelte";
import { SIDEBAR_KEYBOARD_SHORTCUT } from "./constants.js";

type Getter<T> = () => T;

export type SidebarStateProps = {
  open: Getter<boolean>;
  setOpen: (open: boolean) => void;
};

class SidebarState {
  readonly props: SidebarStateProps;
  open = $derived.by(() => this.props.open());
  openMobile = $state(false);
  setOpen: SidebarStateProps["setOpen"];
  state = $derived.by(() => (this.open ? "expanded" : "collapsed"));

  constructor(props: SidebarStateProps) {
    this.setOpen = props.setOpen;
    this.props = props;
  }

  get isMobile() {
    return true;
  }

  handleShortcutKeydown = (e: KeyboardEvent) => {
    if (e.key === SIDEBAR_KEYBOARD_SHORTCUT && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      this.toggle();
    }
  };

  setOpenMobile = (value: boolean) => {
    this.openMobile = value;
  };

  toggle = () => {
    this.openMobile = !this.openMobile;
  };
}

const SYMBOL_KEY = "scn-sidebar";

export function setSidebar(props: SidebarStateProps): SidebarState {
  return setContext(Symbol.for(SYMBOL_KEY), new SidebarState(props));
}

export function useSidebar(): SidebarState {
  return getContext(Symbol.for(SYMBOL_KEY));
}
