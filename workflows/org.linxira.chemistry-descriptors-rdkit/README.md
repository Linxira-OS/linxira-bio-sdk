# Chemistry Descriptors (RDKit)

Computes physicochemical descriptors for SDF molecule records using RDKit:
molecular weight, CLogP, TPSA, H-bond donors/acceptors, rotatable bonds, ring
counts, formal charge, and molecular formula. Output is a TSV descriptor
table; the result envelope carries per-molecule descriptor rows.

Requires the pinned Python 3.12 environment from `requirements.lock`
(`pip install --require-hashes -r requirements.lock`). The pack is invoked
through the Linxira Bio worker; see `docs/capabilities/chemistry.descriptors.v1`
for the capability documentation.
