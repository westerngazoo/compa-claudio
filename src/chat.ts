import { invoke } from "@tauri-apps/api/core";
import type { PersonalityController } from "./personalities";

interface ChatMessage {
  role: "user" | "assistant" | "system";
  content: string;
  images?: string[];
}

interface ChatContext {
  focusedApp?: string | null;
  focusedText?: string | null;
  selection?: string | null;
}

export class Chat {
  private panel: HTMLElement;
  private messagesEl: HTMLElement;
  private form: HTMLFormElement;
  private input: HTMLInputElement;
  private sendBtn: HTMLButtonElement;
  private screenBtn: HTMLButtonElement;
  private history: ChatMessage[] = [];
  private pendingContext: ChatContext | null = null;
  private sending = false;
  private personality: PersonalityController;

  constructor(root: HTMLElement, personality: PersonalityController) {
    this.panel = root.querySelector<HTMLElement>("#chat")!;
    this.messagesEl = root.querySelector<HTMLElement>("#chat-messages")!;
    this.form = root.querySelector<HTMLFormElement>("#chat-form")!;
    this.input = root.querySelector<HTMLInputElement>("#chat-input")!;
    this.sendBtn = root.querySelector<HTMLButtonElement>("#chat-send")!;
    this.screenBtn = root.querySelector<HTMLButtonElement>("#chat-screen")!;
    this.personality = personality;

    this.form.addEventListener("submit", (e) => {
      e.preventDefault();
      const text = this.input.value.trim();
      if (!text || this.sending) return;
      this.input.value = "";
      void this.send(text);
    });

    // The eye button: Claudio takes a look at the screen.
    this.screenBtn.addEventListener("click", () => void this.captureScreen());
  }

  isOpen(): boolean {
    return !this.panel.classList.contains("hidden");
  }

  async open(withContext?: ChatContext) {
    this.panel.classList.remove("hidden");

    if (withContext) {
      this.pendingContext = withContext;
    } else if (!this.pendingContext) {
      // No explicit context → snapshot whatever the user was just looking at.
      // The Rust side keeps a cached read of the last non-self focused window,
      // so this is a fast cache hit, not a live AX call.
      try {
        const ctx = await invoke<{
          focused_app?: string | null;
          focused_text?: string | null;
          selection?: string | null;
        }>("get_focused_context");
        if (ctx && (ctx.focused_app || ctx.focused_text || ctx.selection)) {
          this.pendingContext = {
            focusedApp: ctx.focused_app ?? undefined,
            focusedText: ctx.focused_text ?? undefined,
            selection: ctx.selection ?? undefined,
          };
        }
      } catch {
        // No accessibility, no problem — chat still works, just without context.
      }
    }

    setTimeout(() => this.input.focus(), 80);

    if (this.history.length === 0) {
      this.appendAssistant(this.greetingFor(this.pendingContext));
    }
  }

  private greetingFor(ctx: ChatContext | null): string {
    if (!ctx) return "hey — what are we studying?";
    if (ctx.selection) {
      return `I see the bit you've selected. want me to walk through it?`;
    }
    if (ctx.focusedText && ctx.focusedApp) {
      const app = ctx.focusedApp.split(" — ")[0];
      return `I see you're in ${app}. what do you want to dig into?`;
    }
    if (ctx.focusedApp) {
      return `you're in ${ctx.focusedApp.split(" — ")[0]}. what are we looking at?`;
    }
    return "hey — what are we studying?";
  }

  close() {
    this.panel.classList.add("hidden");
  }

  async toggle(withContext?: ChatContext) {
    if (this.isOpen()) this.close();
    else await this.open(withContext);
  }

  /** Eye button — capture the screen and ask Claudio about it. */
  private async captureScreen() {
    if (this.sending) return;
    this.sending = true;
    this.setBusy(true);
    const thinking = this.appendThinking("looking at your screen");

    let shot: string | null = null;
    try {
      const perm = await invoke<string>("screen_permission_status").catch(
        () => "notApplicable"
      );
      if (perm === "notApplicable") {
        thinking.remove();
        this.appendError("screen capture isn't available on this platform yet.");
        return;
      }
      if (perm !== "granted") {
        thinking.remove();
        await invoke("request_screen_permission").catch(() => {});
        this.appendAssistant(
          "I need Screen Recording permission to look — turn me on in System Settings → Privacy & Security → Screen Recording, then quit & reopen me."
        );
        return;
      }
      shot = await invoke<string>("capture_screen");
      thinking.remove();
    } catch (err) {
      thinking.remove();
      this.appendError(typeof err === "string" ? err : String(err));
    } finally {
      this.sending = false;
      this.setBusy(false);
    }

    if (shot) {
      const question =
        this.input.value.trim() || "what am I looking at? give me a quick read.";
      this.input.value = "";
      await this.send(question, [shot]);
    }
  }

  private async send(text: string, images: string[] = []) {
    this.sending = true;
    this.setBusy(true);

    this.history.push({ role: "user", content: text, images });
    this.appendUser(text, images.length > 0);
    const thinking = this.appendThinking();

    try {
      // Prepend the active personality's voice as a system message so the
      // backend always gets Claudio's current vibe as its system prompt.
      const outgoing: ChatMessage[] = [
        { role: "system", content: this.personality.get().voice },
        ...this.history,
      ];
      const reply = await invoke<string>("send_message", {
        messages: outgoing.map((m) => ({
          role: m.role,
          content: m.content,
          images: m.images ?? [],
        })),
        context: this.pendingContext
          ? {
              focused_app: this.pendingContext.focusedApp ?? null,
              focused_text: this.pendingContext.focusedText ?? null,
              selection: this.pendingContext.selection ?? null,
            }
          : null,
      });
      this.pendingContext = null;
      thinking.remove();
      this.history.push({ role: "assistant", content: reply });
      this.appendAssistant(reply);
    } catch (err) {
      thinking.remove();
      this.appendError(typeof err === "string" ? err : String(err));
    } finally {
      this.sending = false;
      this.setBusy(false);
      setTimeout(() => this.input.focus(), 40);
    }
  }

  private setBusy(on: boolean) {
    this.sendBtn.disabled = on;
    this.screenBtn.disabled = on;
  }

  private appendUser(text: string, sharedScreen = false) {
    const el = document.createElement("div");
    el.className = "msg user";
    if (sharedScreen) {
      const badge = document.createElement("span");
      badge.className = "shot-badge";
      badge.textContent = "shared screen";
      el.appendChild(badge);
    }
    const body = document.createElement("span");
    body.textContent = text;
    el.appendChild(body);
    this.messagesEl.appendChild(el);
    this.scrollToBottom();
  }

  private appendAssistant(text: string) {
    const el = document.createElement("div");
    el.className = "msg assistant";
    el.textContent = "";
    this.messagesEl.appendChild(el);
    this.revealText(el, text);
  }

  private appendError(text: string) {
    const el = document.createElement("div");
    el.className = "msg error";
    el.textContent = text;
    this.messagesEl.appendChild(el);
    this.scrollToBottom();
  }

  private appendThinking(label = "thinking"): HTMLElement {
    const el = document.createElement("div");
    el.className = "msg thinking";
    el.textContent = label;
    this.messagesEl.appendChild(el);
    this.scrollToBottom();
    return el;
  }

  private revealText(el: HTMLElement, text: string) {
    // Cheap streaming-feel reveal so even Mock replies feel alive.
    let i = 0;
    const tick = () => {
      const chunk = Math.max(1, Math.floor(text.length / 60));
      i = Math.min(i + chunk, text.length);
      el.textContent = text.slice(0, i);
      this.scrollToBottom();
      if (i < text.length) setTimeout(tick, 14);
    };
    tick();
  }

  private scrollToBottom() {
    this.messagesEl.scrollTop = this.messagesEl.scrollHeight;
  }
}
