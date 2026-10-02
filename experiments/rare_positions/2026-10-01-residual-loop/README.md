# Historical experiment evidence

`evidence.tar.gz` preserves the original measurements, hashes, source patches
and test logs under their original relative names. `manifest.json` records every
file checksum. These are historical experiments, not current runtime defaults.

Extract into a temporary directory for inspection, or here to run the original
report commands:

```sh
tar -xzf evidence.tar.gz
```

See the experiment report and `../2026-10-02-rust-experiment-decisions.md` for the
final disposition. Rejected code exists only in archival patches.
