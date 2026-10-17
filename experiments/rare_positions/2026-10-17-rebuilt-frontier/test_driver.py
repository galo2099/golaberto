import unittest
from types import SimpleNamespace
import contextlib
import io
import json
import sys
import tempfile
from pathlib import Path
from unittest.mock import patch

from confidence import EpochStats
import driver
from driver import accept_regime_proposal, rebuild_after_joint_growth


class FakeEngine:
    def __init__(self, receipt=None, error=None):
        self.receipt=receipt or {}
        self.error=error
        self.calls=[]
    def call(self, **request):
        self.calls.append(request)
        if self.error:
            raise self.error
        return dict(self.receipt)


class MainFlowEngine:
    def __init__(self, binary, request, team, position, deadline):
        self.calls=[]
        self.sample_epochs=[]
        self.sample_seeds=[]
        self.prepares=0
        self.zero_growth=False
        self.zero_rebuild=False
        self.timeout_rebuild=False
    def call(self, **request):
        self.calls.append(dict(request))
        op=request["op"]
        if op=="describe":
            return {"regions":[{"id":1,"upper":.5,"plan":{"states":1000}}],
                    "candidate_slots":1,"uncovered_upper":0.,"target":{}}
        if op=="prepare":
            self.prepares+=1
            if self.prepares==1:
                return {"accepted":True,"complete":False,"epoch":1,"states":1000,
                        "B":1.,"selected_rival_ids":[2],"mode":"joint","setup_ms":3}
            return {"accepted":True,"complete":True,"epoch":2,"states":1500,
                    "B":0. if self.zero_growth else .8,"selected_rival_ids":[2,3],"mode":"joint","setup_ms":4}
        if op=="regime":
            if sum(c["op"]=="regime" for c in self.calls)==1:
                return {"accepted":False,"complete":False,"epoch":1,"setup_ms":5}
            if self.timeout_rebuild:
                raise TimeoutError("protocol deadline")
            return {"accepted":True,"complete":True,"epoch":3,"states":1700,
                    "B":0. if self.zero_rebuild else .6,"selected_rival_ids":[2,3],"mode":"regime","setup_ms":7}
        if op=="sample":
            epoch=1 if not self.sample_epochs else 3
            self.sample_epochs.append(epoch)
            self.sample_seeds.append(request["seed"])
            return {"n":1,"epoch":epoch,"B":.6 if epoch==3 else 1.,
                    "lower_exceedances":[0]*241,"upper_exceedances":[0]*241,
                    "sum_lower":0.,"sum_upper":0.,"positive_draws":0,
                    "sum_lower_sq":0.,"sum_upper_sq":0.,"elapsed_ms":1.,
                    "mode":"regime" if epoch==3 else "joint"}
        return {}
    def close(self):
        self.call(op="quit")


def state(plan=None):
    return {"epoch":4,"look":3,"focused_looks":2,"n":19,
            "lo":.01,"hi":.08,"row":{"upper":.09},
            "stats":EpochStats(4),"setup_ms":0.,"regime_attempts":[],
            "regime_status":"pending","current_plan":plan or {}}


class RebuildFlowTests(unittest.TestCase):
    def setUp(self):
        self.args=SimpleNamespace(regimes=True,regime_rebuilds=True,state_cap=250000,
                                  time_ms=700,cleanup_reserve=10.)
        self.caps={1:1000,2:2000}
        self.totals={"setup_ms":0.,"budget_ms":0.}
        self.plan={"accepted":True,"complete":True,"mode":"joint","B":.7,
                   "epoch":4,"k":5,"states":1000}

    def test_growth_rebuild_resets_epoch_and_installs_new_law(self):
        engine=FakeEngine({"accepted":True,"complete":True,"epoch":5,"B":.4,
                           "states":1600,"selected_rival_ids":[2,3],"setup_ms":12,
                           "mode":"regime","pivot_id":2,"atom_count":9})
        s=state(self.plan)
        s["stats"].consume({"n":1,"epoch":4,"B":.7,"lower_exceedances":[0]*241,
                            "upper_exceedances":[0]*241,"sum_lower":0.,"sum_upper":0.,
                            "positive_draws":0,"sum_lower_sq":0.,"sum_upper_sq":0.},1)
        self.assertEqual(s["stats"].n,1)
        attempt=rebuild_after_joint_growth(engine,1,s,self.caps,self.args,100.,self.totals,
                                           now=lambda:20.)
        self.assertEqual(engine.calls,[{"op":"regime","id":1,"state_cap":250000,"time_ms":700}])
        self.assertEqual(attempt["trigger"],"after_joint_growth")
        self.assertEqual(s["epoch"],5)
        self.assertEqual((s["look"],s["focused_looks"],s["stats"].n,s["n"]),(0,0,0,19))
        self.assertEqual((s["lo"],s["hi"],s["row"]["upper"]),(.01,.08,.09))
        self.assertEqual(s["current_plan"]["mode"],"regime")
        self.assertEqual(s["setup_ms"],12.)
        self.assertEqual(self.totals["setup_ms"],12.)

    def test_rejected_rebuild_preserves_joint_law_and_sampling_continues_there(self):
        engine=FakeEngine({"accepted":False,"complete":True,"epoch":4,"setup_ms":8})
        s=state(self.plan)
        joint=s["current_plan"]
        rebuild_after_joint_growth(engine,1,s,self.caps,self.args,100.,self.totals,now=lambda:20.)
        self.assertIs(s["current_plan"],joint)
        self.assertEqual(s["epoch"],4)
        self.assertEqual(s["regime_status"],"complete_rejected")
        self.assertEqual((s["setup_ms"],self.totals["setup_ms"]),(8.,8.))
        self.assertEqual(s["regime_attempts"][0]["trigger"],"after_joint_growth")

    def test_attempted_once_per_parent_epoch_even_after_rejection(self):
        engine=FakeEngine({"accepted":False,"complete":False,"setup_ms":2})
        s=state(self.plan)
        rebuild_after_joint_growth(engine,1,s,self.caps,self.args,100.,self.totals,now=lambda:20.)
        rebuild_after_joint_growth(engine,1,s,self.caps,self.args,100.,self.totals,now=lambda:20.)
        self.assertEqual(len(engine.calls),1)

    def test_ineligible_plans_do_not_rebuild(self):
        for plan in [None,{**self.plan,"accepted":False},{**self.plan,"complete":False},
                     {**self.plan,"B":0.},{**self.plan,"mode":"regime"}]:
            engine=FakeEngine({"accepted":False})
            s=state(plan)
            self.assertIsNone(rebuild_after_joint_growth(engine,1,s,self.caps,self.args,100.,self.totals,now=lambda:20.))
            self.assertEqual(engine.calls,[])

    def test_deadline_state_cap_disable_and_setup_failure_are_recorded(self):
        s=state(self.plan);e=FakeEngine()
        rebuild_after_joint_growth(e,1,s,self.caps,self.args,20.,self.totals,now=lambda:11.)
        self.assertEqual(s["regime_status"],"skipped_deadline")
        self.assertEqual(e.calls,[])
        s=state(self.plan);caps={1:250000,2:3_750_000};e=FakeEngine()
        rebuild_after_joint_growth(e,1,s,caps,self.args,100.,self.totals,now=lambda:20.)
        self.assertEqual(s["regime_status"],"skipped_no_growth_state_budget")
        self.args.regime_rebuilds=False;s=state(self.plan);e=FakeEngine()
        rebuild_after_joint_growth(e,1,s,self.caps,self.args,100.,self.totals,now=lambda:20.)
        self.assertEqual(e.calls,[])
        self.args.regime_rebuilds=True
        s=state(self.plan);e=FakeEngine(error=RuntimeError("invalid setup"))
        rebuild_after_joint_growth(e,1,s,self.caps,self.args,100.,self.totals,now=lambda:20.)
        self.assertEqual(s["regime_status"],"setup_failure")
        self.assertTrue(s["regime_attempts"][0]["setup_failure"])
        self.assertEqual(s["setup_ms"],0.)

    def test_main_focus_growth_calls_regime_before_sampling_new_epoch(self):
        engines=[]
        def factory(*args):
            e=MainFlowEngine(*args);engines.append(e);return e
        with tempfile.TemporaryDirectory() as tmp:
            binary=Path(tmp)/"fake-engine";binary.write_text("x")
            request=Path(tmp)/"request.json";request.write_text("{}")
            argv=["driver.py","--binary",str(binary),"--request",str(request),
                  "--budget","30","--cleanup-reserve","1","--initial-draws","1",
                  "--max-draws","2","--initial-state-cap","1000","--state-cap","2000",
                  "--time-ms","700","--no-gd-bounds"]
            output=io.StringIO()
            with patch.object(sys,"argv",argv),patch.object(driver,"Protocol",factory),\
                 patch.object(driver.time,"monotonic",return_value=10.),contextlib.redirect_stdout(output):
                driver.main()
        engine=engines[0]
        ops=[c["op"] for c in engine.calls]
        self.assertEqual(ops,["describe","prepare","sample","regime","prepare","regime","sample","quit"])
        self.assertEqual(engine.sample_epochs,[1,3])
        self.assertEqual(engine.sample_seeds[1],20261017+1+1000003*3)
        result=json.loads(output.getvalue())
        region=result["regions"][0]
        self.assertEqual(region["epoch"],3)
        self.assertEqual(region["current_epoch"]["B"],.6)
        self.assertEqual(region["draws"],2)
        self.assertEqual(region["setup_ms"],19)
        rebuild=region["regime_attempts"][-1]
        self.assertEqual((rebuild["trigger"],rebuild["parent_epoch"],rebuild["k"]),
                         ("after_joint_growth",2,2))

    def test_main_accepted_zero_growth_resets_epoch_without_regime_or_stale_sample(self):
        engines=[]
        def factory(*args):
            e=MainFlowEngine(*args);e.zero_growth=True;engines.append(e);return e
        with tempfile.TemporaryDirectory() as tmp:
            binary=Path(tmp)/"fake-engine";binary.write_text("x")
            request=Path(tmp)/"request.json";request.write_text("{}")
            argv=["driver.py","--binary",str(binary),"--request",str(request),
                  "--budget","30","--cleanup-reserve","1","--initial-draws","1",
                  "--max-draws","2","--initial-state-cap","1000","--state-cap","2000",
                  "--time-ms","700","--no-gd-bounds"]
            output=io.StringIO()
            with patch.object(sys,"argv",argv),patch.object(driver,"Protocol",factory),\
                 patch.object(driver.time,"monotonic",return_value=10.),contextlib.redirect_stdout(output):
                driver.main()
        engine=engines[0]
        self.assertEqual([c["op"] for c in engine.calls],
                         ["describe","prepare","sample","regime","prepare","quit"])
        result=json.loads(output.getvalue());region=result["regions"][0]
        self.assertEqual(region["epoch"],2)
        self.assertEqual(region["current_epoch"]["n"],0)
        self.assertEqual(region["draws"],1)

    def test_main_accepted_zero_rebuild_does_not_sample_after_epoch_change(self):
        engines=[]
        def factory(*args):
            e=MainFlowEngine(*args);e.zero_rebuild=True;engines.append(e);return e
        result,engine=self._run_main_with_factory(factory)
        region=result["regions"][0]
        self.assertEqual([c["op"] for c in engine.calls],
                         ["describe","prepare","sample","regime","prepare","regime","quit"])
        self.assertEqual((region["epoch"],region["current_epoch"]["n"],region["draws"]),(3,0,1))

    def test_main_protocol_timeout_stops_without_sampling_or_reusing_engine(self):
        def factory(*args):
            e=MainFlowEngine(*args);e.timeout_rebuild=True;return e
        result,engine=self._run_main_with_factory(factory)
        region=result["regions"][0]
        self.assertEqual([c["op"] for c in engine.calls],
                         ["describe","prepare","sample","regime","prepare","regime","quit"])
        self.assertEqual(region["regime_status"],"protocol_timeout")
        self.assertEqual(region["draws"],1)

    def _run_main_with_factory(self, factory):
        engines=[]
        def tracked(*args):
            engine=factory(*args);engines.append(engine);return engine
        with tempfile.TemporaryDirectory() as tmp:
            binary=Path(tmp)/"fake-engine";binary.write_text("x")
            request=Path(tmp)/"request.json";request.write_text("{}")
            argv=["driver.py","--binary",str(binary),"--request",str(request),
                  "--budget","30","--cleanup-reserve","1","--initial-draws","1",
                  "--max-draws","2","--initial-state-cap","1000","--state-cap","2000",
                  "--time-ms","700","--no-gd-bounds"]
            output=io.StringIO()
            with patch.object(sys,"argv",argv),patch.object(driver,"Protocol",tracked),\
                 patch.object(driver.time,"monotonic",return_value=10.),contextlib.redirect_stdout(output):
                driver.main()
        return json.loads(output.getvalue()),engines[0]


if __name__ == "__main__":
    unittest.main()
