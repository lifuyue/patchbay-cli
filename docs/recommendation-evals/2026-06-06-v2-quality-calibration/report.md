# V2 Quality Filtering And Weight Calibration

## Stage Summary

V2 prioritizes feed quality over recall. The stage tightened profile-fit gates, stale freshness caps, competition/noise hiding, feedback cooldown, and old weak task handling. It does not implement fallback C or new discovery lanes; low visible counts are carried to V3.

Decision: `proceed_with_carried_risks`.

## Production Changes

- Reweighted value ranking toward profile fit and execution quality; high-value categories now require stronger profile fit.
- Added profile-specific caps for frontend, Rust/Go backend, Python data/CLI, AI agent tools, and DevOps/infra.
- Hid low-trust repos, low-fit candidates, marketplace/fork anomalies, answered support questions, claimed/submitted PR patterns, old broad needs-triage feature requests, and old low-impact needs-triage issues.
- Reduced stale issue freshness: issues older than one year are capped at low freshness even with recent activity.
- Strengthened shown/read/prepared feedback cooldown.
- Expanded offline fixtures from 48 baseline samples to 80 samples, including representative live failures.

## Offline Metrics

Baseline V1:

| metric | value |
| --- | ---: |
| samples | 48 |
| visible | 22 |
| precision@5 | 0.81 |
| precision@10 | 0.81 |
| reject leakage | 2 |
| profile mismatch leakage | 1 |
| ranking inversions | 5 |

V2 final:

| metric | value |
| --- | ---: |
| samples | 80 |
| visible | 23 |
| precision@5 | 0.97 |
| precision@10 | 0.97 |
| reject leakage | 0 |
| profile mismatch leakage | 0 |
| stale high-rank leakage | 0 |
| competition leakage | 0 |
| dashboard noise leakage | 0 |
| feedback cooldown | 5/5 |
| ranking inversions | 8 |

Hard offline goals passed.

## Live Matrix

Run: `/tmp/issue-finder-v2-live-final6`

Command shape for each profile: `target/debug/issue-finder scout --limit 15 --refresh --dry-run --json`

| profile | visible | discovery candidates | top10 low fit | top10 old high freshness | top10 claim/pr | top good/excellent | all visible good/excellent |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| default_cli_devtools | 7 | 161 | 0 | 0 | 0 | 4/5 | 5/7 |
| typescript_frontend | 3 | 197 | 0 | 0 | 0 | 3/3 | 3/3 |
| rust_backend_systems | 0 | 140 | 0 | 0 | 0 | N/A | N/A |
| python_data_cli | 1 | 172 | 0 | 0 | 0 | 1/1 | 1/1 |
| ai_agent_tools | 2 | 171 | 0 | 0 | 0 | 2/2 | 2/2 |
| devops_infra | 1 | 172 | 0 | 0 | 0 | 1/1 | 1/1 |

Global manual top good/excellent: 11/12 = 0.92.

Global all-visible good/excellent observation: 12/14 = 0.86.

All stderr files were empty. Discovery caches were non-empty for all six profiles, so the final run was not affected by the earlier GitHub secondary-limit empty-cache issue.

## Manual Review

| profile | issue | label | notes |
| --- | --- | --- | --- |
| default_cli_devtools | apache/incubator-superset#40407 | excellent | Concrete recent TypeScript/Superset bug, maintainer confirms valid and invites PR. |
| default_cli_devtools | apache/incubator-superset#40401 | good | Clear SQL Lab UI bug with file path and reproduction. |
| default_cli_devtools | bcherny/json-schema-to-typescript#360 | weak | Relevant CLI behavior/docs ambiguity, but old and maintainer described current behavior as expected. |
| default_cli_devtools | bitwarden/clients#20694 | good | Concrete import bug, maintainer marked good first issue in comments. |
| default_cli_devtools | aws-amplify/amplify-cli#13065 | good | Clear Amplify CLI validation bug with reproduction. |
| default_cli_devtools | bitwarden/clients#16746 | good | Concrete desktop/client manifest cleanup with steps and internal tracking. |
| default_cli_devtools | xi-editor/xi-mac#474 | weak | Old but concrete Rust/Xcode build issue with maintainer guidance; lower visible only. |
| typescript_frontend | rjsf-team/react-jsonschema-form#3183 | good | Relevant React form performance issue with reproduction and maintainer welcomes help. |
| typescript_frontend | rjsf-team/react-jsonschema-form#3517 | good | Old but concrete React performance regression with reproduction discussion; freshness is capped. |
| typescript_frontend | apifytech/apify-js#2680 | good | Browser/tooling monitor feature with implementation direction. |
| python_data_cli | PyTorchLightning/pytorch-lightning#9136 | excellent | Detailed Python/ML profiler bug with traceback and reproduction discussion. |
| ai_agent_tools | PyTorchLightning/pytorch-lightning#9136 | excellent | Strong AI/ML tooling fit; same concrete profiler failure. |
| ai_agent_tools | agenta-ai/agenta#4549 | good | Recent LLM evaluation platform UI bug; small but profile-specific. |
| devops_infra | GoogleContainerTools/skaffold#4898 | excellent | Detailed Docker/Kubernetes sync bug with configuration, logs, and clear profile fit. |

## Failure Examples Backfilled

- `bytecodealliance/jco#1594`: WIT/WebAssembly component maintenance was incorrectly treated as frontend component work.
- `containers/ramalama#912`: old broad vLLM EPIC was visible in default lower feed.
- `bastion-rs/bastion#2`: old broad runtime dashboard feature was the only Rust/Go visible candidate.
- `golang-cafe/job-board#45`: old low-impact Dockerfile task was the only DevOps visible candidate in one run.
- Multiple live competition phrase failures were added to fixtures, including assign/take/PR/support-question variants.

## Carried Risks

V2 live recall does not meet the global target:

| profile | visible | target | owner stage |
| --- | ---: | ---: | --- |
| default_cli_devtools | 7 | 11 | V3 |
| typescript_frontend | 3 | 11 | V3 |
| rust_backend_systems | 0 | 11 | V3 |
| python_data_cli | 1 | 11 | V3 |
| ai_agent_tools | 2 | 11 | V3 |
| devops_infra | 1 | 11 | V3 |

Reason: V2 intentionally removed weak candidates without adding fallback C, trusted profile buckets, or strong global fallback. Those are V3 scope.

## Validation

- `cargo test --test recommendation_eval`: passed
- `cargo test`: passed
- `cargo clippy --all-targets -- -D warnings`: passed
- `cargo build`: passed
- Six live profiles with `limit=15`, `refresh=true`, `recordExposure=false`: passed quality review with carried recall risks

