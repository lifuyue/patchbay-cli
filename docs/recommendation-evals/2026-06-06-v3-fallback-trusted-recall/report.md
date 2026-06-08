# V3 Fallback C And Strong Trusted Recall

## Stage Summary

V3 implements Fallback C with strong trusted recall. The final run reaches the V3 visible-count hard pass for all six profiles, reaches the target count for four profiles, keeps top10 profile-fit leakage at zero, and passes the manual top5 gate for every profile.

Decision: `proceed`.

The stage still carries high competition-evidence and claim/PR risk into V4. That is expected by the roadmap: V4 owns timeline completion, claim/open-PR removal, and reranking after completion.

## Production Changes

- Added `data/discovery/profile-trusted-repositories.toml` with profile-specific trusted buckets for CLI/devtools, frontend, Rust/Go backend, Python data/CLI, AI agent tools, and DevOps/infra.
- Added `RepoTrustTier::ProfileTrusted`, profile bucket selection, and enrichment budget allocation that prioritizes overlay, profile trusted, GFI trusted, then global.
- Changed GitHub discovery so primary recall is trusted-first and primary global search is disabled for V3 ranking quality.
- Added profile trusted fallback and strong global fallback with the V3 API caps: 20 trusted repo issue-list requests, 3 fallback candidates per repo, 8 global fallback searches, 30 global fallback candidates, and 40 fallback enrichments.
- Added fallback C reranking in the recommendation engine: primary ranking runs first, trusted fallback fills toward `ceil(limit * 0.80)`, and global fallback is only hard-pass insurance.
- Added a small profile-fit adaptation proven by live data: AI/agent profiles now recognize MCP and Model Context Protocol evidence.

## Offline Metric Diff Against V2

Offline snapshot: `docs/recommendation-evals/2026-06-06-v3-fallback-trusted-recall/offline/`.

| metric | V2 | V3 | note |
| --- | ---: | ---: | --- |
| samples | 80 | 83 | +3 fixtures from live failure backfill |
| visible | 23 | 26 | +3 expected-visible trusted recall fixtures |
| precision@5 | 0.97 | 0.97 | unchanged |
| precision@10 | 0.97 | 0.97 | unchanged |
| reject leakage | 0 | 0 | hard pass |
| profile mismatch leakage | 0 | 0 | hard pass |
| stale high-rank leakage | 0 | 0 | hard pass |
| competition leakage | 0 | 0 | hard pass |
| dashboard noise leakage | 0 | 0 | hard pass |
| fallback fill rate | 1.00 | 1.00 | >= 0.70 goal passed |

Current offline summary:

| metric | value |
| --- | ---: |
| samples | 83 |
| visible | 26 |
| precision@5 | 0.97 |
| precision@10 | 0.97 |
| reject leakage | 0 |
| profile mismatch leakage | 0 |
| stale high-rank leakage | 0 |
| competition leakage | 0 |
| dashboard noise leakage | 0 |
| ranking inversions | 10 |
| feedback cooldown | 5/5 |
| fallback fill rate | 1.00 |
| fixture failures | 0 |

## Live 6 Profile Matrix

All live runs used `limit=15`, `refresh=true`, and `recordExposure=false` in isolated `ISSUE_FINDER_HOME` state. Final evidence sources were the six V3 run snapshots listed in `metrics.json`; `visible.jsonl` contains compact candidate rows without cache paths or tokens.

| profile | visible | hard pass >= 11 | target >= 12 |
| --- | ---: | --- | --- |
| default_cli_devtools | 14 | pass | target |
| typescript_frontend | 11 | pass | hard-pass only |
| rust_backend_systems | 14 | pass | target |
| python_data_cli | 15 | pass | target |
| ai_agent_tools | 11 | pass | hard-pass only |
| devops_infra | 13 | pass | target |

| profile | visible | top10 profileFit < 60 | top10 competition missing | top10 manual claim/PR risk | top5 good/excellent | all-visible good/excellent |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| default_cli_devtools | 14 | 0 | 10 | 2 | 4/5 (0.80) | 5/14 (0.36) |
| typescript_frontend | 11 | 0 | 9 | 2 | 4/5 (0.80) | 7/11 (0.64) |
| rust_backend_systems | 14 | 0 | 8 | 2 | 4/5 (0.80) | 5/14 (0.36) |
| python_data_cli | 15 | 0 | 9 | 1 | 4/5 (0.80) | 10/15 (0.67) |
| ai_agent_tools | 11 | 0 | 8 | 1 | 4/5 (0.80) | 7/11 (0.64) |
| devops_infra | 13 | 0 | 9 | 2 | 4/5 (0.80) | 5/13 (0.38) |

Global manual top5 good/excellent: 24/30 (0.80).

Global all-visible good/excellent observation: 39/78 (0.50). This is intentionally recorded as an observation metric, not a V3 hard blocker.

## Manual Review For All Visible Candidates

| profile | rank | issue | title | source tier | profile fit | manual quality | notes |
| --- | ---: | --- | --- | --- | ---: | --- | --- |
| default_cli_devtools | 1 | bitwarden/clients#20694 | 1password import does not archive "state: archived" entries | gfi_trusted | 100 | excellent | Concrete import bug with reproduction, expected/actual behavior, and contributor code-path confirmation. |
| default_cli_devtools | 2 | aws/aws-cdk#26935 | (aws-glue-alpha): (struct schema produces unsupported inputStrings) | gfi_trusted | 100 | good | Clear CDK Glue schema issue with concrete failing shape; needs alpha-module scoping but is actionable. |
| default_cli_devtools | 3 | astral-sh/uv#4988 | uv python list doesn't show all possible combinations of python installation | profile_trusted | 100 | weak | Strong CLI fit, but comments say much of the issue was closed or partially solved and needs revalidation. |
| default_cli_devtools | 4 | nushell/nushell#9065 | IPv6 LL addresses with link suffix (%eth0) don't work | profile_trusted | 82 | good | Concrete networking/parser bug with reproduction and maintainer-positive discussion. |
| default_cli_devtools | 5 | bitwarden/clients#16746 | Remove obsolete LockScreen config from Microsoft store installation | gfi_trusted | 83 | good | Small but concrete client packaging cleanup with clear target state. |
| default_cli_devtools | 6 | nushell/nushell#7792 | Error: nu::parser::extra_positional for "start msedge" | profile_trusted | 70 | weak | Profile-relevant parser edge case, but old and discussion leaves implementation path less crisp. |
| default_cli_devtools | 7 | djc/quinn#660 | StreamIds are allocated in the order in which they are polled, rather than the order of `open_uni`/`open_bi` | profile_trusted | 76 | good | Concrete QUIC stream ordering issue with enough context to investigate. |
| default_cli_devtools | 8 | diesel-rs/diesel#4840 | Improve the documentation of our derives | profile_trusted | 100 | weak | Relevant docs task, but derive-doc scope is broad and lower value. |
| default_cli_devtools | 9 | rust-lang/rustfmt#4896 | New config option for more control over block wrapping match arm bodies | profile_trusted | 76 | reject | Comments include a contributor saying they could take it and maintainer acceptance, so it is likely claimed. |
| default_cli_devtools | 10 | rust-lang/rustfmt#4483 | Enhance formatting for ConstBlock expressions | profile_trusted | 70 | reject | Mentoring/claim signals make this unsafe to recommend without competition completion. |
| default_cli_devtools | 11 | djc/quinn#1489 | [Bug] CI test failure for quinn-proto on i386 | profile_trusted | 98 | reject | Discussion includes go-ahead/PR-invitation signals; should be filtered after competition completion. |
| default_cli_devtools | 12 | rust-analyzer/rust-analyzer#1441 | Benchmark with different allocators | profile_trusted | 100 | weak | Old benchmark experiment with weak immediate value. |
| default_cli_devtools | 13 | bheisler/criterion.rs#435 | Text is cut off if the terminal width is not large enough | profile_trusted | 100 | weak | Old terminal-width bug with narrow value and limited current signal. |
| default_cli_devtools | 14 | bheisler/criterion.rs#508 | Avoiding warmup time and other delays when using Instruction counts rather than times | profile_trusted | 76 | weak | Old measurement-option issue; potentially useful but weakly scoped. |
| typescript_frontend | 1 | medusajs/medusa#15442 | [Bug]: Workflows are not being picked up by worker if the file is named index.[js,ts] | gfi_trusted | 100 | good | Concrete workflow/worker bug in TypeScript code with clear filename trigger. |
| typescript_frontend | 2 | rjsf-team/react-jsonschema-form#4483 | Does not remove empty objects on clearing input field value ending up with unexpected required value error | profile_trusted | 100 | good | Clear React form behavior bug with visible validation outcome. |
| typescript_frontend | 3 | rjsf-team/react-jsonschema-form#3183 | Bad performance for list render | profile_trusted | 100 | good | Profile-perfect React list rendering performance issue with reproduction context. |
| typescript_frontend | 4 | bpmn-io/bpmn-js#2371 | Alignment tools for labels don't work inside of participant/lane | profile_trusted | 100 | reject | Visible PR/assignment request exists; should not stay in top5 after V4. |
| typescript_frontend | 5 | bitwarden/clients#20694 | 1password import does not archive "state: archived" entries | profile_trusted | 100 | excellent | Strong user-visible import bug with steps and contributor implementation pointer. |
| typescript_frontend | 6 | bitwarden/clients#11406 | Import from LogMeOnce not working | profile_trusted | 100 | good | Concrete importer failure in a frontend/client repo. |
| typescript_frontend | 7 | apifytech/apify-js#2680 | Monitor mode | profile_trusted | 100 | good | Relevant TypeScript tooling feature with enough product context to scope. |
| typescript_frontend | 8 | appbaseio/reactivesearch#251 | Feature Request: Boilerplate middleware for whitelisting authorized actions | profile_trusted | 90 | weak | Frontend state-management request is relevant but old and underspecified. |
| typescript_frontend | 9 | appbaseio/reactivesearch#414 | Turn down sensitivity of URLParams | profile_trusted | 100 | good | Concrete URLParams sensitivity issue; old but implementation surface is narrow. |
| typescript_frontend | 10 | angular/components#29266 | docs-bug(Sidenav): deprecated methods used in the responsive sidenav example | profile_trusted | 100 | reject | Comment indicates someone wants to help; competition handling should remove it. |
| typescript_frontend | 11 | angular/angular#20371 | Possibility to add multiple updateOn events | profile_trusted | 100 | weak | Very old broad feature request; profile fit is high but recommendation value is weak. |
| rust_backend_systems | 1 | nushell/nushell#7792 | Error: nu::parser::extra_positional for "start msedge" | profile_trusted | 70 | weak | Rust CLI/parser edge case but stale and not as strong as later candidates. |
| rust_backend_systems | 2 | buildpacks/pack#2428 | Improve the error message when trying to save a builder in the docker daemon but the architecture doesn't match | profile_trusted | 98 | good | Concrete Go CLI/docker architecture error-message task with clear user impact. |
| rust_backend_systems | 3 | djc/quinn#660 | StreamIds are allocated in the order in which they are polled, rather than the order of `open_uni`/`open_bi` | profile_trusted | 88 | good | Good Rust systems bug with a concrete ordering behavior to verify. |
| rust_backend_systems | 4 | nushell/nushell#9065 | IPv6 LL addresses with link suffix (%eth0) don't work | profile_trusted | 70 | good | Concrete network URL parsing task with maintainer-positive signal. |
| rust_backend_systems | 5 | go-swagger/go-swagger#717 | Code gen: generate constants for validation parameters | profile_trusted | 100 | good | Actionable Go codegen enhancement in a backend/API tooling repo. |
| rust_backend_systems | 6 | diesel-rs/diesel#4840 | Improve the documentation of our derives | profile_trusted | 100 | weak | Relevant Rust docs improvement, but value and acceptance path are less strong. |
| rust_backend_systems | 7 | diesel-rs/diesel#4216 | Add support for currently unsupported postgres json/jsonb functions | profile_trusted | 100 | reject | Comments indicate contributors are already working on JSON functions. |
| rust_backend_systems | 8 | rust-lang/rustfmt#4483 | Enhance formatting for ConstBlock expressions | profile_trusted | 70 | reject | Claim/mentoring context means it should be filtered once competition is complete. |
| rust_backend_systems | 9 | go-swagger/go-swagger#872 | Improve readability of models as strings | profile_trusted | 88 | weak | Readable model strings are relevant but old and lower leverage. |
| rust_backend_systems | 10 | djc/quinn#1744 | quinn_udp build fails on DragonFlyBSD | profile_trusted | 100 | good | Concrete platform build failure with bounded investigation surface. |
| rust_backend_systems | 11 | rust-analyzer/rust-analyzer#1441 | Benchmark with different allocators | profile_trusted | 100 | weak | Old benchmark topic with low immediate execution signal. |
| rust_backend_systems | 12 | rust-analyzer/rust-analyzer#7950 | Consider expanding the benchmark suite | profile_trusted | 100 | weak | Old benchmark-suite expansion request with uncertain current need. |
| rust_backend_systems | 13 | bheisler/criterion.rs#508 | Avoiding warmup time and other delays when using Instruction counts rather than times | profile_trusted | 100 | weak | Old Criterion measurement workflow issue, visible but not strong. |
| rust_backend_systems | 14 | bheisler/criterion.rs#195 | Remove Unsafe Blocks | profile_trusted | 100 | weak | Very old cleanup request with weak current signal. |
| python_data_cli | 1 | pypa/hatch#2011 | "env show --json" fails with an error if some plugin is not installed | profile_trusted | 100 | good | Concrete Python packaging CLI bug with narrow trigger and clear fix area. |
| python_data_cli | 2 | astral-sh/uv#4988 | uv python list doesn't show all possible combinations of python installation | profile_trusted | 100 | weak | Useful Python tooling issue, but comments indicate partial closure and require revalidation. |
| python_data_cli | 3 | jupyter-server/jupyter_server#434 | 'port' moved from NotebookApp to ServerApp, but Jupyter ignores ServerApp.port | profile_trusted | 100 | good | Concrete Jupyter server config bug with reproduction and maintainer clarification. |
| python_data_cli | 4 | jupyter-server/jupyter_server#1008 | Shutting down the server started with password pre-set will encounter an error `tornado.httpclient.HTTPClientError: HTTP 403: Forbidden` | profile_trusted | 100 | good | Actionable Jupyter shutdown/auth bug with traceback and profile fit. |
| python_data_cli | 5 | PyTorchLightning/pytorch-lightning#9136 | AdvancedProfiler: ValueError: Attempting to stop recording an action (run_test_evaluation) which was never started. | profile_trusted | 100 | excellent | Detailed Python ML tooling bug with traceback, reproduction narrative, and strong profile fit. |
| python_data_cli | 6 | PyTorchLightning/pytorch-lightning#14063 | Schedule in PyTorchProfiler doesn't work | profile_trusted | 100 | good | Concrete PyTorch profiler schedule bug with enough context to investigate. |
| python_data_cli | 7 | pantsbuild/pants#14431 | Consider disallowing PEX_EXTRA_SYS_PATH in [test].extra_env_vars | profile_trusted | 100 | weak | Actionable Pants policy idea, but stale and more design-policy than bug fix. |
| python_data_cli | 8 | dask/dask#2682 | Add a tests for our tests helpers | profile_trusted | 100 | reject | Comment says a PR will be submitted; should be hidden after competition completion. |
| python_data_cli | 9 | pantsbuild/pants#15376 | Python DeprecationWarning is shown when using backslash in string literals in pants.toml | profile_trusted | 100 | good | Concrete Pants/Python deprecation warning with config-level reproduction. |
| python_data_cli | 10 | pypa/hatch#2069 | `hatch version` doesn't work on a `setuptools-scm` project unless `hatch-vcs` is installed | profile_trusted | 100 | good | Narrow Hatch versioning bug around setuptools-scm/hatch-vcs dependency behavior. |
| python_data_cli | 11 | astral-sh/uv#12244 | uv panics when resizing terminal window | profile_trusted | 100 | good | Concrete terminal-resize panic in a major Python CLI tool. |
| python_data_cli | 12 | pylint-dev/pylint#8256 | Unnecessary use of a comprehension: Wrong fix | profile_trusted | 82 | good | Specific Pylint autofix bug with bounded code path. |
| python_data_cli | 13 | pylint-dev/pylint#5793 | arguments-differ: number of parameters was some number ... and is now the same number in overridden ... | profile_trusted | 82 | good | Concrete Pylint false positive/typing behavior issue with narrow scope. |
| python_data_cli | 14 | ipython/ipython#9874 | test on pypy once pypy implements supported Python 3 | profile_trusted | 100 | weak | Old PyPy test coverage request; relevant but not a strong recommendation. |
| python_data_cli | 15 | dask/dask#9393 | Is there a good way to issue a warning from a task? | profile_trusted | 100 | reject | Comments include intent to take a shot and maintainer expectation of a PR. |
| ai_agent_tools | 1 | CodeGraphContext/CodeGraphContext#1115 | API: Catch Connection Cancellation on Client Disconnects | profile_trusted | 100 | good | Fresh MCP server bug with clear file path and acceptance criterion. |
| ai_agent_tools | 2 | vllm-project/vllm-omni#4077 | [RFC]: Support Model FLOPs Utilization (MFU) Metrics for Diffusion Models (DiTs) | profile_trusted | 100 | reject | Comment asks to be assigned; V4 competition completion should remove this from top5. |
| ai_agent_tools | 3 | langchain-ai/langchain#30924 | openai: intermittent `LengthFinishReasonError` in `AzureChatOpenAI` | profile_trusted | 100 | good | Strong AI integration bug with self-contained AzureChatOpenAI repro, though older and high-triage. |
| ai_agent_tools | 4 | rhesis-ai/rhesis#1867 | Add Pydantic AI integration (auto-instrumentation + Penelope Target) | profile_trusted | 100 | good | Well-scoped Pydantic AI instrumentation and target integration task. |
| ai_agent_tools | 5 | rhesis-ai/rhesis#1872 | Add GitLab MCP tool integration | profile_trusted | 100 | good | Good MCP/GitLab provider task; one implementation-plan comment should be rechecked by V4. |
| ai_agent_tools | 6 | latitude-dev/latitude-llm#3425 | [Feature]: AutoGen integration | profile_trusted | 100 | good | Clear AutoGen telemetry integration gap in an AI observability repo. |
| ai_agent_tools | 7 | latitude-dev/latitude-llm#3401 | [Feature]: Python SDK for Latitude | profile_trusted | 100 | weak | Strong AI fit but broad SDK feature scope needs decomposition. |
| ai_agent_tools | 8 | agenta-ai/agenta#4549 | Fix SDK eval breadcrumbs showing `Auto Evals` | profile_trusted | 100 | good | Small concrete LLM eval product bug with screenshot context. |
| ai_agent_tools | 9 | containers/ramalama#1783 | Allow passing additional config files (e.g. chat templates) into the model container | profile_trusted | 97 | good | Concrete local AI model/container UX task with maintainer discussion and implementation lead. |
| ai_agent_tools | 10 | containers/ramalama#801 | unable to run ramalama using --runtime vllm on macOS | profile_trusted | 97 | weak | AI runtime issue is relevant, but discussion points partly outside RamaLama ownership. |
| ai_agent_tools | 11 | LMCache/LMCache#1826 | [RFC] LMCache & Agentic Application/Benchmark/Workflow Traces | profile_trusted | 100 | weak | Agentic benchmark tracker is profile-perfect but broad and partially occupied by maintainers. |
| devops_infra | 1 | rook/rook#17285 | rook-config-override without trailing newline can crash new Ceph daemons on Ceph 19.2.3 | profile_trusted | 100 | excellent | Strong Kubernetes/Ceph operator bug with exact failure mode and expected behavior. |
| devops_infra | 2 | kubernetes-sigs/kind#3834 | Swap stats is not shown as part of the metrics/resource endpoint | profile_trusted | 100 | good | Concrete Kubernetes node metrics bug with command output and maintainer discussion. |
| devops_infra | 3 | linkerd/linkerd2#5219 | EOF when running linkerd tap command against a category of resources on a k3d cluster | profile_trusted | 100 | good | Concrete Linkerd/k3d tap bug; old, but profile fit and debugging path are clear. |
| devops_infra | 4 | kubevirt/kubevirt#5932 | k8s enables structured logging more and more, consider switching to their structured logger | profile_trusted | 98 | reject | Contains /assign and stale workflow comments; should be removed by competition completion. |
| devops_infra | 5 | linkerd/linkerd2#1960 | linkerd2 web does not support relative paths | profile_trusted | 100 | good | Old but clear Linkerd relative-path support bug with user demand. |
| devops_infra | 6 | GoogleContainerTools/skaffold#7368 | Skaffold & local docker daemon registry mirror | profile_trusted | 100 | good | Actionable Skaffold/docker registry mirror bug with detailed context. |
| devops_infra | 7 | GoogleContainerTools/skaffold#4898 | [sync] dockerfile infer sync mode not working as expected. | profile_trusted | 100 | weak | Old sync inference issue; maintainer suggested a workaround and follow-up is unclear. |
| devops_infra | 8 | cilium/cilium#45231 | Policy status operator misses disallowed namespace endpoint selector | profile_trusted | 76 | reject | Comments reference multiple contributors and PR activity; unsafe until V4 completion. |
| devops_infra | 9 | cert-manager/cert-manager#2334 | Add network policy allowance into documentation | profile_trusted | 88 | weak | NetworkPolicy docs task may already be covered by a PR and is stale. |
| devops_infra | 10 | argoproj/argo-cd#14638 | Optionally add `app.kubernetes.io/managed-by` label to all resources | profile_trusted | 90 | weak | Useful GitOps label request but broad and with manual workaround discussion. |
| devops_infra | 11 | cert-manager/cert-manager#3103 | Adding probes to the cert-manager pods | profile_trusted | 100 | weak | Probe support is relevant, but old and broad chart-design scope. |
| devops_infra | 12 | kubernetes-sigs/kind#1175 | proxy info from docker config is not respected | profile_trusted | 100 | reject | A PR is referenced in comments; should not remain visible after V4. |
| devops_infra | 13 | buildpacks/pack#2470 | Address breaking changes to go packages in docker 29.0.0 release | profile_trusted | 94 | weak | Docker 29 migration is partly handled already and remaining scope is broad. |

## Failure Examples And Fixes

- AI/agent tools initially underfilled because MCP issues did not count as AI/agent profile evidence and the trusted repo bucket did not include the best MCP-specific sources. The final run added `CodeGraphContext/CodeGraphContext` and `rhesis-ai/rhesis`, recognized MCP evidence, and reached 11 visible candidates. The representative offline backfill is `ai_mcp_tool_integration_good`.
- Python data/CLI initially reached only 9 visible candidates. The final run added stronger trusted Python tooling sources, including Pants, Hatch, and Jupyter Server, then reached 15 visible candidates. Representative offline backfills are `python_pants_skip_warning_good` and `python_hatch_optional_uv_good`.
- Manual review still found claim/PR risks in visible rows, for example `vllm-project/vllm-omni#4077`, `bpmn-io/bpmn-js#2371`, `rust-lang/rustfmt#4483`, `dask/dask#2682`, `kubevirt/kubevirt#5932`, and `kubernetes-sigs/kind#1175`. These are carried into V4 because V4 owns completion of comments/timeline evidence and reranking after competition facts are known.

## Fixture Additions

- Added `profile_trusted` to the recommendation eval schema source tier enum.
- Added `ai_mcp_tool_integration_good` to `profile_ai_agent_tools.json` for MCP profile-fit and trusted fallback recall.
- Added `python_pants_skip_warning_good` and `python_hatch_optional_uv_good` to `profile_python_data_cli.json` for Python trusted fallback recall.
- Increased the recommendation eval fixture sample count to 83.

## Stage Goal Check

| V3 live goal | result |
| --- | --- |
| `rust_backend_systems visible >= 11` | pass: 14 |
| `devops_infra visible >= 11` | pass: 13 |
| all six profiles visible >= 11 | pass |
| target visible >= 12 for at least four profiles | pass: 4 profiles |
| top10 profileFit < 60 <= 1 | pass: max 0 |
| manual top5 good/excellent >= 80% for each profile | pass: min 0.80 |
| manual top5 good/excellent >= 80% globally | pass: 0.80 |

## Validation

- `ISSUE_FINDER_RECOMMENDATION_EVAL_REPORT_DIR=docs/recommendation-evals/2026-06-06-v3-fallback-trusted-recall/offline cargo test --test recommendation_eval`: passed
- `cargo test`: passed
- `cargo clippy --all-targets -- -D warnings`: passed
- `cargo build`: passed
- `git diff --check`: passed

## Decision

Decision: `proceed`.

V3 recall and profile-fit goals are met. Proceed to V4 with the carried risk that competition evidence is still incomplete for many top10 rows and manual review found claim/PR signals that require the V4 competition completion pass.
