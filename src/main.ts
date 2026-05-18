import { Mascot } from "./mascot";
import { Chat } from "./chat";
import { Settings } from "./settings";
import { Onboarding } from "./onboarding";
import { PersonalityController, type Personality } from "./personalities";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalPosition } from "@tauri-apps/api/dpi";

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
  // We move the window ourselves: snapshot its position on mousedown, then
  // reposition by the cursor delta, coalesced to one call per frame.
  const mascotEl = document.querySelector<HTMLElement>("#mascot")!;
  const win = getCurrentWindow();
  let scaleFactor = 1;
  void win
    .scaleFactor()
    .then((s) => (scaleFactor = s))
    .catch(() => {});

  mascotEl.addEventListener("mousedown", (e) => {
    if (e.button !== 0) return;
    const startCx = e.screenX;
    const startCy = e.screenY;
    let baseX = 0;
    let baseY = 0;
    let ready = false;
    let moved = false;
    let latest: MouseEvent | null = null;
    let rafId: number | null = null;

    void win
      .outerPosition()
      .then((p) => {
        baseX = p.x / scaleFactor;
        baseY = p.y / scaleFactor;
        ready = true;
      })
      .catch(() => {});

    const apply = () => {
      rafId = null;
      if (!ready || !latest) return;
      const dx = latest.screenX - startCx;
      const dy = latest.screenY - startCy;
      void win.setPosition(new LogicalPosition(baseX + dx, baseY + dy)).catch(() => {});
    };

    const onMove = (m: MouseEvent) => {
      // A few px of slack so a plain click / double-click doesn't move the
      // window — keeps double-click-to-open-chat working.
      if (!moved && Math.hypot(m.screenX - startCx, m.screenY - startCy) > 3) {
        moved = true;
      }
      if (!moved) return;
      latest = m;
      if (rafId === null) rafId = requestAnimationFrame(apply);
    };

    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      if (rafId !== null) cancelAnimationFrame(rafId);
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
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
