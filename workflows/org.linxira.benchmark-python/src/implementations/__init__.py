"""Registry of the independent Python implementations hosted by this pack.

Each module exposes the same small surface so the harness stays generic:

* ``CAPABILITY``   the capability id it re-implements
* ``INPUT_ROLES``  required input roles, identical to the Rust contract
* ``PARAMETERS``   accepted parameter names (the harness rejects anything else)
* ``software()``   provenance entries for the libraries actually imported
* ``run(inputs, parameters)``  returns the ``result`` object of the envelope
"""

from __future__ import annotations

from types import ModuleType

from . import enrichment_overrepresentation_v1
from . import expression_pca_v1
from . import fastq_qc_v1
from . import sequence_stats_v1
from . import set_venn_v1
from . import structure_pdb_summary_v1
from . import variant_stats_v1

IMPLEMENTATIONS: dict[str, ModuleType] = {
    enrichment_overrepresentation_v1.CAPABILITY: enrichment_overrepresentation_v1,
    expression_pca_v1.CAPABILITY: expression_pca_v1,
    fastq_qc_v1.CAPABILITY: fastq_qc_v1,
    sequence_stats_v1.CAPABILITY: sequence_stats_v1,
    set_venn_v1.CAPABILITY: set_venn_v1,
    structure_pdb_summary_v1.CAPABILITY: structure_pdb_summary_v1,
    variant_stats_v1.CAPABILITY: variant_stats_v1,
}
