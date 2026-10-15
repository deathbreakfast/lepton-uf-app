import { authenticator } from "otplib";
import { normalizeBase32 } from "./totp";

const STEP_MS = 30_000;

/**
 * Fresh-code step-up rejects a code from a time step that was already used. A
 * code is also accepted one step ahead, so the first write needs no wait and a
 * later write waits only until the next step opens.
 */
export class FreshCodes {
  private lastStep = Math.floor(Date.now() / STEP_MS);

  constructor(private readonly secret: string) {}

  async next(): Promise<string> {
    let step = this.lastStep + 1;
    const nowStep = Math.floor(Date.now() / STEP_MS);
    if (step > nowStep + 1) {
      const waitMs = (step - 1) * STEP_MS - Date.now() + 250;
      await new Promise((r) => setTimeout(r, Math.max(waitMs, 0)));
    }
    this.lastStep = step;
    const at = authenticator.clone({ epoch: step * STEP_MS });
    return at.generate(normalizeBase32(this.secret));
  }
}
