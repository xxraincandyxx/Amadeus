---
name: Paper Visual Correctness Polish
description: >-
  When the user has a LaTeX paper (any .tex file, .pdf, or paper directory)
  and wants to check visual correctness, fix rendering bugs, polish formatting,
  or prepare for submission. Use this skill whenever the user mentions:
  LaTeX compilation errors, citation issues [?), undefined references, figure
  placement, table overflows, underfull/overfull hbox/vbox warnings, bibliography
  formatting, reference style problems, PDF rendering checks, em-dashes/en-dashes,
  verbatim overflow, margin issues, bad hyphenation, empty pages, orphan widows,
  or any "fix my PDF" or "make the paper look right" request. Be proactive — even
  if the user just says "compile my paper" or "check the PDF" or "polish the paper",
  this skill applies. Also trigger on requests like "paper is ready for submission",
  "review the PDF", or "make this LaTeX render cleanly."
---

# Paper Visual Correctness Polish

## Goal

Take a LaTeX paper directory or `.tex` file and produce a visually correct, clean PDF
free of rendering bugs, bad breaks, and style inconsistencies. Every fix should be
verifiable by re-compiling and visually inspecting the output.

## Workflow

Follow these steps in order. Do not skip steps — each catches different classes of bugs.

### Step 1: Locate and understand the project

1. Find the LaTeX root file (`main.tex` or similar). It `\input`s or `\include`s sections.
2. Identify the bibliography system: `.bib` file(s), `bibliographystyle`, `bibliography` command.
3. Note the document class and key packages from the preamble. **Check if the style class numbers sections** — e.g., AAAI/NeurIPS/ICLR styles use unnumbered sections, so `\ref{sec:method}` will resolve to empty (`(Sec. )` in text). Use "Section name" text instead of `\ref{sec:...}` for section references when the style doesn't number them.
4. Check for a compile script (`compile.sh`, `Makefile`, `latexmkrc`). Use it if available.

### Step 2: Clean compile

```bash
cd <paper-dir>
rm -f *.aux *.bbl *.blg *.log *.out *.toc *.lot *.lof
pdflatex -interaction=nonstopmode main.tex
bibtex main
pdflatex -interaction=nonstopmode main.tex
pdflatex -interaction=nonstopmode main.tex
```

Run **3 passes after bibtex**. Many warnings (undefined citations, undefined references)
are expected on pass 1 only. Pass 3 should resolve them. If `cleveref` is used, a 4th
pass may be needed after label edits.

### Step 3: Scan the compile log for real errors

Run after the clean compile:
```bash
grep -n 'LaTeX Error' main.log
grep -n '^!' main.log
```

Common errors and fixes:

| Error | Cause & Fix |
|---|---|
| `Something's wrong -- perhaps a missing \item` | Check `algorithmic`, `description`, `enumerate` for bad nesting |
| `Undefined control sequence \`cmd | Missing package or undefined macro — add `\newcommand` or `\usepackage` |
| `File 'something.pdf' not found` | Figure missing or wrong relative path in `includegraphics` |
| `Package cleveref Warning: label(s) may have changed` | Run a 4th pdflatex pass |
| `\ELSE \COMMENT{...}` in algorithmic | `\COMMENT` cannot follow `\ELSE` on same line — move to separate `\STATE` |
| `Package biblatex Warning: 'section/bibliography' title changed` | Run `biber`/`bibtex` again, then two pdflatex passes |
| `LaTeX Warning: Reference 'label' on page N on input line M undefined` | Missing `\label{label}` or typo in the label name |

### Step 4: Fix rendering and typography issues

Check for underfull/overfull boxes:
```bash
grep -c 'Overfull' main.log        # count overfull warnings
grep -c 'Underfull' main.log       # count underfull warnings
```

Prioritize fixes:
1. **Overfull \hbox (> 5pt)** — content exceeds margin. Fix: shorten column headers in tables, narrow long words, add `\hyphenation{word}`, or use tighter column spacing with `@{}`.
2. **Underfull \hbox (badness > 10000)** — line too sparse. Fix: reword nearby sentences to give TeX more words for line-breaking, or add soft hyphens.
3. **Overfull \hbox in tables** — Most common. Narrow column headers, switch `l`/`r` to `c` columns, or use `@{}` spacing. Table rows are the #1 cause of rendering warnings.
4. **Underfull \vbox** — page has too little or too much content causing gap. Fix: adjust content flow, move section breaks, or use `\enlargethispage{<len>}`.

Note: Underfull `\hbox` warnings below ~4500 badness are cosmetic and often inherent to narrow two-column layouts. Focus energy on warnings above 10000 and on overfulls.

### Step 5: Visual verification (convert PDF to images, read them)

This is the **critical step**. Compile errors can be zero but the PDF still has bugs
like `\ref{...}` showing literally, `[?]` citations, broken figure captions, etc.

```bash
rm -f main_page-*.png
pdftoppm -png -r 150 main.pdf main_page-
```

Then **read each image file** to visually inspect. Look for:
- `[?]` in citations (bibtex didn't resolve — check `.bib` syntax, recompile with 3+ passes)
- `(Section ?)` or empty `(Sec. )` or literal `\ref{...}` / `\cref{...}` text
- Missing figures where `\includegraphics` is called (check `graphicspath`, file exists)
- Verbatim blocks overflowing margin (wrap in `lstlisting` with `breaklines=true` or shrink with `\footnotesize`)
- Empty last page with just whitespace (rearrange content, or use `\clearpage` strategically)
- Bad table formatting: text wrapping mid-cell, overfull rows, misaligned columns
- Em-dashes/en-dashes rendering as literal text (need `textcomp` package: `\usepackage{textcomp}`)

**Tip:** You can also use the Read tool directly on the `.pdf` to get page-level content,
but converting to PNG is superior for catching visual/layout bugs that appear in the
rendered PDF but not in the `.log`.

### Step 6: Fix found issues, recompile, re-verify

For each bug found:
1. Edit the relevant `.tex` file
2. Recompile (full 3-pass bibtex cycle)
3. Re-render images for affected pages only (or all pages for the final pass)
4. Re-read images to confirm fix

Repeat until all pages are clean.

### Step 7: Final polish

After all bugs are fixed:
- Check that citation styles are consistent (numeric vs. author-year)
- Ensure all `\label{}` and `\ref{}` pairs exist and resolve
- Verify figure/table numbering is sequential
- Check that the title, author, and abstract look correct for the target venue
- Remove unnecessary packages from preamble
- Ensure `\bibliographystyle` matches the conference/journal format

### Step 8: Cleanup

**Remove all temporary artifacts** generated during the polish cycle:

```bash
cd <paper-dir>
rm -f main_page-*.png          # temporal page images from Step 5
rm -f *.aux *.bbl *.blg *.log *.out *.toc *.lot *.lof *.synctex.gz  # LaTeX intermediates
```

The only files that should remain are the source `.tex` / `.bib` files and the final `.pdf`.

## Common Patterns

### Citation `[?]` bugs
```bash
# Diagnose: check bibtex actually ran and produced output
cat main.bbl | head -5
# Should show \begin{thebibliography} not "I couldn't open file"

# Fix: the .bib file has syntax errors (trailing comma, missing closing brace)
# Or the wrong database file is referenced in \bibliography{...}
# Or a bib entry is missing required fields (e.g., 'journal')
```

### Empty `\ref{sec:...}` with unnumbered section styles (AAAI gotcha)

AAAI, NeurIPS, ICLR and other conference styles do not number sections. Using
`\ref{sec:method}` in "See Section~\ref{sec:method}" will produce "See Section " with
nothing after it. Fix by using textual references:

```latex
% Bad (unnumbered style):
See Section~\ref{sec:method} for details.
% → renders as "See Section  for details."

% Good:
See the Method section for details.
```

### Cleveref doesn't resolve
```latex
% Add to preamble:
\usepackage{textcomp}    % for \textendash, \textemdash if needed
\usepackage[capitalize,noabbrev]{cleveref}

% After editing any \label or \cref, run pdflatex a 4th time
```

### Verbatim overflows
```latex
% Bad:
\begin{verbatim}
very-long-command-line-that-exceeds-the-text-width-and-overflows
\end{verbatim}

% Good option 1: use listings with breaklines
\usepackage{listings}
\begin{lstlisting}[breaklines=true, basicstyle=\ttfamily\footnotesize]
very-long-command-line-that-wraps-properly
\end{lstlisting}

% Good option 2: shrink font locally
\bgroup\footnotesize
\begin{verbatim}
shorter-looking-command
\end{verbatim}
\egroup
```

### Table row too wide (overfull hbox) — the #1 issue

Tables cause the majority of overfull warnings. The fix is usually narrowing content:

```latex
% Common: column headers are too verbose. Shorten them.
% Before: "Token-level" / "Chunk-level" / "Matrix state"
% After:  "Token" / "Chunk" / "Matrix"

% Use compact column spacing and centered columns:
% Instead of: l r@{\hspace{2pt}}l r@{\hspace{2pt}}l r@{\hspace{2pt}}l
% Use:       l@{\hspace{3pt}}c@{\hspace{3pt}}c@{\hspace{3pt}}c
```

### Missing figure
```latex
% Check the figure file exists at the right relative path
ls -la figures/training_curve.pdf

% Check graphicspath in preamble matches
% \graphicspath{{figures/}} means \includegraphics{training_curve}
% NOT \includegraphics{figures/training_curve}
```

## What not to fix

- Do not alter the paper's scientific arguments or results
- Do not rewrite substantial prose — only fix typos, minor grammar, and formatting
- Do not change equation numbering strategy (numbered vs. unnumbered)
- Do not modify `.bib` entries beyond fixing obvious syntax errors (missing fields, trailing commas)
- Do not introduce new figures or tables

## Quick Checklist

- [ ] Zero `LaTeX Error` in `.log`
- [ ] All citations resolve (no `[?]` in PDF)
- [ ] All refs resolve (no `??`, no `(Sec. )`, no literal `\ref{}`)
- [ ] No overfull hboxes > 5pt
- [ ] No underfull hboxes > 10000 badness
- [ ] Figures render at expected positions
- [ ] Tables have no misaligned or overflowing columns
- [ ] No empty orphan pages
- [ ] Page count matches venue limit
- [ ] Temporal artifacts cleaned up (no `*.png`, no `.aux`, no `.bbl`, etc.)
