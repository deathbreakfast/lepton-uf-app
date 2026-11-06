import { authenticator } from "otplib";
import { normalizeBase32 } from "./totp";

const STEP_MS = 30_000;

/**
 * Fresh-code step-up rejects a code from a time step that was already used. Each
 * code comes from its own time step, and `next` waits until that step is the
 * current one, so the server never sees a code from the future.
 */
export class FreshCodes {
  private lastStep = Math.floor(Date.now() / STEP_MS);

  constructor(private readonly secret: string) {}

  async next(): Promise<string> {
    const step = this.lastStep + 1;
    const waitMs = step * STEP_MS - Date.now() + 250;
    if (waitMs > 0) {
      await new Promise((r) => setTimeout(r, waitMs));
    }
    this.lastStep = step;
    const at = authenticator.clone({ epoch: step * STEP_MS });
    return at.generate(normalizeBase32(this.secret));
  }
}
