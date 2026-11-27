# r2-2551q-recomputed-money-type 3

verdict: `passed`
known_gap: `None`

- **box_text_before_type**: `5.00`
- **box_text**: `9.00`
- **saved_value**: `9.0`
- **replace_vs_append**: `replaces_selection`
- **failure**: `{
  "v": 1,
  "ok": true,
  "recipe": "r2-open-penalties",
  "fingerprint": "fnv1a64:884763e782b43f37",
  "session_reused": true,
  "steps": [
    {
      "id": "s1",
      "ok": true,
      "elapsed_ms": 249
    },
    {
      "id": "s2",
      "ok": true,
      "delivery_status": "delivered_unverified",
      "elapsed_ms": 441
    },
    {
      "id": "s3",
      "ok": true,
      "elapsed_ms": 104
    },
    {
      "id": "s4",
      "ok": true,
      "delivery_status": "delivered_unverified",
      "elapsed_ms": 2028
    },
    {
      "id": "s5",
      "ok": true,
      "elapsed_ms": 97
    },
    {
      "id": "s6",
      "ok": true,
      "delivery_status": "delivered_unverified",
      "elapsed_ms": 443
    },
    {
      "id": "s7",
      "ok": true,
      "elapsed_ms": 96
    }
  ],
  "elapsed_ms": 3461
}`

## steps
- data setup rc=0
- open penalties rc=0
- type rc=0
- eopt_micro_answered=False
