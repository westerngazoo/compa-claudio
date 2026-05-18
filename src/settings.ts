import { invoke } from "@tauri-apps/api/core";
import { PERSONALITIES, type PersonalityController } from "./personalities";

interface BackendInfo {
  id: string;
  label: string;
  available: boolean;
}

export class Settings {
  private panel: HTMLElement;
  private backendList: HTMLElement;
  private personalityList: HTMLElement;
  private autoCheckbox: HTMLInputElement;
  private closeBtn: HTMLButtonElement;
  private personality: PersonalityController;

  constructor(root: HTMLElement, personality: PersonalityController) {
    this.panel = root.querySelector<HTMLElement>("#settings")!;
    this.backendList = root.querySelector<HTMLElement>("#backend-list")!;
    this.personalityList = root.querySelector<HTMLElement>("#personality-list")!;
    this.autoCheckbox = root.querySelector<HTMLInputElement>("#personality-auto")!;
    this.closeBtn = root.querySelector<HTMLButtonElement>("#settings-close")!;
    this.personality = personality;

    this.closeBtn.addEventListener("click", () => this.close());

    this.autoCheckbox.addEventListener("change", () => {
      this.personality.setAuto(this.autoCheckbox.checked);
    });

    // Keep the picker's highlight in sync when the personality changes for
    // any reason (manual click, or an auto-switch while settings is open).
    this.personality.onChange(() => {
      if (this.isOpen()) this.renderPersonalities();
    });
  }

  async open() {
    this.panel.classList.remove("hidden");
    this.renderPersonalities();
    await this.refreshBackends();
  }

  close() {
    this.panel.classList.add("hidden");
  }

  isOpen(): boolean {
    return !this.panel.classList.contains("hidden");
  }

  toggle() {
    if (this.isOpen()) this.close();
    else void this.open();
  }

  private renderPersonalities() {
    this.autoCheckbox.checked = this.personality.isAuto();
    const currentId = this.personality.get().id;
    this.personalityList.innerHTML = "";

    for (const p of PERSONALITIES) {
      const row = document.createElement("div");
      row.className = "personality-option";
      if (p.id === currentId) row.classList.add("selected");

      const swatch = document.createElement("span");
      swatch.className = "swatch";
      swatch.style.background = p.accent;

      const text = document.createElement("span");
      text.className = "p-text";
      const name = document.createElement("span");
      name.className = "p-name";
      name.textContent = p.name;
      const tag = document.createElement("span");
      tag.className = "p-tag";
      tag.textContent = p.tagline;
      text.append(name, tag);

      row.append(swatch, text);
      row.addEventListener("click", () => {
        this.personality.setManual(p.id);
        this.renderPersonalities();
      });

      this.personalityList.appendChild(row);
    }
  }

  private async refreshBackends() {
    this.backendList.innerHTML = "";
    try {
      const [backends, currentId] = await Promise.all([
        invoke<BackendInfo[]>("list_backends"),
        invoke<string>("get_current_backend"),
      ]);

      for (const b of backends) {
        const row = document.createElement("div");
        row.className = "backend-option";
        if (b.available) row.classList.add("available");
        if (b.id === currentId) row.classList.add("selected");

        row.innerHTML = `
          <span class="dot"></span>
          <span class="label"></span>
          <span class="badge"></span>
        `;
        row.querySelector(".label")!.textContent = b.label;
        row.querySelector(".badge")!.textContent = b.available ? "ready" : "offline";

        row.addEventListener("click", async () => {
          try {
            await invoke("set_backend", { id: b.id });
            await this.refreshBackends();
          } catch (err) {
            console.error(err);
          }
        });

        this.backendList.appendChild(row);
      }
    } catch (err) {
      this.backendList.textContent = `Couldn't load backends: ${err}`;
    }
  }
}
