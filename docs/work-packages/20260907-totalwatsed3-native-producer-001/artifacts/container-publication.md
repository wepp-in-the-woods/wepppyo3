# Reviewed container publication

confirmed: the common runtime image is published and verified as
`ghcr.io/rogerlew/wepppy@sha256:3201f5cdd187e0e1bae280425c60ab81141c89371276861c076aa287bc0efcb7`.
The [canonical workflow](https://github.com/rogerlew/wepppy/actions/runs/34192361367)
completed successfully from WEPPpy revision
`d8fbe9f4ab270b6a7ab119d7cdda55d3cb628986`, pinning native release revision
`bb7451eeb83690e0ffc5f5eed7c4f4c9c1688d23`. OCI revision labels and the registry
index digest match the workflow receipt. No production rollout was performed.

## Remote retrieval and packed implementation

Clean Forest checkouts started with isolated empty LFS stores. All 127 native
fixture Parquet payloads and all 639 WEPPpy LFS files were fetched from GitHub,
materialized and verified. The native binary and paired Python implementation
hashes match the reviewed release. Both source revisions are remotely available.

The pulled image passed its required-native startup preflight and all six packed
implementation-file hashes in release-integration-manifest.json. Twenty-five
public-facade parity cases pass with the image's own Python/native code. Only the
original decimal-pleasing input fixture was mounted read-only from the clean
remote WEPPpy checkout: Docker excludes WEPPpy tests. No source-code directory
was mounted. Other frozen inputs/oracles come from the image's native checkout.

## Real image-contained workflow

Use the canonical Forest Compose rq-worker environment, secrets and entrypoint,
with the published-image-compose-published.yml disposable override. Set
TOTALWATSED_PUBLISHED_IMAGE to the immutable reference above and
WCTL_COMPOSE_FILE_EXTRAS to that override; invoke wctl run --no-deps with
/opt/venv/bin/python and the mounted totalwatsed3_acceptance.py harness.
The override replaces development source mounts with evidence and read-only
scale inputs; the actual image Mounts receipt confirms no implementation overlay.

The real WepppyRqWorker runs the production 5,860-hillslope stage, a dependent
strict-parity/water-balance/query-catalog check, and the production 586-hillslope
stage as a subsequent job. All three finish. Peak is 1,191,563,264 bytes
(1.11 GiB), below 9 GiB and the 12 GiB limit; OOM/max events and restarts are zero.
UID is 1000, GID/groups 993, umask 0022, CPUs 0-11 and WEPPPY_NCPU=12.
No producer or controller is mocked. The private jobs/queue and one-shot
containers were removed only after evidence capture and status readback.

Receipts, parity details, downstream results, generated README, actual mounts,
container state and cleanup records are retained as published-image-* artifacts.
The raw workflow log is retained under target/ with its SHA-256 in publication.json.
This proves the published Forest workflow; it is not Kubernetes identity/mount
attestation. Earlier local-image, cache-sensitive and failed measurements remain
separately labeled historical evidence.
