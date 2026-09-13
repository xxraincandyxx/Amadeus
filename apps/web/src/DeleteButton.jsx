// @amadeus-header
// summary: Renders a two-step destructive button that requires a second click to confirm.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: ConfirmDeleteButton
// uses:
// - module: apps/web/src/i18n.js
// invariants:
// - The destructive action runs only after two deliberate activations of the control.
// - The armed state resets itself after a timeout so a stray click never deletes.
// side_effects: none
// tests:
// - cmd: npm run build
// @end-amadeus-header

import { Trash } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

export function ConfirmDeleteButton({ label = "", title, confirmLabel, disabled = false, className = "", onConfirm }) {
  const [armed, setArmed] = useState(false);

  useEffect(() => {
    if (!armed) return undefined;
    const timer = window.setTimeout(() => setArmed(false), 4000);
    return () => window.clearTimeout(timer);
  }, [armed]);

  const text = armed ? confirmLabel : title || label;
  return (
    <button
      type="button"
      className={`${className} ${armed ? "confirm-armed" : ""}`.trim()}
      title={text}
      aria-label={text}
      disabled={disabled}
      onClick={() => {
        if (armed) {
          setArmed(false);
          onConfirm();
          return;
        }
        setArmed(true);
      }}
    >
      <Trash aria-hidden="true" />
      {label && <span>{armed ? confirmLabel : label}</span>}
    </button>
  );
}
