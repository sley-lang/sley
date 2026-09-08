# SLEY2 remote head locator

Derived view of `machineresearch/sley-2.0/machine-summary.json` `locator`
(rendered by `scripts/check_remote_head.py --render`; git is authoritative for
commit identity, this file only makes remote navigation fast).

Canonical integration branch: `main`
Latest validated integration commit: `560a5f16ebe9edaaee6837b779f6af94ad9ae310`
Active REWEAVE / campaign lanes:
- arch-tighten: arch/tighten-r1 (owner architecture-tightening campaign (integrator); audit slices landing; merge to main after the lane sessions hand off)
- lane3 RW-080: lane3/rw080-lower-scaffold-f1, lane3/rw080-f6f7-vm, lane3/rw080-checker-scaffold (owner RW-080 codec construction (Ariadne lane); provisional slices 1 to 7 on main under the 2026-09-07 operator override; BLOCKED, not authority)
- lane2 S20-770: lane2/s20770-council-rereview-r1, lane2/s20770-acceptance-closeout-r1 (owner required-contract index (Ariadne package); Council re-review in progress; not merged)
Current canonical Sley 2.0 spec path: `/home/greyforge/machineresearch/Sley2.0mastergoal.md` (sha256 `e26eed88167a3ef47472e3b2eea13b7d42c4bdb1befc3cbeca16c28501a350b9`; in-repo dossier `machineresearch/sley-2.0/`)
Current REWEAVE master path: `/home/greyforge/machineresearch/SLEY_2X_REWEAVE_MASTER_SPEC_V1.md` (sha256 `61d20471906b00fab05a9e0f174ad9c616e734a5f38fd590ed44af2533535a63`; adopted by `docs/adr/ADR-0049-reweave-scope-adoption.md`)
Current schema epoch: `ae5b235713b46c04f73c1decd0fb0bb57c5557d0fe89dae7ddac4a7dba25564e` (SSMC1 epoch 1 (bootstrap SchemaEpochId))
Current host ABI: HOST_ABI_V2 (`sley2-host-abi-2`, digest `bc564653302a73eb5f998427250a2bb7cd87f5685ef12619bd4ae1f1b2af70d5`)
Current execution-package version: EXEC_PACKAGE_V2 (`sley2-exec-package-2`, digest `f4958c5e3d57762173b881288b008af17d45b5f07a431fcc442d9eec5770da94`)
Current bootstrap profile: BOOTSTRAP_PROFILE_2 (`sley2-bootstrap-profile-2`, digest `fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459`)
Review candidates (immutable refs):
- round 1: `review/arch-tighten-r1-round1` = `8ff792e9f9a5f26e6556b3204887c3f6d4e26b6d` (Nabu (FAIL, 3 P1, 1 P3: erratum record for the v1 digest literal, anti-goal report drift, candidate not bound to a stable ref), Ariadne (round 1 in progress))
Known blocked gates:
- R2 exit NOT_READY: premium delta re-review of the RW-075 correction still FAIL/PENDING (scripts/check_r2_exit.py)
- RW-080 BLOCKED: Nabu round-12 review, premium round 2, S20-780 independent acceptance pending; slices 1 to 7 provisional
- Self-host succession not started: S, C1, C2, C3 do not exist (SH0)
- Finding register FINDING_REGISTER_OPEN (open Council reviews across packages; evidence/review/finding-register.json)
- release-candidate-smoke re-attest pending since the S20-330 closure (RESUME.md 2026-09-05)
- Operator gates: narrowed schema-epoch decision, succession trials, root license text, second-host attestation, release decision; publication_authorized false
- Open 2.0 requirement gap AT-MW-02: master 8.2 GetEntityVersion/GetSignature have no SMP1 method; owned by S20-410 under an SMP1 revision
Remote: origin = private working repository under the same GitHub organization (URL in git remote -v; the name carries the clean-room sentinel substring and is not spelled here); mirror = the public sanitized mirror (main, arch/tighten-r1 and the lane branches pushed to the private origin on 2026-09-08 (AT-RF-01 resolved by operator instruction); the public mirror keeps its unrelated sanitized history and is refreshed only by the mirror pipeline under a publication decision)
Last updated UTC: 2026-09-08T09:20:00Z

Verification at render time: PASS
