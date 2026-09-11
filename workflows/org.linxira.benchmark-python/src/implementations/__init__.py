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

from . import sequence_stats_v1

IMPLEMENTATIONS: dict[str, ModuleType] = {
    sequence_stats_v1.CAPABILITY: sequence_stats_v1,
}
