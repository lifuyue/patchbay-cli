# V4 Competition Evidence Completion Report
## Stage Summary
Decision: `proceed`. V4 reduces final top10 competition-evidence missing to 0 in the final live matrix and hides open PR / claimed / working / submitted-PR evidence before final selection. Five of six profiles meet the V3 hard-pass visible target (`>=11`); default remains at 10 after stricter claim filtering and is recorded as a recall observation for later stages.
## Production Changes
- Added post-ranking competition completion with bounded timeline/comment retry and final reranking.
- Limited missing/failed/skipped competition evidence out of top5 and to at most two in top10.
- Expanded natural-language competition markers for `/assign`, work-on-it, sent/filed PR, draft PR, and PR-solves-this comments.
- Propagated comment enrichment failures into competition warnings so candidates without comment evidence cannot masquerade as clear.
- Aligned offline eval profiles with the fixed six live profile configs and added DevOps/infra aliases for registry, storage, observability, CRD/controller, and related infrastructure terms.
- Expanded trusted profile buckets for default, frontend, AI, and DevOps based on real open beginner/help-wanted supply.

## Offline Metrics
- samples: 93
- visible: 27
- precisionAt5: 0.96875
- precisionAt10: 0.96875
- rejectLeakage: 0
- competitionLeakage: 0
- dashboardNoiseLeakage: 0
- profileMismatchLeakage: 0
- fallbackFillRate: 1.0

## Live Matrix
| profile | visible | top10 missing evidence | top10 structured competition leaks | top5 good/excellent | all visible good/excellent | seconds/source |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| default_cli_devtools | 10 | 0 | 0 | 4/5 | 5/10 | rerun7 452s |
| typescript_frontend | 11 | 0 | 0 | 5/5 | 8/11 | final 469s |
| rust_backend_systems | 12 | 0 | 0 | 4/5 | 5/12 | rerun4 388s |
| python_data_cli | 13 | 0 | 0 | 5/5 | 8/13 | final 384s |
| ai_agent_tools | 11 | 0 | 0 | 5/5 | 10/11 | final 371s |
| devops_infra | 11 | 0 | 0 | 5/5 | 7/11 | rerun10 469s |

## Manual Review
### default_cli_devtools
| rank | issue | quality | notes |
| ---: | --- | --- | --- |
| 1 | nushell/nushell#9065 | good | Concrete Nushell IPv6 link-local URL bug with maintainer openness. |
| 2 | nushell/nushell#7792 | good | Concrete Windows CLI start/msedge behavior; old but actionable. |
| 3 | astral-sh/uv#4988 | good | Concrete uv Python listing gap; partial prior fix but musl/arch verification remains actionable. |
| 4 | djc/quinn#660 | good | Clear QUIC stream ID ordering issue with design discussion. |
| 5 | rust-lang/rustfmt#4483 | weak | Old rustfmt task with prior mentoring thread; still scoped but lower confidence. |
| 6 | djc/quinn#1744 | good | Concrete DragonFlyBSD build failure with platform-specific path. |
| 7 | nushell/nushell#3728 | weak | Old syntax-plugin request with stale/reopen history. |
| 8 | kata-containers/kata-containers#9320 | weak | Tiny documentation reminder task; useful but low value. |
| 9 | bheisler/criterion.rs#508 | weak | Old Criterion benchmarking ergonomics issue. |
| 10 | bheisler/criterion.rs#195 | weak | Old unsafe-block cleanup with complex prior discussion. |

### typescript_frontend
| rank | issue | quality | notes |
| ---: | --- | --- | --- |
| 1 | rjsf-team/react-jsonschema-form#3183 | good | React JSONSchema Form large-list performance issue with repro and maintainer welcomes help. |
| 2 | rjsf-team/react-jsonschema-form#3517 | good | React JSONSchema Form array-item performance regression with version context. |
| 3 | facebook/docusaurus#7238 | good | Docusaurus top-level-await server build transform bug with clear frontend build fit. |
| 4 | marmelab/react-admin#9419 | good | React-admin useWatch/default value bug; concrete form behavior. |
| 5 | rjsf-team/react-jsonschema-form#4483 | good | RJSF clearing-input required-value bug; concrete form issue. |
| 6 | appbaseio/reactivesearch#251 | weak | Old middleware feature request; relevant but broad. |
| 7 | appbaseio/reactivesearch#414 | good | ReactiveSearch URLParams sensitivity issue; narrow UI behavior. |
| 8 | jameskerr/react-arborist#325 | good | React Arborist ARIA forwarding accessibility task. |
| 9 | vuetifyjs/vuetify#22893 | good | Vuetify carousel/window mouse wheel feature; UI component fit. |
| 10 | elastic/eui#7093 | weak | EUI SSR htmlIdGenerator migration is broad refactor. |
| 11 | angular/angular#20371 | weak | Old Angular forms updateOn feature request. |

### rust_backend_systems
| rank | issue | quality | notes |
| ---: | --- | --- | --- |
| 1 | nushell/nushell#9065 | good | Nushell IPv6 URL parsing issue; relevant Rust CLI/networking. |
| 2 | nushell/nushell#7792 | good | Nushell Windows start command behavior; actionable CLI issue. |
| 3 | djc/quinn#660 | good | QUIC stream ID ordering semantics; strong Rust systems fit. |
| 4 | go-swagger/go-swagger#717 | good | Go Swagger code generation constants task; clear backend codegen fit. |
| 5 | rust-lang/rustfmt#4483 | weak | Old rustfmt task with prior mentoring thread. |
| 6 | go-swagger/go-swagger#872 | weak | Go Swagger model string readability enhancement is scoped but old. |
| 7 | go-swagger/go-swagger#1856 | weak | Go Swagger docs clarification; lower implementation value. |
| 8 | djc/quinn#1744 | good | DragonFlyBSD Quinn build failure; concrete systems/platform issue. |
| 9 | rust-analyzer/rust-analyzer#7950 | weak | Rust-analyzer benchmark-suite expansion is broad. |
| 10 | bheisler/criterion.rs#508 | weak | Criterion instruction-count warmup ergonomics is old. |
| 11 | bheisler/criterion.rs#195 | weak | Criterion unsafe cleanup is old and complex. |
| 12 | bheisler/criterion.rs#250 | weak | Criterion throughput regression wording is narrow and old. |

### python_data_cli
| rank | issue | quality | notes |
| ---: | --- | --- | --- |
| 1 | jupyter-server/jupyter_server#434 | good | Jupyter Server port config bug with concrete behavior. |
| 2 | jupyter-server/jupyter_server#1008 | good | Jupyter shutdown error with password preset; reproducible server bug. |
| 3 | PyTorchLightning/pytorch-lightning#9136 | good | Lightning AdvancedProfiler ValueError in test evaluation path. |
| 4 | PyTorchLightning/pytorch-lightning#14063 | good | Lightning profiler schedule bug; concrete Python tooling fit. |
| 5 | pantsbuild/pants#14431 | good | Pants PEX_EXTRA_SYS_PATH test env behavior; strong Python build/test fit. |
| 6 | astral-sh/uv#4988 | good | uv Python listing gap; relevant Python CLI issue. |
| 7 | PyTorchLightning/pytorch-lightning#7361 | weak | Lightning num_nodes/ClusterEnvironment issue has prior PR direction but remains useful. |
| 8 | pantsbuild/pants#13825 | good | Pants skip_* warning message issue; concrete CLI UX. |
| 9 | PyTorchLightning/pytorch-lightning#18060 | good | Lightning checkpoint batch progress bug; concrete training state issue. |
| 10 | pantsbuild/pants#15376 | weak | Pants deprecation warning in toml string literals; lower value. |
| 11 | pylint-dev/pylint#8256 | weak | Pylint comprehension wrong-fix issue; small lint fix. |
| 12 | pylint-dev/pylint#5793 | weak | Pylint arguments-differ wording issue; small lint fix. |
| 13 | pypa/hatch#2141 | weak | Hatch optional uv integration task; useful but lower certainty. |

### ai_agent_tools
| rank | issue | quality | notes |
| ---: | --- | --- | --- |
| 1 | rhesis-ai/rhesis#1872 | good | Rhesis GitLab MCP integration; clear AI-agent tool work. |
| 2 | rhesis-ai/rhesis#1873 | good | Rhesis Asana MCP integration; clear integration task. |
| 3 | rhesis-ai/rhesis#1874 | good | Rhesis Shortcut MCP integration; clear integration task. |
| 4 | rhesis-ai/rhesis#1867 | good | Rhesis Pydantic AI instrumentation integration; strong fit. |
| 5 | latitude-dev/latitude-llm#3425 | good | Latitude AutoGen integration; strong AI-agent observability fit. |
| 6 | latitude-dev/latitude-llm#3400 | good | Latitude Hermes Agent OTLP traces integration; strong fit. |
| 7 | latitude-dev/latitude-llm#3401 | good | Latitude Python SDK task; good agent/tooling fit. |
| 8 | containers/ramalama#801 | good | Ramalama vLLM runtime on macOS bug; relevant local AI tooling. |
| 9 | microsoft/markitdown#12 | weak | Markitdown LLM integration is broad and has subthread PR references; keep lower visible. |
| 10 | traceloop/opentelemetry-mcp-server#7 | good | OpenTelemetry MCP Sentry backend support; clear MCP/observability integration. |
| 11 | traceloop/opentelemetry-mcp-server#6 | good | OpenTelemetry MCP Datadog backend support; clear MCP/observability integration. |

### devops_infra
| rank | issue | quality | notes |
| ---: | --- | --- | --- |
| 1 | rook/rook#17285 | excellent | Rook/Ceph config override newline crash; concrete Kubernetes storage bug. |
| 2 | linkerd/linkerd2#5219 | good | Linkerd tap EOF on k3d resource category; concrete Kubernetes/service-mesh bug. |
| 3 | kubernetes-sigs/kind#3834 | good | kind swap metrics endpoint issue; concrete Kubernetes metrics behavior. |
| 4 | GoogleContainerTools/skaffold#4898 | good | Skaffold Dockerfile sync inference issue with reproduction. |
| 5 | external-secrets/external-secrets#5549 | good | External Secrets validation status bug; concrete operator behavior. |
| 6 | goharbor/harbor#21066 | good | Harbor GitLab registry replication failure; concrete registry integration bug. |
| 7 | goharbor/harbor#22539 | weak | Harbor Docker Compose install feature is useful but broader. |
| 8 | cert-manager/cert-manager#2334 | weak | cert-manager network policy docs has many comments and PR references. |
| 9 | cert-manager/cert-manager#2538 | good | cert-manager ingress class behavior remains concrete despite discussion. |
| 10 | cert-manager/cert-manager#3103 | weak | cert-manager probes request has long thread and prior PR context. |
| 11 | GoogleContainerTools/jib#3132 | weak | Jib Gradle configuration cache support is relevant but peripheral to core DevOps profile. |

## Failure Examples Refilled Into Fixtures
- missing timeline completed with open PR hidden
- clear completed timeline remains visible
- claim plus PR completion hidden
- /assign plus ready-to-contribute hidden
- issue-body /assign plus filed PR hidden
- take-this-issue plus sent-pull-request hidden
- work-on-it claim hidden
- PR-solves-this hidden
- registry/replication DevOps profile-fit sample visible

## Residual Risks
- `default_cli_devtools` visible count is 10 after stricter competition filtering. V4 live contract requires at least five profiles at hard pass, which is met; default recall remains a product observation.
- Refresh=true live runtime is 6-9 minutes per profile. This is intentionally carried to V5 Performance, Cache, And API Budget.
- Some lower-ranked visible candidates are old or broad but outside V4 competition leakage scope; their manual quality is recorded in `visible.jsonl`.

## Validation
- `ISSUE_FINDER_RECOMMENDATION_EVAL_REPORT_DIR=docs/recommendation-evals/2026-06-07-v4-competition-completion/offline cargo test --test recommendation_eval`: passed.
- `cargo test`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo build`: passed.
- `git diff --check`: passed.
