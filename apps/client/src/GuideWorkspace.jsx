// @amadeus-header
// summary: Renders the searchable bilingual user guide and developer-reference links.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - fn: GuideWorkspace
// uses:
// - module: apps/client/src/guideCatalog.js
// - module: apps/client/src/guideContent.js
// - module: apps/client/src/MarkdownContent.jsx
// - module: apps/client/src/panelResize.js
// - library: Phosphor Icons
// invariants:
// - User manuscripts render offline from bundled Markdown.
// - Search and chapter navigation remain keyboard accessible.
// side_effects:
// - Opens developer references in the default browser.
// tests:
// - cmd: npm test
// - cmd: npm run build
// @end-amadeus-header

import { useEffect, useMemo, useState } from "react";
import {
  ArrowSquareOut,
  BookOpenText,
  BracketsCurly,
  ChatCircleText,
  GearSix,
  MagnifyingGlass,
  RocketLaunch,
  TreeStructure,
  Wrench,
} from "@phosphor-icons/react";

import { DEVELOPER_REFERENCES, GUIDE_CHAPTERS, guideChapterLabel } from "./guideCatalog";
import { GUIDE_CONTENT } from "./guideContent";
import { MarkdownContent } from "./MarkdownContent";
import { useResizablePanel } from "./panelResize";

const CHAPTER_ICONS = {
  start: RocketLaunch,
  conversation: ChatCircleText,
  agents: TreeStructure,
  tools: Wrench,
  commands: BracketsCurly,
  settings: GearSix,
};

const REPOSITORY_URL = "https://github.com/xxraincandyxx/Amadeus/blob/master";

export function GuideWorkspace({ language, initialChapter, onChapterChange, t }) {
  const navigationResize = useResizablePanel({
    storageKey: "amadeus.guideSidebarWidth",
    defaultWidth: 292,
    minimum: 220,
    maximum: 420,
  });
  const locale = language === "zh-CN" ? "zh-CN" : "en";
  const [activeChapter, setActiveChapter] = useState(initialChapter || GUIDE_CHAPTERS[0].id);
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (initialChapter && GUIDE_CHAPTERS.some((chapter) => chapter.id === initialChapter)) {
      setActiveChapter(initialChapter);
    }
  }, [initialChapter]);

  const filteredChapters = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase(locale);
    if (!normalizedQuery) return GUIDE_CHAPTERS;
    return GUIDE_CHAPTERS.filter((chapter) => {
      const label = guideChapterLabel(chapter, locale);
      const content = GUIDE_CONTENT[locale][chapter.id];
      return `${label.title} ${label.summary} ${content}`.toLocaleLowerCase(locale).includes(normalizedQuery);
    });
  }, [locale, query]);

  const selectChapter = (chapterId) => {
    setActiveChapter(chapterId);
    setQuery("");
    onChapterChange?.(chapterId);
  };

  const content = GUIDE_CONTENT[locale][activeChapter] || GUIDE_CONTENT.en[activeChapter];

  return (
    <section className="guide-workspace" aria-labelledby="guide-workspace-title" ref={navigationResize.containerRef} style={navigationResize.containerStyle}>
      <aside className="guide-navigation" aria-label={t("Guide chapters")}>
        <div className="guide-navigation-heading">
          <BookOpenText />
          <div><strong id="guide-workspace-title">{t("Guide")}</strong><span>{t("Product handbook")}</span></div>
        </div>
        <label className="guide-search" htmlFor="guide-search-input">
          <MagnifyingGlass aria-hidden="true" />
          <input
            id="guide-search-input"
            type="search"
            aria-label={t("Search guide")}
            value={query}
            placeholder={t("Search guide")}
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <nav className="guide-chapter-list">
          {filteredChapters.map((chapter) => {
            const Icon = CHAPTER_ICONS[chapter.icon] || BookOpenText;
            const label = guideChapterLabel(chapter, locale);
            return (
              <button
                key={chapter.id}
                className={chapter.id === activeChapter ? "active" : ""}
                type="button"
                onClick={() => selectChapter(chapter.id)}
              >
                <Icon />
                <span><strong>{label.title}</strong><small>{label.summary}</small></span>
              </button>
            );
          })}
          {!filteredChapters.length && <div className="guide-search-empty">{t("No guide results")}</div>}
        </nav>
        <div className="guide-reference-block">
          <span>{t("Developer reference")}</span>
          {DEVELOPER_REFERENCES.map((reference) => (
            <a key={reference.id} href={`${REPOSITORY_URL}/${reference.path}`} target="_blank" rel="noreferrer">
              {t(reference.label)}<ArrowSquareOut />
            </a>
          ))}
        </div>
      </aside>
      <div className="resize-handle guide-sidebar-resize" aria-label={t("Resize guide sidebar")} title={t("Drag to resize sidebar")} {...navigationResize.handleProps} />
      <article className="guide-document" key={`${locale}-${activeChapter}`}>
        <div className="guide-document-inner"><MarkdownContent text={content} /></div>
      </article>
    </section>
  );
}
