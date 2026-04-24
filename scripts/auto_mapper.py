import os
import json
import sys
from PIL import Image, ImageDraw, ImageFont
from google import genai
from google.genai import types

# Typst target dimensions for US-Legal
TYPST_W = 612
TYPST_H = 1008
BASELINE_OFFSET_Y = 2  # Typographic baseline offset

def main():
    if len(sys.argv) < 3:
        print("Usage: python auto_mapper.py <schema.json> <blank_image.png>")
        sys.exit(1)

    schema_path = sys.argv[1]
    image_path = sys.argv[2]
    
    # 1. Setup Gemini
    api_key = os.environ.get("GEMINI_API_KEY")
    if not api_key:
        print("Error: GEMINI_API_KEY not found in environment")
        sys.exit(1)
        
    client = genai.Client(api_key=api_key)

    # 2. Extract Fields and Labels from Schema
    with open(schema_path, 'r') as f:
        schema = json.load(f)

    fields_to_map = {}
    for page in schema.get('pages', []):
        for sec in page.get('sections', []):
            for field in sec.get('fields', []):
                # We store the label to give semantic context to the AI
                fields_to_map[field['name']] = field.get('label', field['name'])

    print(f"Loaded {len(fields_to_map)} fields to map.")

    # 3. Load Image
    img = Image.open(image_path)
    
    # 4. Prompt Gemini
    prompt = f"""
You are an expert OCR and spatial reasoning AI. I am providing you with an image of a blank Philippine tax form printed directly from the eBIRForms software.
I need you to find the exact bounding boxes for specific fields on this form.

CRITICAL INSTRUCTIONS FOR eBIRFORMS HTML LAYOUT:
1. This image is printed from an HTML application. The input boxes are often inside very thin HTML table cells. You MUST strictly separate adjacent column boundaries. Do not merge adjacent cells into one box.
2. IMPORTANT PLACEHOLDER RULE: The software often pre-fills empty financial fields with `0.00` and date fields with formats like `MM/DD/YYYY` or `12/31/2026`. You MUST IGNORE these placeholder texts. Draw the bounding box for the entire empty cell/input area, treating the `0.00` as if the box were blank.
3. For checkboxes, find the box itself.
4. You must find all {len(fields_to_map)} fields. Do not skip any.

Return the bounding boxes as a raw JSON object (no markdown code blocks, just raw JSON).
The keys should be the exact field ID provided below.
The values should be a list of 4 integers: [ymin, xmin, ymax, xmax] using a normalized 0 to 1000 scale.

Fields to find (ID -> Human Readable Label):
{json.dumps(fields_to_map, indent=2)}

Remember: return ONLY a valid JSON object.
"""

    print("Requesting bounding boxes from Gemini 1.5 Pro...")
    
    response = client.models.generate_content(
        model='gemini-3.1-flash-image-preview',
        contents=[img, prompt],
        config=types.GenerateContentConfig(
            response_mime_type="application/json",
            temperature=0.0
        )
    )
    
    try:
        text = response.text.strip()
        boxes = json.loads(text)
    except Exception as e:
        print("Failed to parse Gemini response as JSON:")
        print(response.text)
        sys.exit(1)

    print(f"Successfully received {len(boxes)} bounding boxes!")

    # 5. Transform to Typst scale and apply baseline offsets
    typst_mapping = {}
    img_w, img_h = img.size
    
    # We will also draw the debug overlay on the original image
    draw = ImageDraw.Draw(img)

    for key, box in boxes.items():
        if len(box) != 4: continue
        ymin, xmin, ymax, xmax = box
        
        # Draw on debug image (scale 0-1000 back to physical image pixels)
        px_xmin = int((xmin / 1000.0) * img_w)
        px_ymin = int((ymin / 1000.0) * img_h)
        px_xmax = int((xmax / 1000.0) * img_w)
        px_ymax = int((ymax / 1000.0) * img_h)
        
        draw.rectangle([px_xmin, px_ymin, px_xmax, px_ymax], outline="red", width=3)
        draw.text((px_xmin, max(0, px_ymin - 10)), key.split(':')[-1], fill="blue")

        # Transform to Typst points
        # X is straightforward scaling
        typst_x = (xmin / 1000.0) * TYPST_W
        typst_w = ((xmax - xmin) / 1000.0) * TYPST_W
        
        # Y uses ymin + baseline offset
        typst_y = (ymin / 1000.0) * TYPST_H + BASELINE_OFFSET_Y
        typst_h = ((ymax - ymin) / 1000.0) * TYPST_H

        typst_mapping[key] = {
            "page": 1,
            "x": round(typst_x, 1),
            "y": round(typst_y, 1),
            "w": round(typst_w, 1),
            "h": round(typst_h, 1)
        }

    # 6. Save Outputs
    output_dir = "templates"
    os.makedirs(output_dir, exist_ok=True)
    
    mapping_path = os.path.join(output_dir, "mapping.json")
    with open(mapping_path, 'w') as f:
        json.dump(typst_mapping, f, indent=2)
        
    debug_path = os.path.join(output_dir, "debug_overlay.png")
    img.save(debug_path)

    print(f"Mapping saved to {mapping_path}")
    print(f"Debug overlay saved to {debug_path}")

if __name__ == "__main__":
    main()
