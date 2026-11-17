import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = (
    ROOT
    / "packages/form-specs/schema/macos-candidate-runtime-observation-v1.schema.json"
)


class MacosCandidateRuntimeObservationSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))

    def test_contract_is_permanently_non_promotional_and_exact(self):
        properties = self.schema["properties"]
        self.assertEqual(properties["schema_version"], {"const": 1})
        self.assertEqual(
            properties["scope"],
            {"const": "macos_candidate_runtime_observation"},
        )
        self.assertEqual(properties["promotion_eligible"], {"const": False})
        self.assertEqual(properties["trusted_producer"], {"const": False})
        self.assertEqual(properties["form_code"], {"const": "2551Q"})
        self.assertEqual(properties["form_revision"], {"const": "2018"})

    def test_every_object_branch_is_closed(self):
        def walk(value):
            if isinstance(value, dict):
                if value.get("type") == "object":
                    self.assertIs(
                        value.get("additionalProperties"),
                        False,
                        msg=f"open object branch: {value}",
                    )
                for child in value.values():
                    walk(child)
            elif isinstance(value, list):
                for child in value:
                    walk(child)

        walk(self.schema)

    def test_schema_has_no_path_or_taxpayer_identity_fields(self):
        forbidden = {
            "path",
            "destination_path",
            "envelope_json",
            "taxpayer",
            "tin",
            "name",
            "address",
            "email",
            "phone",
        }

        def property_names(value):
            names = set()
            if isinstance(value, dict):
                properties = value.get("properties")
                if isinstance(properties, dict):
                    names.update(properties)
                for child in value.values():
                    names.update(property_names(child))
            elif isinstance(value, list):
                for child in value:
                    names.update(property_names(child))
            return names

        self.assertTrue(forbidden.isdisjoint(property_names(self.schema)))


if __name__ == "__main__":
    unittest.main()
