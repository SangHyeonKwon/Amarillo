import { type MouseEvent, useState } from "react";

import { shortAddress } from "@/lib/format";

interface AddressBadgeProps {
  address: string;
  /** Show the full address instead of the truncated form. */
  full?: boolean;
}

/** Monospace truncated address with a copy-to-clipboard affordance. */
export function AddressBadge({ address, full }: AddressBadgeProps) {
  const [copied, setCopied] = useState(false);

  async function copy(e: MouseEvent) {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      // Clipboard unavailable (e.g. non-secure context) — silently ignore.
    }
  }

  return (
    <span className="addr" title={address}>
      {full ? address : shortAddress(address)}
      <button onClick={copy} aria-label="Copy address">
        {copied ? "✓" : "⧉"}
      </button>
    </span>
  );
}
