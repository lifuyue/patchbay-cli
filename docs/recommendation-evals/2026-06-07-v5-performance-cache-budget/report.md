# V5 Performance, Cache, And API Budget Report
## Stage Summary
Decision: `proceed`. V5 adds source-layer discovery/enrichment caches, a bounded GitHub API budget tracker, budget-exhaustion explanations, `scout --stats-json`, and a dry-run scout-result cache. Offline eval and hard tests pass. Refresh=true remains slow but is now bounded and measurable; refresh=false warm-cache meets the V5 target with 0 network requests.
## Production Changes
- Added `github_budget` request accounting with per-source network request, cache-hit, and budget-exhaustion metrics.
- Split discovery cache by lane/source and gave fallback discovery an independent TTL.
- Split enrichment cache by source for repo metadata, issue details, comments, timeline, growth, and competition completion timeline/comments.
- Added graceful budget exhaustion warnings so cache misses or exhausted sources do not panic.
- Added `scout --stats-json` to expose `ScoutResult` with API budget stats without changing existing `--json` candidate output.
- Added dry-run scout-result caching so fresh eval snapshots replay instantly with ranking-equivalent results.
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
## Live Refresh=true Matrix
| profile | visible | requests/budget | seconds | top10 missing | top10 structured leaks | top5 good/excellent | all visible good/excellent |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| default_cli_devtools | 10 | 941/1200 | 454.6 | 0 | 0 | 4/5 | 5/10 |
| typescript_frontend | 11 | 1082/1200 | 481.3 | 0 | 0 | 5/5 | 8/11 |
| rust_backend_systems | 12 | 511/1200 | 261.3 | 0 | 0 | 4/5 | 5/12 |
| python_data_cli | 12 | 879/1200 | 386.4 | 0 | 0 | 5/5 | 7/12 |
| ai_agent_tools | 11 | 882/1200 | 364.8 | 0 | 0 | 5/5 | 10/11 |
| devops_infra | 11 | 1120/1200 | 478.6 | 0 | 0 | 5/5 | 7/11 |
## Warm Cache Matrix
| profile | warm seconds | warm requests | cache hit | ranking equivalent |
| --- | ---: | ---: | --- | --- |
| default_cli_devtools | 0.023 | 0 | scout_result | true |
| typescript_frontend | 0.023 | 0 | scout_result | true |
| rust_backend_systems | 0.026 | 0 | scout_result | true |
| python_data_cli | 0.026 | 0 | scout_result | true |
| ai_agent_tools | 0.021 | 0 | scout_result | true |
| devops_infra | 0.026 | 0 | scout_result | true |

Warm six-profile total: `0.169s`; max single profile: `0.026s`.
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
| 10 | elastic/eui#7093 | weak | EUI SSR htmlIdGenerator migration is broad and includes a prior take-this-on comment. |
| 11 | angular/angular#20371 | weak | Old Angular forms updateOn feature request with long discussion. |

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
| 7 | PyTorchLightning/pytorch-lightning#7361 | weak | Lightning num_nodes/ClusterEnvironment issue has prior direction but remains useful. |
| 8 | pantsbuild/pants#13825 | weak | Pants skip_* warning has a direct take-this-on comment, so keep lower confidence. |
| 9 | PyTorchLightning/pytorch-lightning#18060 | good | Lightning checkpoint batch progress bug; concrete training state issue. |
| 10 | pantsbuild/pants#15376 | weak | Pants deprecation warning in toml string literals; lower value. |
| 11 | pylint-dev/pylint#5793 | weak | Pylint arguments-differ wording issue; small lint/docs fix. |
| 12 | pypa/hatch#2141 | weak | Hatch optional uv integration task has a prior attempted-take-look comment. |

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

## Failure Examples And Fixtures
- No new recommendation-ranking fixtures were added because V5 does not intentionally change ranking semantics.
- Added/updated cache and budget tests for cache-hit no-network behavior, stale cache refresh, fallback TTL separation, timeline completion cache reuse, budget exhaustion, and fresh-cache ranking equivalence.
## Residual Risks
- Refresh=true runtime remains high at 261-481s per profile even though request counts are bounded and visible in metrics.
- Some lower-ranked candidates retain soft competition language not represented as structured competition facts, for example `pantsbuild/pants#13825`, `elastic/eui#7093`, and `pypa/hatch#2141`. This is an inherited ranking-quality risk, not introduced by V5 cache/budget changes.
- `default_cli_devtools` remains at visible=10, consistent with V4 after stricter competition filtering.
## Validation
- `ISSUE_FINDER_RECOMMENDATION_EVAL_REPORT_DIR=docs/recommendation-evals/2026-06-07-v5-performance-cache-budget/offline cargo test --test recommendation_eval`: passed.
- `cargo test`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo build`: passed.
- `git diff --check`: passed.
