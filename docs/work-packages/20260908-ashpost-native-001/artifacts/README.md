# Evidence index

This directory is initially empty except for this index. The executing agent
must add and index raw evidence for:

- repository, toolchain, image, and fixture provenance;
- immutable Python oracles for both ash models;
- native API and multi-file atomic publication contracts;
- parity, schema, facade, version, documentation, and catalog checks;
- bounded-scheduler lifetime and process-count measurements;
- standalone and Compose timing plus cgroup memory evidence;
- OR-202 RQ result, output hashes, restart/OOM state, and subsequent-job proof;
- focused/full validation and changed-file reports; and
- independent correctness, QA, performance, and security reviews.

Every claim must be labeled `Static:` or `Ran:`. Keep large generated outputs
outside Git unless a reviewed reusable test fixture requires Git LFS.
