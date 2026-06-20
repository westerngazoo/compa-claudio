import { invoke } from "@tauri-apps/api/core";

interface AppInfo {
  name: string;
  pid: number;
}

/**
 * "Look at…" right-click menu — lets the user pin Claudio's attention to a
 * specific running app, overriding the focus-based context reader. Picking
 * "follow focus (auto)" clears the pin.
 */
export class LookAtMenu {
  private el: HTMLElement;
  private items: HTMLElement;
  private outsideHandler: ((e: MouseEvent) => void) | null = null;

  constructor(root: HTMLElement) {
    this.el = root.querySelector<HTMLElement>("#lookat-menu")!;
    this.items = this.el.querySelector<HTMLElement>(".lookat-items")!;
  }

  isOpen(): boolean {
    return !this.el.classList.contains("hidden");
  }

  async show(x: number, y: number) {
    this.items.innerHTML = "";
    const loading = document.createElement("div");
    loading.className = "lookat-empty";
    loading.textContent = "loading…";
    this.items.appendChild(loading);

    this.positionAt(x, y);
    this.el.classList.remove("hidden");
    this.attachOutsideHandler();

    try {
      const [apps, current] = await Promise.all([
        invoke<AppInfo[]>("list_apps"),
        invoke<number | null>("get_target_app"),
      ]);
      this.render(apps, current);
    } catch (e) {
      loading.textContent = `couldn't list apps: ${e}`;
    }
  }

  hide() {
    this.el.classList.add("hidden");
    this.detachOutsideHandler();
  }

  private render(apps: AppInfo[], currentPid: number | null) {
    this.items.innerHTML = "";

    const autoRow = this.makeRow("follow focus (auto)", currentPid === null, () => {
      void invoke("set_target_app", { pid: null });
      this.hide();
    });
    this.items.appendChild(autoRow);

    if (apps.length === 0) {
      const empty = document.createElement("div");
      empty.className = "lookat-empty";
      empty.textContent = "no other apps running";
      this.items.appendChild(empty);
      return;
    }

    const sep = document.createElement("div");
    sep.className = "lookat-sep";
    this.items.appendChild(sep);

    for (const app of apps) {
      const row = this.makeRow(app.name, currentPid === app.pid, () => {
        void invoke("set_target_app", { pid: app.pid });
        this.hide();
      });
      this.items.appendChild(row);
    }
  }

  private makeRow(label: string, selected: boolean, onClick: () => void): HTMLElement {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "lookat-row" + (selected ? " selected" : "");
    const name = document.createElement("span");
    name.className = "lookat-name";
    name.textContent = label;
    row.appendChild(name);
    row.addEventListener("click", onClick);
    return row;
  }

  private positionAt(x: number, y: number) {
    // Keep the menu inside the window bounds.
    const menuW = 200;
    const menuH = 260;
    const maxX = window.innerWidth - menuW - 4;
    const maxY = window.innerHeight - menuH - 4;
    const clampedX = Math.max(4, Math.min(x, maxX));
    const clampedY = Math.max(4, Math.min(y, maxY));
    this.el.style.left = `${clampedX}px`;
    this.el.style.top = `${clampedY}px`;
  }

  private attachOutsideHandler() {
    if (this.outsideHandler) return;
    this.outsideHandler = (e: MouseEvent) => {
      if (!this.el.contains(e.target as Node)) {
        this.hide();
      }
    };
    // Defer so the right-click that opened the menu doesn't immediately close it.
    setTimeout(() => {
      if (this.outsideHandler) {
        document.addEventListener("mousedown", this.outsideHandler);
      }
    }, 0);
  }

  private detachOutsideHandler() {
    if (this.outsideHandler) {
      document.removeEventListener("mousedown", this.outsideHandler);
      this.outsideHandler = null;
    }
  }
}
