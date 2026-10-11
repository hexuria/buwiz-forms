"""Print label/item_number per field_key from rules/forms/<layout>/fields.json. Usage: fields_view.py <layout-dir-name>"""
import json, sys
d = json.load(open(f'rules/forms/{sys.argv[1]}/fields.json'))
for f in d['fields']:
    print(f"{f['field_key']} | item {f.get('item_number')} | p{f.get('page')} | {f.get('control_kind')} | {f.get('label')}")
