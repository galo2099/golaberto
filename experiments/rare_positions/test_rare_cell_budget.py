import json
from pathlib import Path
import tempfile
import unittest

from summarize_rare_cell_budget import summarize


class RareBudgetTest(unittest.TestCase):
    def fixture(self, directory, changed_hits=False):
        def export(values):
            return {'rare_position_estimates': {'1': {str(i): e for i, e in enumerate(values)}}}
        old = [{'hits': 0, 'samples': 100000, 'probability': 1e-8},
               {'hits': 0, 'samples': 100000, 'probability': 0},
               {'hits': 10, 'samples': 100000, 'probability': .1}]
        new = [dict(e) for e in old]
        new[1]['probability'] = 1e-20
        if changed_hits:
            new[1]['hits'] = 1
        for side, values in [('baseline', old), ('candidate', new)]:
            (directory / f'group-1-x-808-{side}.json').write_text(json.dumps(export(values)))
        (directory / 'summary.json').write_text(json.dumps({'pairs': [{'input': 'group-1-x.json',
            'seed': 808, 'timing': {s: {'http_ms': 10, 'cpu_ms': 30} for s in ['baseline', 'candidate']}}]}))

    def test_gain_uses_recovered_rare_cells_instead_of_all_positive_cells(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            self.fixture(directory)
            row = summarize(directory)['groups']['group-1-x.json']
            self.assertEqual(row['baseline_rare_cell_runs'], 1)
            self.assertEqual(row['permitted_work_increase_pct'], 100)
            self.assertEqual(row['distinct_gained_cells'], [(1, 2)])

    def test_different_mc_observations_cannot_count_as_paired_gain(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            self.fixture(directory, changed_hits=True)
            with self.assertRaisesRegex(AssertionError, 'Initial MC differs'):
                summarize(directory)

    def test_lost_rare_cells_offset_gains_before_granting_work(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            self.fixture(directory)
            path = directory / 'group-1-x-808-candidate.json'
            export = json.loads(path.read_text())
            export['rare_position_estimates']['1']['0']['probability'] = 0
            path.write_text(json.dumps(export))
            row = summarize(directory)['groups']['group-1-x.json']
            self.assertEqual(row['gain_cell_runs'], 1)
            self.assertEqual(row['loss_cell_runs'], 1)
            self.assertEqual(row['permitted_work_increase_pct'], 0)
            self.assertEqual(row['distinct_lost_cells'], [(1, 1)])


if __name__ == '__main__':
    unittest.main()
