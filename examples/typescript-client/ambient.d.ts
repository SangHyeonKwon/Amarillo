// Minimal ambient declarations so the example client typechecks WITHOUT any
// npm dependencies (D017 — "no runtime deps; no devDependencies either"). If
// you copy this into a real project that already pulls in @types/node, delete
// this file — your @types/node declarations are richer.

declare module "node:crypto" {
  export interface Hmac {
    update(data: string | Uint8Array): Hmac;
    digest(): Uint8Array;
    digest(encoding: "hex"): string;
  }
  export function createHmac(algorithm: string, key: Uint8Array | string): Hmac;
  export function timingSafeEqual(a: Uint8Array, b: Uint8Array): boolean;
}

// `Buffer` is a Node-only global; `Buffer.from(hex, "hex")` gives a Uint8Array
// equivalent we hand to `timingSafeEqual`.
declare const Buffer: {
  from(input: string, encoding: "hex" | "utf8"): Uint8Array & {
    toString(encoding: "utf8" | "hex"): string;
    length: number;
  };
};

// Node `process` — just the bits we use in the demo's CLI argv handling.
declare const process: {
  argv: string[];
  exit(code: number): never;
  env: Record<string, string | undefined>;
};
