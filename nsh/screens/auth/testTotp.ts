// Tests and fixtures only (a screen never imports it): a generated authenticator secret and its current 6-digit code
// (RFC 6238: SHA-1, 30 s steps), the same scheme bir-core checks (crypto.rs validate_totp, totp-rs, one step of skew).
import { Buffer } from "buffer";
import { createHmac, randomBytes } from "crypto";

const ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/** A fresh secret as authenticator apps take it: 20 random bytes in base32, no padding. */
export function generatedSecret(): string {
  const bytes = randomBytes(20);
  let bits = 0;
  let value = 0;
  let out = "";
  for (const byte of bytes) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      out += ALPHABET[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) out += ALPHABET[(value << (5 - bits)) & 31];
  return out;
}

function decode(secret: string): Buffer {
  const clean = secret.replace(/=+$/, "").toUpperCase();
  const out: number[] = [];
  let bits = 0;
  let value = 0;
  for (const ch of clean) {
    value = (value << 5) | ALPHABET.indexOf(ch);
    bits += 5;
    if (bits >= 8) {
      out.push((value >>> (bits - 8)) & 255);
      bits -= 8;
    }
  }
  return Buffer.from(out);
}

/** The code an authenticator app shows for `secret` at `now` (milliseconds). */
export function totpCode(secret: string, now = Date.now()): string {
  let step = Math.floor(now / 1000 / 30);
  const counter = Buffer.alloc(8);
  for (let i = 7; i >= 0; i--) {
    counter[i] = step & 255;
    step = Math.floor(step / 256);
  }
  const mac = createHmac("sha1", decode(secret)).update(counter).digest();
  const at = mac[mac.length - 1] & 15;
  const n = ((mac[at] & 127) << 24) | (mac[at + 1] << 16) | (mac[at + 2] << 8) | mac[at + 3];
  return String(n % 1_000_000).padStart(6, "0");
}
