(function () {
  "use strict";

  const CSS = `
:host { display: block; width: 100%; color: var(--text, #e8edf5); font-family: inherit; box-sizing: border-box; }
* { box-sizing: border-box; }
.panel-container { width: 100%; max-width: 920px; margin: 0 auto; display: flex; flex-direction: column; gap: 16px; }
.header-card {
  display: flex; align-items: center; justify-content: space-between; padding: 16px 20px;
  background: var(--surface, rgba(255, 255, 255, 0.035)); border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  border-radius: var(--radius, 12px);
}
.title-wrap { display: flex; align-items: center; gap: 12px; }
.icon-box {
  width: 40px; height: 40px; border-radius: 10px; background: rgba(var(--accent-rgb, 110, 168, 254), 0.15);
  color: var(--accent, #6ea8fe); display: grid; place-items: center; font-size: 20px;
}
.title { font-size: 16px; font-weight: 700; color: var(--text, #e8edf5); }
.subtitle { font-size: 12px; color: var(--text-faint, #96a3b8); margin-top: 2px; }
.badge {
  display: inline-flex; align-items: center; padding: 4px 10px; border-radius: 99px; font-size: 11px;
  font-weight: 600; background: rgba(101, 211, 145, 0.12); color: #65d391; border: 1px solid rgba(101, 211, 145, 0.25);
}
.field-card {
  display: flex; flex-direction: column; gap: 10px; background: var(--surface, rgba(255, 255, 255, 0.035));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.1)); border-radius: var(--radius, 12px); padding: 16px;
}
.label { font-size: 11px; font-weight: 700; color: var(--text-dim, #94a3b8); text-transform: uppercase; letter-spacing: 0.06em; }
.dropzone {
  border: 2px dashed var(--border, rgba(255, 255, 255, 0.2)); border-radius: var(--radius-sm, 8px);
  padding: 24px; text-align: center; cursor: pointer; background: rgba(0, 0, 0, 0.15);
}
.btn-primary {
  width: 100%; padding: 12px; background: var(--accent, #6ea8fe); color: #0b101b; border: none;
  border-radius: var(--radius-sm, 8px); font-weight: 700; font-size: 14px; cursor: pointer;
}
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
`;

  class LocarynVisionOcrPanel extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: "open" });
      this.isAnalyzing = false;
      this.imageLoaded = false;
    }
    connectedCallback() { this.render(); }

    async analyze() {
      this.isAnalyzing = true;
      this.render();
      try {
        const bridge = window.locaryn || window.LocarynPluginAPI;
        if (bridge && bridge.invokeExtensionTool) {
          await bridge.invokeExtensionTool("ocr_extract_text", { image_path: "document.png" });
        }
      } catch (err) {
        alert("Erreur d'analyse OCR: " + err);
      } finally {
        this.isAnalyzing = false;
        this.render();
      }
    }

    render() {
      this.shadowRoot.innerHTML = `
        <style>${CSS}</style>
        <div class="panel-container">
          <div class="header-card">
            <div class="title-wrap">
              <div class="icon-box">👁️</div>
              <div>
                <div class="title">Studio Vision & OCR</div>
                <div class="subtitle">Lecture de documents, scans et détection d'objets via Florence-2 & YOLO</div>
              </div>
            </div>
            <div class="badge">Actif</div>
          </div>

          <div class="field-card">
            <label class="label">Image ou document scanné</label>
            <div class="dropzone">
              <div style="font-size: 24px; margin-bottom: 6px;">📄</div>
              <div style="font-weight: 600;">Glisser un fichier image ou PDF ici</div>
              <div style="font-size: 12px; color: var(--text-dim); margin-top: 4px;">PNG, JPEG, WebP, PDF</div>
            </div>
          </div>

          <button class="btn-primary" id="vo-btn" ${this.isAnalyzing ? "disabled" : ""}>
            ${this.isAnalyzing ? "Analyse en cours..." : "Lancer l'analyse OCR"}
          </button>
        </div>
      `;

      const btn = this.shadowRoot.querySelector("#vo-btn");
      if (btn) btn.addEventListener("click", () => this.analyze());
    }
  }

  if (!customElements.get("locaryn-vision-ocr-panel")) {
    customElements.define("locaryn-vision-ocr-panel", LocarynVisionOcrPanel);
  }
})();
