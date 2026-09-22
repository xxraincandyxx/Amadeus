// @amadeus-header
// summary: Renders the editable right sidebar for a user-customized agent character.
// layer: ui
// status: active
// feature_flags: none
// provides:
// - component: AgentProfilePanel
// uses:
// - package: @phosphor-icons/react
// - module: apps/client/src/agentProfileState.js
// invariants:
// - Profile edits are explicit and do not mutate runtime session metadata.
// - The image placeholder remains visible until a supported local image is selected.
// side_effects:
// - Reads a user-selected image through the browser FileReader API.
// tests:
// - cmd: npm test
// - cmd: npm run build
// @end-amadeus-header

import { Camera, Check, IdentificationCard, Plus, Trash, UserCircle, X } from "@phosphor-icons/react";
import { useRef, useState } from "react";

import { MAX_AGENT_AVATAR_BYTES } from "./agentProfileState";

function TagEditor({ id, label, placeholder, values, onChange, t }) {
  const [value, setValue] = useState("");

  const addTag = () => {
    const next = value.trim();
    if (!next || values.some((tag) => tag.toLocaleLowerCase() === next.toLocaleLowerCase())) return;
    onChange([...values, next]);
    setValue("");
  };

  return (
    <fieldset className="agent-profile-tags">
      <legend>{label}</legend>
      {values.length > 0 && (
        <div className="agent-profile-tag-list">
          {values.map((tag) => (
            <span key={tag}>{tag}<button type="button" onClick={() => onChange(values.filter((item) => item !== tag))} aria-label={t("Remove {tag}", { tag })}><X /></button></span>
          ))}
        </div>
      )}
      <div className="agent-profile-tag-input">
        <input
          id={id}
          value={value}
          maxLength={40}
          placeholder={placeholder}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== "Enter") return;
            event.preventDefault();
            addTag();
          }}
        />
        <button type="button" onClick={addTag} disabled={!value.trim()} aria-label={t("Add {label}", { label: label.toLocaleLowerCase() })}><Plus /></button>
      </div>
    </fieldset>
  );
}

export function AgentProfilePanel({ session, profile, runtime, parentSession, children, onSave, onClose, t }) {
  const [draft, setDraft] = useState(profile);
  const [feedback, setFeedback] = useState("");
  const imageInputRef = useRef(null);

  const update = (field, value) => {
    setDraft((current) => ({ ...current, [field]: value }));
    setFeedback("");
  };

  const selectImage = (event) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    if (!/^image\/(?:png|jpeg|webp)$/i.test(file.type)) {
      setFeedback(t("Image must be PNG, JPEG, or WebP."));
      return;
    }
    if (file.size > MAX_AGENT_AVATAR_BYTES) {
      setFeedback(t("Image must be smaller than 1 MB."));
      return;
    }
    const reader = new FileReader();
    reader.addEventListener("load", () => update("avatarDataUrl", typeof reader.result === "string" ? reader.result : null));
    reader.addEventListener("error", () => setFeedback(t("The image could not be read.")));
    reader.readAsDataURL(file);
  };

  const submit = (event) => {
    event.preventDefault();
    try {
      const saved = onSave(draft);
      setDraft(saved);
      setFeedback(t("Profile saved"));
    } catch {
      setFeedback(t("The profile could not be saved."));
    }
  };

  const runtimeRole = parentSession ? t("Sub-agent") : t("Coordinator");
  return (
    <aside className="details-panel agent-profile-panel" aria-labelledby="agent-profile-title">
      <div className="details-header">
        <span><IdentificationCard /><span><strong id="agent-profile-title">{t("Agent profile")}</strong><small>{t("Character and preferences")}</small></span></span>
        <button type="button" onClick={onClose} aria-label={t("Close")}><X /></button>
      </div>
      <form className="agent-profile-form" onSubmit={submit}>
        <section className="agent-profile-avatar-section" aria-label={t("Profile image")}>
          <div className="agent-profile-avatar">
            {draft.avatarDataUrl ? <img src={draft.avatarDataUrl} alt={draft.displayName} /> : <UserCircle weight="thin" aria-hidden="true" />}
          </div>
          <div>
            <strong>{t("Profile image")}</strong>
            <small>{t("PNG, JPEG, or WebP. Maximum 1 MB.")}</small>
            <div className="agent-profile-avatar-actions">
              <button type="button" onClick={() => imageInputRef.current?.click()}><Camera />{t(draft.avatarDataUrl ? "Replace" : "Upload")}</button>
              {draft.avatarDataUrl && <button type="button" className="danger" onClick={() => update("avatarDataUrl", null)}><Trash />{t("Remove")}</button>}
            </div>
          </div>
          <input ref={imageInputRef} className="visually-hidden" type="file" accept="image/png,image/jpeg,image/webp" onChange={selectImage} />
        </section>

        <section className="agent-profile-section">
          <div className="agent-profile-section-title"><strong>{t("Basic information")}</strong><small>{t("Shown only in this local workspace.")}</small></div>
          <label><span>{t("Display name")}</span><input value={draft.displayName} maxLength={80} onChange={(event) => update("displayName", event.target.value)} placeholder={session.name} required /></label>
          <label><span>{t("Role or title")}</span><input value={draft.role} maxLength={80} onChange={(event) => update("role", event.target.value)} placeholder={runtimeRole} /></label>
          <label><span>{t("About")}</span><textarea value={draft.bio} maxLength={600} rows={4} onChange={(event) => update("bio", event.target.value)} placeholder={t("Describe this agent's character and working style.")} /></label>
        </section>

        <section className="agent-profile-section">
          <div className="agent-profile-section-title"><strong>{t("Preferences")}</strong><small>{t("Capture the character traits you want to remember.")}</small></div>
          <TagEditor id="agent-likes" label={t("Likes")} placeholder={t("Add a like")} values={draft.likes} onChange={(values) => update("likes", values)} t={t} />
          <TagEditor id="agent-dislikes" label={t("Dislikes")} placeholder={t("Add a dislike")} values={draft.dislikes} onChange={(values) => update("dislikes", values)} t={t} />
        </section>

        <details className="agent-profile-runtime">
          <summary>{t("Runtime details")}</summary>
          <dl><div><dt>{t("Role")}</dt><dd>{runtimeRole}</dd></div>{parentSession && <div><dt>{t("Parent agent")}</dt><dd>{parentSession.name}</dd></div>}<div><dt>{t("Child agents")}</dt><dd>{children.length}</dd></div><div><dt>{t("Status")}</dt><dd>{runtime.status === "awaiting_approval" ? t("Needs approval") : t(runtime.status?.replaceAll("_", " ") || "idle")}</dd></div><div><dt>{t("Session ID")}</dt><dd className="mono">{session.id}</dd></div></dl>
        </details>

        <div className="agent-profile-actions">
          <span role="status" className={feedback === t("Profile saved") ? "saved" : "error"}>{feedback && (feedback === t("Profile saved") ? <Check /> : null)}{feedback}</span>
          <button type="button" onClick={() => { setDraft(profile); setFeedback(""); }}>{t("Reset changes")}</button>
          <button className="primary" type="submit">{t("Save profile")}</button>
        </div>
      </form>
    </aside>
  );
}
