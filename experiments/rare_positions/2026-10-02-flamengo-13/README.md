# Experiment evidence

Reports are in the parent directory. `text-evidence.tar.gz` preserves the full
JSON summaries, source patches and test logs under their original relative
filenames. Extract it here when reproducing or inspecting individual runs:

```bash
tar -xzf text-evidence.tar.gz
```

The other `*-raw.tar.gz` archives contain the individual request runs. Shipping
verification is summarized in `shipping/summary.json` where present.
