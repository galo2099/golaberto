import unittest
from scheduler import next_plan_request, stage_deadline, stage_timeout_ms, sampling_priority, rebuild_request

class SchedulerTests(unittest.TestCase):
    def test_incomplete_plan_doubles_capacity_before_growing_k(self):
        plan={"k":7,"states":1200,"complete":False}
        self.assertEqual(next_plan_request(plan,7,2000,12,16000,4,True),(7,4000))

    def test_only_grows_k_after_complete_plan_and_focused_look(self):
        plan={"k":7,"states":3000,"complete":True}
        self.assertEqual(next_plan_request(plan,7,4000,12,16000,0,True),(7,4000))
        self.assertEqual(next_plan_request(plan,7,4000,12,16000,1,True),(8,4000))

    def test_rejected_attempt_at_capacity_ceiling_can_try_next_k(self):
        plan={"k":7,"states":16000,"complete":False}
        self.assertEqual(next_plan_request(plan,7,16000,12,16000,0,False),(8,16000))
        self.assertEqual(next_plan_request(plan,7,16000,12,16000,0,True),(7,16000))

    def test_rejected_partial_candidate_grows_before_advancing_from_complete_install(self):
        installed={"k":7,"states":1200,"complete":True}
        # Candidate k=8 was rejected, but its table was partial. Continue k=8.
        self.assertEqual(next_plan_request(installed,8,2000,12,16000,1,False,False),(8,4000))
        # Only after reaching the ceiling may a rejected partial candidate yield to k=9.
        self.assertEqual(next_plan_request(installed,8,16000,12,16000,1,False,False),(9,16000))

    def test_epoch_look_reset_does_not_shrink_requested_capacity_or_k(self):
        plan={"k":8,"states":8000,"complete":False}
        self.assertEqual(next_plan_request(plan,8,16000,12,32000,0,True),(8,32000))

    def test_gd_stage_has_20_second_allowance_and_shares_cleanup_reserve(self):
        self.assertEqual(stage_deadline(180.0,20.0,20.0,10.0),40.0)
        self.assertEqual(stage_deadline(25.0,20.0,20.0,10.0),15.0)
        self.assertEqual(stage_timeout_ms(40.0,39.7,500),299)

    def test_sampling_priority_favors_reducible_over_irreducible_width(self):
        n=2_000_000
        lower=[0]*241
        upper=[n if 2.0**-j <= .08 else 0 for j in range(241)]
        irreducible=sampling_priority(0,.08,n,1.,lower,upper,2000,1000)
        other=sampling_priority(0,.08,1000,1.,[0]*241,[0]*241,1000,1000)
        self.assertGreater(other,irreducible)

    def test_sampling_priority_accounts_for_measured_batch_cost(self):
        counts=[0]*241
        cheap=sampling_priority(0,.08,1000,1.,counts,counts,10,1000)
        expensive=sampling_priority(0,.08,1000,1.,counts,counts,100,1000)
        self.assertGreater(cheap,expensive)

    def test_sampling_priority_explores_no_hits_and_handles_empty_epoch(self):
        counts=[0]*241
        no_hits=sampling_priority(0,.03,100,1.,counts,counts,20,1000)
        empty=sampling_priority(0,.03,0,None,counts,counts,0,1000)
        self.assertGreater(no_hits,0)
        self.assertGreater(empty,0)

    def test_rebuild_budget_is_strictly_inside_cleanup_window_and_shared_state_cap(self):
        self.assertEqual(rebuild_request(300000,3_750_000,4_000_000,250_000,500,100,98.2,1),
                         (250000,500,None))
        self.assertEqual(rebuild_request(250000,3_800_000,4_000_000,250_000,500,100,99.2,1),
                         (200000,None,"deadline"))
        self.assertEqual(rebuild_request(1000,4_000_000,4_000_000,250_000,500,100,90,1),
                         (None,None,"state_budget"))

if __name__ == "__main__": unittest.main()
