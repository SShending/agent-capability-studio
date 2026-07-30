import { invoke } from "@tauri-apps/api/core";

export const desktop = {
  listSkills: () => invoke("list_skills"),
  getSkill: (id) => invoke("get_skill", { id }),
  auditDraft: (id, markdown) => invoke("audit_draft", { id, markdown }),
  saveDraft: (id, markdown, expectedHash) => invoke("save_draft", { id, markdown, expectedHash }),
  previewNewSkill: (markdown) => invoke("preview_new_skill", { markdown }),
  createSkill: (markdown, expectedDraftHash) => invoke("create_skill", { markdown, expectedDraftHash })
};
