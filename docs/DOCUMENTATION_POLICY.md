# Documentation Policy

## Canonical Source

Each available capability has `zh-CN.md` and `en-US.md` under
`docs/capabilities/CAPABILITY_ID/`. These files are shipped offline and are the
canonical source for the GUI help panel, CLI help, AI retrieval, and future
website generation.

The native GUI embeds these canonical files at compile time. Release packaging
also stages the unchanged `docs/` tree according to
`packaging/bundle-manifest.json`; there is no manually maintained copy. Run
`scripts/stage-release.py --check` to validate the shared package inputs.

Both locales must contain these exact second-level sections:

- Purpose
- Inputs
- Parameters
- Outputs
- Examples
- Interpretation
- Caveats
- Runtime Dependencies
- Citations
- Troubleshooting

Chinese pages use the equivalent Chinese section names recorded by the
repository validator. A translation must preserve commands, identifiers,
scientific limits, and citations. It does not need to mirror English sentence
structure.

## Release Gate

CI rejects an `available` capability when either locale is missing, required
sections are absent, a planned feature is described as usable, or the example
command does not match the capability catalog. Experimental GUI surfaces may
link to general product documentation until they become available
capabilities.

Documentation is part of the capability contract. Updating behavior, defaults,
accepted formats, scientific interpretation, or runtime requirements requires
updating both locales in the same change.
