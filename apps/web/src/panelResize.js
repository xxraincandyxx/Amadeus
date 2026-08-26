// @amadeus-header
// summary: Provides persisted pointer and keyboard resizing for desktop side panels.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: clampPanelWidth
// - fn: resizedPanelWidth
// - fn: useResizablePanel
// uses:
// - library: React
// invariants:
// - Panel widths remain within their configured minimum and maximum.
// - Continuous pointer movement updates CSS without rerendering the React tree.
// side_effects:
// - Reads and writes browser local storage.
// - Registers temporary window pointer listeners while dragging.
// tests:
// - apps/web/src/panelResize.test.js
// @end-amadeus-header

import { useCallback, useEffect, useRef, useState } from "react";

export function clampPanelWidth(width, minimum, maximum) {
  const numericWidth = Number(width);
  if (!Number.isFinite(numericWidth)) return minimum;
  return Math.min(maximum, Math.max(minimum, numericWidth));
}

export function resizedPanelWidth(startWidth, pointerDelta, minimum, maximum) {
  return clampPanelWidth(startWidth + pointerDelta, minimum, maximum);
}

function storedPanelWidth(storageKey, defaultWidth, minimum, maximum) {
  try {
    const stored = window.localStorage.getItem(storageKey);
    return stored === null ? defaultWidth : clampPanelWidth(stored, minimum, maximum);
  } catch {
    return defaultWidth;
  }
}

export function useResizablePanel({ storageKey, defaultWidth, minimum, maximum }) {
  const [committedWidth, setCommittedWidth] = useState(() => storedPanelWidth(storageKey, defaultWidth, minimum, maximum));
  const containerRef = useRef(null);
  const liveWidthRef = useRef(committedWidth);
  const dragCleanupRef = useRef(null);

  const applyWidth = useCallback((nextWidth) => {
    const width = clampPanelWidth(nextWidth, minimum, maximum);
    liveWidthRef.current = width;
    containerRef.current?.style.setProperty("--panel-width", `${width}px`);
    return width;
  }, [maximum, minimum]);

  const commitWidth = useCallback((nextWidth) => {
    const width = applyWidth(nextWidth);
    setCommittedWidth(width);
    try {
      window.localStorage.setItem(storageKey, String(width));
    } catch {
      // Storage can be unavailable in hardened browser contexts.
    }
  }, [applyWidth, storageKey]);

  useEffect(() => () => dragCleanupRef.current?.(), []);

  const onPointerDown = useCallback((event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = liveWidthRef.current;

    const cleanup = () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerUp);
      window.removeEventListener("keydown", onCancel);
      document.body.classList.remove("is-resizing-panel");
      dragCleanupRef.current = null;
    };
    const onPointerMove = (moveEvent) => {
      applyWidth(resizedPanelWidth(startWidth, moveEvent.clientX - startX, minimum, maximum));
    };
    const onPointerUp = () => {
      cleanup();
      commitWidth(liveWidthRef.current);
    };
    const onCancel = (keyEvent) => {
      if (keyEvent.key !== "Escape") return;
      cleanup();
      applyWidth(startWidth);
    };

    dragCleanupRef.current?.();
    dragCleanupRef.current = cleanup;
    document.body.classList.add("is-resizing-panel");
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
    window.addEventListener("keydown", onCancel);
  }, [applyWidth, commitWidth, maximum, minimum]);

  const onKeyDown = useCallback((event) => {
    let nextWidth = liveWidthRef.current;
    if (event.key === "ArrowLeft") nextWidth -= event.shiftKey ? 32 : 8;
    else if (event.key === "ArrowRight") nextWidth += event.shiftKey ? 32 : 8;
    else if (event.key === "Home") nextWidth = minimum;
    else if (event.key === "End") nextWidth = maximum;
    else return;
    event.preventDefault();
    commitWidth(nextWidth);
  }, [commitWidth, maximum, minimum]);

  return {
    containerRef,
    containerStyle: { "--panel-width": `${committedWidth}px` },
    handleProps: {
      role: "separator",
      tabIndex: 0,
      "aria-orientation": "vertical",
      "aria-valuemin": minimum,
      "aria-valuemax": maximum,
      "aria-valuenow": committedWidth,
      onPointerDown,
      onKeyDown,
      onDoubleClick: () => commitWidth(defaultWidth),
    },
  };
}
