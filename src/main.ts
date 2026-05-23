import { Mascot } from "./mascot";
import { Chat } from "./chat";
import { Settings } from "./settings";
import { Onboarding } from "./onboarding";
import { PersonalityController, type Personality } from "./personalities";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

window.addEventListener("DOMContentLoaded", () => {
  const root = document.body;

  const personality = new PersonalityController();
  const mascot = new Mascot(root);
  const chat = new Chat(root, personality);
  const settings = new Settings(root, personality);
  const onboarding = new Onboarding(root);

  // When Claudio's personality changes, swap his hat + shift the UI accent.
  personality.onChange((p: Personality) => {
    mascot.setPersonality(p);
    document.documentElement.style.setProperty("--warm-clay", p.accent);
  });
  personality.init();

  void onboarding.maybeShow();

  // --- Click-through reporting ---------------------------------------------
  // The Rust click-through poller needs to know when a panel is open: panel
  // open → the whole window catches clicks; idle → only Claudio does. One
  // MutationObserver catches every panel show/hide.
  const panelSelectors = ["#chat", "#settings", "#onboarding"];
  const reportPanelState = () => {
    const open = panelSelectors.some(
      (sel) => !document.querySelector(sel)?.classList.contains("hidden")
    );
    void invoke("set_panel_open", { open }).catch(() => {});
  };
  const panelObserver = new MutationObserver(reportPanelState);
  for (const sel of panelSelectors) {
    const el = document.querySelector(sel);
    if (el) {
      panelObserver.observe(el, { attributes: true, attributeFilter: ["class"] });
    }
  }
  reportPanelState();

  // --- Window dragging -----------------------------------------------------
  // Native macOS window drag via Tauri's startDragging — the OS itself keeps
  // the cursor glued to wherever the user grabbed Claudio, so there's no
  // coordinate math that can drift on multi-monitor / mixed-DPR setups.
  // (Needed `core:window:allow-start-dragging` capability, granted in
  // src-tauri/capabilities/default.json.)
  //
  // The 3-px threshold ensures a plain click / double-click doesn't kick off
  // a drag — keeps double-click-to-open-chat working.
  const mascotEl = document.querySelector<HTMLElement>("#mascot")!;
  const win = getCurrentWindow();

  mascotEl.addEventListener("mousedown", (e) => {
    if (e.button !== 0) return;
    const startCx = e.screenX;
    const startCy = e.screenY;
    let started = false;

    const onMove = (m: MouseEvent) => {
      if (started) return;
      if (Math.hypot(m.screenX - startCx, m.screenY - startCy) > 3) {
        started = true;
        cleanup();
        void win.startDragging().catch(() => {});
      }
    };
    const cleanup = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", cleanup);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", cleanup);
  });

  // Double-click the mascot → open chat.
  mascot.onClick(() => {
    if (settings.isOpen()) settings.close();
    void chat.toggle();
  });

  mascot.onThoughtClick(() => {
    if (settings.isOpen()) settings.close();
    void chat.open();
    mascot.hideThought();
  });

  // Corner controls
  document.querySelector<HTMLButtonElement>("#btn-settings")?.addEventListener("click", () => {
    if (chat.isOpen()) chat.close();
    settings.toggle();
  });
  document.querySelector<HTMLButtonElement>("#btn-quit")?.addEventListener("click", () => {
    void invoke("quit_app");
  });

  // Keyboard: Esc closes panels; Cmd/Ctrl+W or +Q quits.
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      if (settings.isOpen()) settings.close();
      else if (chat.isOpen()) chat.close();
      return;
    }
    if ((e.metaKey || e.ctrlKey) && (e.key === "w" || e.key === "q")) {
      e.preventDefault();
      void invoke("quit_app");
    }
  });

  // Event-driven personality auto-switch — the Rust AccessibilitySensor
  // publishes a "context-event" whenever the focused app changes.
  void listen<{ kind: string; app?: string | null }>("context-event", (e) => {
    if (e.payload.kind === "focusChanged") {
      personality.applyForApp(e.payload.app ?? null);
    }
  });

  // Slice-1 stand-in for the dwell detector: hover the mascot for 1.2s and it
  // pops a thought bubble. To be replaced by the real dwell detector later.
  let hoverTimer: number | null = null;
  mascotEl.addEventListener("mouseenter", () => {
    if (hoverTimer) window.clearTimeout(hoverTimer);
    hoverTimer = window.setTimeout(() => {
      if (!chat.isOpen() && !settings.isOpen()) mascot.showThought();
    }, 1200);
  });
  mascotEl.addEventListener("mouseleave", () => {
    if (hoverTimer) window.clearTimeout(hoverTimer);
    setTimeout(() => mascot.hideThought(), 1800);
  });
});
