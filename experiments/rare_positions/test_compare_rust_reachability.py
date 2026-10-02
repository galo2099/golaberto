import unittest

from benchmark_joint_cap_requests import compare
from compare_rust_reachability import states


class HttpReachabilityComparisonTest(unittest.TestCase):
    def test_public_and_internal_reachability_names(self):
        names = ["reachable", "witness", "reachable_by_construction", "undecided",
                 "impossible_by_joint_points"]
        response = {"rare_position_estimates": {"17": {
            str(i): {"probability": 0., "reachability": name}
            for i, name in enumerate(names)}}}
        actual = states(response)
        self.assertEqual([actual[17, i + 1] for i in range(len(names))],
                         ["reachable_zero"] * 3 + ["undecided", "impossible"])

    def test_lost_estimate_does_not_lose_existing_http_proof(self):
        def response(probability, reachability):
            return {"game_importance": {}, "rare_position_estimates": {
                "17": {"12": {"probability": probability, "std_err": 0.,
                              "reachability": reachability}}}}

        report = compare(response(1e-34, "reachable"), response(0., "reachable"))
        self.assertEqual(report["lost"], [{"team": 17, "position": 13}])
        self.assertEqual(report["lost_reachability"], [])
        report = compare(response(0., "reachable"), response(0., "undecided"))
        self.assertEqual(report["lost_reachability"], [{"team": 17, "position": 13}])


if __name__ == "__main__":
    unittest.main()
