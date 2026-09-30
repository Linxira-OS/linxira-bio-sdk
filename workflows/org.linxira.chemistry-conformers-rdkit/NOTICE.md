# Linxira Bio Chemistry Conformers Pack

Runtime dependencies are installed separately and are not vendored. The
declared Python dependencies are pinned by the `requirements.lock` hash file:

- RDKit (`rdkit`) 2026.3.5, CPython 3.12 wheels and sdist
- NumPy (`numpy`) 2.5.2, CPython 3.12 wheels and sdist

Distribution: AGPL-3.0-or-later, copyright Linxira OS.

This pack executes `src/conformers.py` with the interpreter selected by the
worker (`LINXIRA_BIO_WORKFLOW_PYTHON`); it never mutates the global Python
environment. The script embeds SDF molecule records with RDKit ETKDGv3 at a
fixed seed, minimises every conformer with MMFF94, and writes a multi-record
conformer SDF plus a versioned result envelope.
