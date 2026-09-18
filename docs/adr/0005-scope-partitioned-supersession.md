# Scope-Partitioned Supersession

Engineering policies frequently diverge across environments (e.g., staging vs. production) or targets (e.g., Android vs. iOS). We decided that supersession assertions only invalidate predecessor nodes within scopes that match or are covered by the successor's declared scope. Outside of that scope, the predecessor remains active guidance, accompanied by a contextual advisory note indicating that a successor exists in other scopes.
