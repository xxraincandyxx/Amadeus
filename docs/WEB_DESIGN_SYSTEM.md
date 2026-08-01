# Amadeus Interface Design System

This document is the visual and interaction contract for the React web workspace and the Tauri macOS client. The goal is continuous refinement without visual drift.

## Product character

Amadeus is a dense developer workspace, not a marketing dashboard. Its interface should feel calm, native, and operational:

- dark neutral surfaces with one burnt-orange operational accent;
- a persistent project/session sidebar;
- a narrow readable conversation column;
- a fixed composer that remains the primary action;
- compact controls, quiet borders, and explicit system state;
- low-motion feedback that communicates progress or state changes.

The current design dials are variance 4, motion 3, and density 7. New work should preserve that balance unless a deliberate redesign proposal changes this document first.

## Tokens

The canonical runtime tokens live in `apps/web/src/styles.css` under `:root`.

| Role | Token | Current value | Usage |
| --- | --- | --- | --- |
| Canvas | `--bg` | `#171717` | Main workspace |
| Surface | `--surface` | `#202020` | Cards and grouped content |
| Raised surface | `--surface-raised` | `#292929` | Inputs and elevated controls |
| Hover surface | `--surface-hover` | `#323232` | Interactive hover state |
| Sidebar | `--sidebar` | `#333333` | Persistent navigation |
| Border | `--border` | `#3a3a3a` | Primary separators |
| Text | `--text` | `#f0f0f0` | Primary copy |
| Muted text | `--muted` | `#a7a7a7` | Secondary information |
| Quiet text | `--quiet` | `#757575` | Metadata |
| Accent | `--accent` | `#ef7d32` | Active work and approvals |
| Success | `--green` | `#55c97a` | Connected/completed status |
| Failure | `--red` | `#ec6b6b` | Errors and destructive affordances |
| Component radius | `--radius` | `14px` | Event and tool surfaces |

Do not add a second accent color. Green and red are semantic status colors, not decorative accents. Prefer existing tokens over literal values when a color carries the same role across components.

## Shape and spacing

- Major cards use 12–17 px radii; compact controls use 7–10 px; status badges may be pill-shaped.
- The conversation column is capped at 920 px for readable long-form output.
- Desktop navigation is 292 px wide. Mobile navigation becomes an overlay below 820 px.
- Controls normally use 34–42 px heights. Avoid oversized marketing-style calls to action.
- Use borders, dividers, and negative space for hierarchy before adding shadows.
- Shadows are reserved for the composer, dialogs, drawers, and other genuinely elevated layers.

## Typography and icons

The interface uses the local Geist/SF Pro/Segoe UI system stack and platform monospace fonts for identifiers, commands, and code. Keep body copy at readable 13–15 px sizes and reserve larger display type for empty or welcome states.

Use `@phosphor-icons/react` exclusively. Icons should normally render at 14–20 px and inherit semantic color from their control. Preserve the geometric sparkle as the Amadeus product mark. Do not introduce emoji, hand-drawn icon paths, or unrelated raster branding.

## Rich response content

Assistant Markdown belongs directly in the message flow rather than inside a generic card. Preserve the body measure and use typographic hierarchy for headings, paragraphs, lists, blockquotes, and emphasis. Inline code uses a restrained neutral surface with the orange accent reserved for text contrast. Fenced code uses the established 11 px radius, a quiet language header, a Phosphor copy action, and horizontal overflow. Tables use sparse row separators, a slightly raised header surface, and horizontal scrolling at narrow widths.

Reasoning remains a separate inline inspector before the final answer. Available reasoning uses the existing Show and Hide disclosure. Missing provider reasoning uses a non-interactive Reasoning unavailable status so absence cannot be mistaken for a rendering failure.

## Component behavior

Every interactive component must support the states that can occur in production.

| Component | Required states |
| --- | --- |
| Session list | empty, active, idle, running, awaiting approval, failed |
| Conversation | welcome, hydrated history, streaming, tool execution, approval, error |
| Composer | ready, empty, disabled/offline, busy/cancellable, keyboard focus |
| Connection | unknown/loading, connected, offline, testing, invalid URL, reconnecting |
| Dialog | open, keyboard focus, validation error, submit-disabled where applicable |
| External resource | hover, focus, clear destination indicator |

Errors that block work should be contextual and actionable. A failed connection should offer Retry and Settings; a form validation error should stay inside its dialog. Toasts should be reserved for transient confirmations.

## Motion and accessibility

- Motion communicates feedback, progress, or a state transition. Decorative infinite motion is not part of the product language.
- Honor `prefers-reduced-motion` for every transition and animation.
- All controls require visible keyboard focus.
- Interactive targets should be at least 34 px in dense desktop UI and larger where mobile layout permits.
- Text and control contrast must meet WCAG AA.
- Dialogs need an accessible name, modal semantics, and logical keyboard order. Focus trapping is a planned hardening item for a shared dialog primitive.
- Do not encode status through color alone; pair color with text, position, or an icon.

## Native macOS rules

The Tauri window uses native traffic lights with an overlay title bar. Browser-only decorative traffic lights are hidden in native mode, and the layout reserves the titlebar area. Do not replace native close, minimize, or full-screen controls with HTML buttons.

The web workspace remains the single product implementation. Native adjustments should use the `is-tauri` root class or an equivalent explicit runtime boundary. Platform-specific styling must not regress the browser layout.

## Contribution checklist for visual changes

1. State the user problem and the interaction state being added or improved.
2. Reuse the established token, shape, typography, and icon rules.
3. Check loading, empty, offline, error, success, disabled, and reduced-motion states.
4. Test keyboard navigation and focus visibility.
5. Inspect the result at the normal desktop window, the minimum native window, and 390 × 844 mobile.
6. Test both the browser and Tauri client when shared layout or runtime detection changed.
7. Include before/after screenshots in the pull request and note any intentional deviation from this document.

If a feature cannot fit this language cleanly, update the design system as part of the same proposal rather than introducing an isolated visual exception.
