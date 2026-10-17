import unittest
from scheduler import next_plan_request, stage_deadline, stage_timeout_ms

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

    def test_epoch_look_reset_does_not_shrink_requested_capacity_or_k(self):
        plan={"k":8,"states":8000,"complete":False}
        self.assertEqual(next_plan_request(plan,8,16000,12,32000,0,True),(8,32000))

    def test_gd_stage_has_20_second_allowance_and_shares_cleanup_reserve(self):
        self.assertEqual(stage_deadline(180.0,20.0,20.0,10.0),40.0)
        self.assertEqual(stage_deadline(25.0,20.0,20.0,10.0),15.0)
        self.assertEqual(stage_timeout_ms(40.0,39.7,500),299)

if __name__ == "__main__": unittest.main()
