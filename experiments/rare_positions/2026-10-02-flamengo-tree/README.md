# R41 bounded complete tree evidence

See [the experiment report](../2026-10-02-rust-flamengo-tree.md) for results,
correctness assumptions and reproduction commands. The production source is
unchanged; `prototype.patch` applies to a temporary copy of `f0a332b8`.

`summary.json` is a compact overview. `evidence.tar.gz` contains212 raw files:
standalone quality/resource screens, five500-pilot HTTP pairs,35 smaller-pilot
HTTP pairs across seven snapshots, three flag-off controls, and build/test logs.
The archive paths follow the original experimental directory structure.

`manifest.json` records the archive hash and every member's byte count/SHA256.
All members were read back and verified after packing. HTTP summaries retain
both the original comparison and its correction for the public `reachable`
status; raw responses and measured times are unchanged.

To inspect:

```sh
tar -xzf experiments/rare_positions/2026-10-02-flamengo-tree/evidence.tar.gz \
  -C /tmp/flamengo-tree-evidence
```

Create the destination directory first. The replay patch retains no
team-specific runtime policy. Both measured full-request funding arms are
rejected; the patch is an offline experiment, not an enabling release.
