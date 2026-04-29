# eBIRForms Digitization & Field Mapping Use Cases

This document outlines the different architectural approaches and user experiences we can support for digitizing new BIR tax forms. It serves as a guide for planning future features for the `PdfLayoutEditorView`.

## 1. Manual Field Definition (The Traditional Approach)
**Workflow:**
1. User uploads a blank PDF of the tax form.
2. The user enters "Draw Mode", drags a box over an input area on the PDF.
3. A popup or sidebar prompt immediately asks the user to name the field (e.g., `frm2551Qv2018:qtr_1`).
4. The user repeats this for every single field on the form.

**Pros:** 
- Simple, straightforward implementation.
- Immediate 1-to-1 mapping.
**Cons:**
- Highly tedious and prone to typos.
- Requires the user to constantly switch between mouse (drawing) and keyboard (typing field names).

## 2. Manifest-Driven Mapping (The "Box First, Map Later" Approach)
**Workflow:**
1. User uploads a blank PDF.
2. The user rapidly draws bounding boxes over all input areas without stopping to name them. The system assigns them random or sequential placeholder IDs (e.g., `box_1`, `box_2`).
3. The user uploads a "Manifest" (a JSON or text file containing the definitive list of expected field keys extracted from the legacy application).
4. **Mapping Phase:** The system lists all unassigned keys from the manifest in the sidebar. The user clicks a key, then clicks a drawn box to link them together (or uses drag-and-drop).

**Pros:**
- Much faster workflow. The user can stay in "mouse mode" to draw all boxes, then stay in "mapping mode" to assign them.
- Prevents typos because field names come strictly from the source-of-truth manifest.
**Cons:**
- Requires a two-step process.
- Requires a separate tool or script to generate the initial JSON manifest.

## 3. AI-Assisted Boundary & Label Detection (The "Smart" Approach)
**Workflow:**
1. User uploads a blank PDF.
2. The system runs an edge-detection or OCR AI model over the PDF to automatically identify input boxes and adjacent text labels.
3. The AI generates a draft `formtype.json` with pre-drawn bounding boxes and highly-probable field keys based on the adjacent text.
4. The user opens the Layout Editor in "Review Mode" to adjust slightly misaligned boxes and correct any misidentified keys.

**Pros:**
- Reduces human effort by 90%.
- Highly scalable for adding dozens of new tax forms quickly.
**Cons:**
- High implementation complexity (requires integrating computer vision, OCR, or multimodal LLM APIs).
- May require a backend service if models are too heavy for local execution.

---

## Next Steps for Implementation
To validate these ideas and choose the best path forward, we should execute the following procedures:
1. **Agent Task 1:** Audit the current `FormType` struct to ensure it can support nullable keys (for the "Box First" approach).
2. **Agent Task 2:** Prototype a simple JSON manifest uploader in the sidebar to test mapping unassigned fields.
3. **Agent Task 3:** Evaluate Rust-native OCR libraries or lightweight multimodal endpoints for the AI-assisted pipeline.
