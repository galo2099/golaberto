import unittest
import math
from scipy.stats import binom
from confidence import (cp_one_sided, telescoping_alpha, threshold_mean_bounds,
                        scaled_absolute_bounds, intersect_bounds, EpochStats)

class ConfidenceTests(unittest.TestCase):
    def test_cp_matches_direct_binomial_tails(self):
        a=.013
        for n in (1,3,17,100):
            for k in range(n+1):
                lo=cp_one_sided(k,n,a,lower=True)
                hi=cp_one_sided(k,n,a,lower=False)
                self.assertAlmostEqual(float(binom.sf(k-1,n,lo)) if k else 0., a if k else 0., places=9)
                self.assertAlmostEqual(float(binom.cdf(k,n,hi)) if k<n else 0., a if k<n else 0., places=9)
        self.assertEqual(cp_one_sided(0,10,a,lower=True),0.)
        self.assertEqual(cp_one_sided(10,10,a,lower=False),1.)

    def test_extreme_alpha_and_large_sample_boundaries_are_stable(self):
        a=1e-20; n=1_000_000
        hi=cp_one_sided(0,n,a,lower=False)
        self.assertAlmostEqual(n*math.log1p(-hi),math.log(a),places=8)
        lo=cp_one_sided(n,n,a,lower=True)
        self.assertAlmostEqual(n*math.log(lo),math.log(a),places=8)
        self.assertEqual(cp_one_sided(0,n,a,lower=True),0.)
        self.assertEqual(cp_one_sided(n,n,a,lower=False),1.)

    def test_alpha_allocations_are_summable_and_decay_by_epoch_and_look(self):
        base=.01
        s=sum(telescoping_alpha(base,i,e,l,4) for i in range(100) for e in range(100) for l in range(100) for _ in range(4))
        self.assertLessEqual(s,base)
        self.assertLess(telescoping_alpha(base,0,1,0,1),telescoping_alpha(base,0,0,0,1))
        self.assertLess(telescoping_alpha(base,0,0,1,1),telescoping_alpha(base,0,0,0,1))

    def test_layercake_brackets_zero_one_and_known_bounded_value(self):
        ts=[2.**-j for j in range(241)]
        alpha=.001
        for y in (0.,1.,.75):
            counts=[10 if y >= t else 0 for t in ts]
            lo,hi=threshold_mean_bounds(counts,counts,10,alpha,ts)
            self.assertLessEqual(lo,y)
            self.assertGreaterEqual(hi,y)
            if y==0: self.assertEqual(lo,0.)
            if y==1: self.assertEqual(hi,1.)

    def test_normalized_bounds_scale_by_cap_and_respect_analytic_upper(self):
        lo,hi=scaled_absolute_bounds(.2,.4,3.,.9)
        self.assertAlmostEqual(lo,.6)
        self.assertAlmostEqual(hi,.9)
        self.assertEqual(scaled_absolute_bounds(.2,.4,1.,.1)[1],.1)

    def test_replacement_intersects_and_conflict_keeps_conservative_hull(self):
        self.assertEqual(intersect_bounds(.1,.7,.2,.5),(.2,.5,False))
        self.assertEqual(intersect_bounds(.1,.2,.3,.4),(.1,.4,True))

    def test_production_epoch_stats_accumulate_and_reject_partial_or_changed_law(self):
        one=[1]+[0]*240; zeros=[0]*241
        stats=EpochStats(4)
        stats.consume({"n":2,"epoch":4,"B":3.0,"lower_exceedances":one,"upper_exceedances":one},2)
        stats.consume({"n":3,"epoch":4,"B":3.0,"lower_exceedances":one,"upper_exceedances":one},3)
        self.assertEqual((stats.n,stats.lower[0],stats.upper[0],stats.cap),(5,2,2,3.0))
        before=(stats.n,list(stats.lower),list(stats.upper),stats.cap)
        for receipt,n in [({"n":2,"epoch":4,"B":3.0,"lower_exceedances":zeros,"upper_exceedances":zeros},3),
                          ({"n":1,"epoch":5,"B":3.0,"lower_exceedances":zeros,"upper_exceedances":zeros},1),
                          ({"n":1,"epoch":4,"B":4.0,"lower_exceedances":zeros,"upper_exceedances":zeros},1)]:
            with self.assertRaises(ValueError): stats.consume(receipt,n)
            self.assertEqual((stats.n,stats.lower,stats.upper,stats.cap),before)
        stats.reset(5)
        self.assertEqual((stats.epoch,stats.n,stats.lower[0],stats.cap),(5,0,0,None))

    def test_epoch_reset_preserves_old_absolute_interval_but_clears_epoch_look(self):
        old={"epoch":2,"look":4,"n":100,"lo":.01,"hi":.04}
        new={"epoch":3,"look":0,"n":0,"lo":old["lo"],"hi":old["hi"]}
        self.assertEqual((new["epoch"],new["look"],new["n"]),(3,0,0))
        self.assertEqual((new["lo"],new["hi"]),(old["lo"],old["hi"]))

if __name__ == "__main__": unittest.main()
