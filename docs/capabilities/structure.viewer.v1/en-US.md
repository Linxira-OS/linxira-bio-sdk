# Interactive Structure Viewer

## Purpose

Inspect PDB and mmCIF coordinates locally with interactive rotation, zoom, pan, multiple molecular representations, confidence coloring, and PNG snapshots.

## Inputs

A local PDB or mmCIF file, optionally gzip-compressed and detected by content.

## Parameters

Choose backbone, ball-and-stick, or space-filling representation; toggle hetero atoms, hydrogen, and confidence coloring; adjust the view interactively.

## Outputs

An interactive desktop view and optional PNG snapshot of the current camera and representation.

## Examples

```bash
linxira-bio-ui <input.pdb|mmcif>
```

## Interpretation

Confidence coloring uses coordinate confidence values when present; representation changes geometry display, not the underlying structure.

## Caveats

The viewer is exploratory and does not perform molecular dynamics, validation, docking, or clinical interpretation.

## Runtime Dependencies

The native Rust egui application uses local graphics rendering and does not use a WebView or upload coordinates.

## Citations

Cite the structure source, prediction or experiment method, relevant database accession, and capability version.

## Troubleshooting

If rendering is unavailable, set `LINXIRA_BIO_RENDERER=glow` or `wgpu`; confirm coordinates are finite and the file contains atom records.
