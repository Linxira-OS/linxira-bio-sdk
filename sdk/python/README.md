# linxira-bio-sdk

Python client for Linxira Bio. The SDK is a subprocess client; it does not
contain the native engine. Install a Linxira Bio release, or build
`linxira-bio-worker` and make it discoverable, before running analyses.

## Install

```bash
pip install linxira-bio-sdk
```

## Use

```python
from linxira_bio import LinxiraClient

client = LinxiraClient()  # or worker_bin="/path/to/linxira-bio-worker"
result = client.execute(
    "sequence.stats.v1",
    {"fasta": "tests/fixtures/sequences/tiny.fa"},
)
print(result.status, result.result)
```

`LinxiraClient` searches for `linxira-bio-worker` in this order:

1. `LINXIRA_BIO_WORKER`
2. `PATH`
3. Build output directories relative to a source checkout

The optional `AutoClient` exposes every catalog capability as a method. Its
catalog is read from `LINXIRA_BIO_CATALOG`, a packaged `catalog.json`, a source
checkout, or an installed `linxira-bio` executable.

## Requirements

- Python 3.9+
- `linxira-bio-worker` for the target platform

## License

AGPL-3.0-or-later.
