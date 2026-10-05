"""Fast in-memory checks for CPU attribution arm-to-binary metadata."""
import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("analyze_cpu_attribution.py")
SPEC = importlib.util.spec_from_file_location("cpu_attribution_analyzer", SCRIPT)
ANALYZER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ANALYZER)


class BinaryArmMetadataTest(unittest.TestCase):
    def setUp(self):
        self.mapping = {
            "production-baseline": {"path": "/frozen/production", "sha256": "a" * 64},
            "frozen-r60-flag-off": {"path": "/frozen/r60", "sha256": "b" * 64},
            "diagnostic-r60-flag-off": {"path": "/diagnostic/r60", "sha256": "c" * 64},
            "diagnostic-r60-family-on": {"path": "/diagnostic/r60", "sha256": "c" * 64},
            "frozen-r60-family-on": {"path": "/frozen/r60", "sha256": "b" * 64},
        }

    def test_all_five_arms_resolve_to_the_expected_binary_roles(self):
        self.assertEqual(ANALYZER.validate_binary_mapping(self.mapping), self.mapping)

    def test_missing_arm_metadata_is_rejected(self):
        del self.mapping["frozen-r60-family-on"]
        with self.assertRaisesRegex(ValueError, "exactly one entry"):
            ANALYZER.validate_binary_mapping(self.mapping)

    def test_flag_variants_must_share_their_binary(self):
        self.mapping["frozen-r60-family-on"]["path"] = "/other/frozen/r60"
        with self.assertRaisesRegex(ValueError, "same binary"):
            ANALYZER.validate_binary_mapping(self.mapping)


if __name__ == "__main__":
    unittest.main()
