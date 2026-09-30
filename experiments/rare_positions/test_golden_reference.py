import unittest

from build_golden_reference import probability_reference
from compare_golden_reference import compare


class GoldenReferenceTest(unittest.TestCase):
    def test_independent_agreement_and_provisional_values(self):
        self.assertEqual(probability_reference([]), (None, None, None, 0))
        sample = lambda p, accepted=True: {"p": p, "accepted": accepted}
        status, _, _, count = probability_reference([sample(1e-20), sample(2e-20)])
        self.assertEqual((status, count), ("reference_is", 2))
        status, _, _, count = probability_reference([sample(1e-20), sample(1e-17)])
        self.assertEqual((status, count), ("provisional_is", 2))
        status, _, _, count = probability_reference([sample(1e-20), sample(0, False)])
        self.assertEqual((status, count), ("provisional_is", 1))

    def test_compare_excludes_unknowns_and_reports_missing_tails(self):
        def cell(position, status, p, scorable):
            return {"team": 1, "position": position, "status": status,
                    "probability": p, "scorable": scorable}
        case = {"case": "toy", "cells": [
            cell(1, "reference_is", 1e-20, True),
            cell(2, "impossible", 0, True),
            cell(3, "undecided", None, False),
            cell(4, "provisional_is", 1e-25, False),
        ]}
        matrix = {"1": {str(i): {"probability": p} for i, p in enumerate([0, 1e-10, 0, 0])}}
        report = compare(case, matrix)
        self.assertEqual(report["reference_positive_cells"], 1)
        self.assertEqual(report["covered"], 0)
        self.assertEqual(report["missing"][0]["position"], 1)
        self.assertEqual(report["impossible_positive_errors"][0]["position"], 2)
        self.assertEqual(report["excluded_cells"], 2)


if __name__ == "__main__":
    unittest.main()
