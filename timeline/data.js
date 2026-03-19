window.BENCHMARK_DATA = {
  "lastUpdate": 1773907332204,
  "repoUrl": "https://github.com/deepjoy/taskmill",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "code@deepjoy.com",
            "name": "DJ Majumdar",
            "username": "deepjoy"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "5821a5005e3574142bb2fcfcc55d029e82c57ef5",
          "message": "fix(ci): bootstrap _benchmarks branch on first push to main (#53)\n\nThe github-action-benchmark step was failing with\n\"fatal: couldn't find remote ref _benchmarks\" because the branch had\nnever been created. Add a step that fetches the branch if it exists, or\ncreates it from HEAD on first run.",
          "timestamp": "2026-03-18T15:04:54Z",
          "tree_id": "ee68dc79409d25b5cb49b574c44f7d7ece247926",
          "url": "https://github.com/deepjoy/taskmill/commit/5821a5005e3574142bb2fcfcc55d029e82c57ef5"
        },
        "date": 1773847115710,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 12255531,
            "range": "± 441879",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 54849272,
            "range": "± 4020305",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 598885260,
            "range": "± 32208886",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 29095721,
            "range": "± 542711",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 63888808,
            "range": "± 1039713",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 136932720,
            "range": "± 5324530",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 26291982,
            "range": "± 577469",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 81757761,
            "range": "± 1498907",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 153259724,
            "range": "± 3518025",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 640774744,
            "range": "± 10157016",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 702750564,
            "range": "± 11784426",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 708880798,
            "range": "± 12532306",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 712771315,
            "range": "± 11873822",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 706847056,
            "range": "± 11130064",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 708996273,
            "range": "± 11434592",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "code@deepjoy.com",
            "name": "DJ Majumdar",
            "username": "deepjoy"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e97d72d38b4ff3eb4376c8c74442aaf0f83227ca",
          "message": "refactor: decompose internal god objects into focused, single-responsibility modules (#56)\n\n- **Decompose `spawn_task` god function** — break the 380-line function\ninto\n  focused submodules (`spawn/context.rs`, `spawn/completion.rs`,\n`spawn/failure.rs`, `spawn/parent.rs`) with an ~85-line orchestrator;\nDRY up\n  `ActiveTaskMap` bulk operations with a shared `drain_where` helper\n- **Decompose `TaskStore` into focused services** — move\ndependency-graph\noperations to `store/dependencies.rs`, consolidate\n`pop`/`complete`/`fail`\ninto `lifecycle/transitions.rs` so the state machine is visible in one\nplace\n- **Decompose `SubmitBuilder::resolve` precedence chain** — split into\n  `apply_prefix`, `apply_defaults`, `apply_module_scalar_defaults`, and\n`apply_overrides`; unify the duplicated typed/untyped module-defaults\nlogic\nNo external API surface change.",
          "timestamp": "2026-03-19T05:07:20Z",
          "tree_id": "d3f412640197cda28f38b5c1dcccd5116913fa97",
          "url": "https://github.com/deepjoy/taskmill/commit/e97d72d38b4ff3eb4376c8c74442aaf0f83227ca"
        },
        "date": 1773899141062,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 4011004,
            "range": "± 199521",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 45266068,
            "range": "± 3558596",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 575022924,
            "range": "± 31801849",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 20080983,
            "range": "± 281985",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 53882328,
            "range": "± 741895",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 123868567,
            "range": "± 1928482",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 16662414,
            "range": "± 348929",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 71457052,
            "range": "± 1316179",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 140066891,
            "range": "± 2366874",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 619448252,
            "range": "± 8196034",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 685512762,
            "range": "± 8795498",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 678999599,
            "range": "± 9258915",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 677747638,
            "range": "± 10305615",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 684477231,
            "range": "± 10256722",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 684783943,
            "range": "± 10455432",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 592895,
            "range": "± 15244",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 603688,
            "range": "± 4354",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 621502,
            "range": "± 6504",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 145626,
            "range": "± 580",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 359702,
            "range": "± 6808",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1290046,
            "range": "± 3564",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 973042,
            "range": "± 33303",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 980820,
            "range": "± 23449",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 1022744,
            "range": "± 18362",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 43,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/linear",
            "value": 75,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 185,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 452,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 579275757,
            "range": "± 7955972",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 303877235,
            "range": "± 6316129",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 305024480,
            "range": "± 5741198",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 306896263,
            "range": "± 5549942",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 306589024,
            "range": "± 4993746",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 175405015,
            "range": "± 4259812",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 229869025,
            "range": "± 7251670",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 1229566991,
            "range": "± 14001605",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 228428,
            "range": "± 4390",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 229668,
            "range": "± 5636",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 227853,
            "range": "± 5364",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 724946546,
            "range": "± 9223198",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 617030193,
            "range": "± 7711904",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 615304762,
            "range": "± 8555873",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 615550652,
            "range": "± 7507181",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 32268553,
            "range": "± 2404602",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 616336617,
            "range": "± 8295338",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 614407625,
            "range": "± 8336323",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 599553333,
            "range": "± 12571702",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 165620233,
            "range": "± 3927550",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 86593482,
            "range": "± 2808077",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 162529113,
            "range": "± 7757332",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 229382045,
            "range": "± 9845176",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 381323341,
            "range": "± 19776357",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1166401,
            "range": "± 141744",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9786182,
            "range": "± 1223566",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 68607264,
            "range": "± 4553190",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 124961,
            "range": "± 2564",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 212288,
            "range": "± 2997",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 607557,
            "range": "± 4976",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 130373,
            "range": "± 2542",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 188843,
            "range": "± 4205",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 456860,
            "range": "± 6279",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "41898282+github-actions[bot]@users.noreply.github.com",
            "name": "github-actions[bot]",
            "username": "github-actions[bot]"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3710bd4a15856d51666527d612086ade26ec2044",
          "message": "chore: release v0.5.1 (#50)\n\n## 🤖 New release\n\n* `taskmill`: 0.5.0 -> 0.5.1 (✓ API compatible changes)\n\n<details><summary><i><b>Changelog</b></i></summary><p>\n\n<blockquote>\n\n## [0.5.1](https://github.com/deepjoy/taskmill/compare/v0.5.0...v0.5.1)\n- 2026-03-19\n\n### Fixed\n\n- *(bench)* eliminate per-sample scheduler setup cost in history\nbenchmarks ([#55](https://github.com/deepjoy/taskmill/pull/55))\n- *(bench)* remove premature cancellation token call in history\nbenchmark setup ([#54](https://github.com/deepjoy/taskmill/pull/54))\n- *(ci)* bootstrap _benchmarks branch on first push to main\n([#53](https://github.com/deepjoy/taskmill/pull/53))\n- *(ci)* restore stderr capture for benchmark output on main\n([#51](https://github.com/deepjoy/taskmill/pull/51))\n- *(ci)* exclude lib target from cargo bench to fix benchmark CI\n([#49](https://github.com/deepjoy/taskmill/pull/49))\n\n### Other\n\n- decompose internal god objects into focused, single-responsibility\nmodules ([#56](https://github.com/deepjoy/taskmill/pull/56))\n- eliminate stringly-typed history status and DRY violations\n([#52](https://github.com/deepjoy/taskmill/pull/52))\n</blockquote>\n\n\n</p></details>\n\n---\nThis PR was generated with\n[release-plz](https://github.com/release-plz/release-plz/).\n\nCo-authored-by: github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>",
          "timestamp": "2026-03-19T05:22:20Z",
          "tree_id": "be5c39031e7d0cf08dc9cc93f3c8d5625a2fa55f",
          "url": "https://github.com/deepjoy/taskmill/commit/3710bd4a15856d51666527d612086ade26ec2044"
        },
        "date": 1773900065353,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 4055337,
            "range": "± 215683",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 46761990,
            "range": "± 3798029",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 595001779,
            "range": "± 31329239",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 20364360,
            "range": "± 266404",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 54661785,
            "range": "± 697894",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 123788145,
            "range": "± 2366456",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 16742728,
            "range": "± 304076",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 73672956,
            "range": "± 990191",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 144813406,
            "range": "± 3400174",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 632833252,
            "range": "± 8716679",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 698781473,
            "range": "± 11197024",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 696594890,
            "range": "± 10903465",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 700782286,
            "range": "± 10166203",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 700883392,
            "range": "± 12095033",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 705114806,
            "range": "± 9734973",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 598126,
            "range": "± 15661",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 612452,
            "range": "± 5338",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 626020,
            "range": "± 7692",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 149804,
            "range": "± 1209",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 367265,
            "range": "± 2253",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1314962,
            "range": "± 5456",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 992498,
            "range": "± 40427",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 1032073,
            "range": "± 28721",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 1053784,
            "range": "± 22809",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 44,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/linear",
            "value": 75,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 185,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 406,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 593550722,
            "range": "± 9104153",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 302905202,
            "range": "± 4848248",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 300063917,
            "range": "± 4731810",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 305816253,
            "range": "± 4936362",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 309003029,
            "range": "± 5363934",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 177803292,
            "range": "± 4275308",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 235947775,
            "range": "± 7116313",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 1248083940,
            "range": "± 14216927",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 231079,
            "range": "± 5056",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 231259,
            "range": "± 4594",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 232009,
            "range": "± 6447",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 748663595,
            "range": "± 11251721",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 627690434,
            "range": "± 9935683",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 630371628,
            "range": "± 9159498",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 630089309,
            "range": "± 9303328",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 33522934,
            "range": "± 2789656",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 624701840,
            "range": "± 9197231",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 626966044,
            "range": "± 9970243",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 618615204,
            "range": "± 8525145",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 169672599,
            "range": "± 3729823",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 89764726,
            "range": "± 2762185",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 163128237,
            "range": "± 6021909",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 235203097,
            "range": "± 10887394",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 391307204,
            "range": "± 19077466",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1168654,
            "range": "± 127346",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 10019773,
            "range": "± 1138497",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 53988641,
            "range": "± 5051561",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 127550,
            "range": "± 3058",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 216360,
            "range": "± 6727",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 611527,
            "range": "± 40616",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 135208,
            "range": "± 2838",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 195239,
            "range": "± 2752",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 467853,
            "range": "± 4684",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "code@deepjoy.com",
            "name": "DJ Majumdar",
            "username": "deepjoy"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "208f55b592e9243664cf6636894cacb26e86de9d",
          "message": "perf: reduce SQL round-trips in scheduler hot paths (#57)\n\n## Summary\n\n- **Batch dependency queries**: Replace per-dep iterative SQL lookups\n(history check, active check, edge insert) with single batched queries,\nand swap BFS cycle detection for a recursive CTE that resolves in one\nround-trip.\n- **Merge completion + dependency resolution into a single\ntransaction**: New `complete_with_record_and_resolve` combines the\n`complete_with_record` and `resolve_dependents` calls, eliminating a\n`BEGIN IMMEDIATE` / `COMMIT` cycle on every task completion.\n- **Add fast-path flags to skip unnecessary queries**:\n`has_paused_tasks` (atomic bool) skips the `paused_tasks()` query when\nnothing has been preempted; `has_tags` skips `populate_tags` when no\ntags exist; `check_scheduled` skips `next_run_after` when no scheduled\ntasks are present.\n- **Lightweight task claim**: New `claim_task` uses a simple `UPDATE …\nSET status='running'` without `RETURNING *`, patching the already-held\nin-memory `TaskRecord` instead of re-fetching the full row.\n- **Gate pprof behind optional `profile` feature**: Moves `pprof` from a\nmandatory dev-dependency to an optional feature, fixing CI builds on\nplatforms where pprof fails to compile.\n- **Add 0.4.x → 0.5.0 migration guide** covering the `Module` →\n`Domain<D>` API transition.",
          "timestamp": "2026-03-19T06:05:07Z",
          "tree_id": "09787bfa4be2c1af94feb122d8b407e7d6ae44eb",
          "url": "https://github.com/deepjoy/taskmill/commit/208f55b592e9243664cf6636894cacb26e86de9d"
        },
        "date": 1773902235126,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3338333,
            "range": "± 158932",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17117304,
            "range": "± 782541",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 76623834,
            "range": "± 3335743",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 14959249,
            "range": "± 166127",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 35776292,
            "range": "± 429576",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 70580546,
            "range": "± 946606",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 12356931,
            "range": "± 228993",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 50586471,
            "range": "± 534329",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 99368745,
            "range": "± 894848",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 451025952,
            "range": "± 5535679",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 505587180,
            "range": "± 5552310",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 505671552,
            "range": "± 6226503",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 512371837,
            "range": "± 7204179",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 512052724,
            "range": "± 5862260",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 511023318,
            "range": "± 6372218",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 621463,
            "range": "± 18967",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 612883,
            "range": "± 14227",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 622451,
            "range": "± 10405",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 144966,
            "range": "± 955",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 355951,
            "range": "± 1142",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1313781,
            "range": "± 2676",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 1050708,
            "range": "± 46344",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 1023151,
            "range": "± 30685",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 1075617,
            "range": "± 19739",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 47,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/linear",
            "value": 76,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 188,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 267,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 475689726,
            "range": "± 6878399",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 217155563,
            "range": "± 3143934",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 217545937,
            "range": "± 2952916",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 215927053,
            "range": "± 2967198",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 218665038,
            "range": "± 3385430",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 181496264,
            "range": "± 3950588",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 239151261,
            "range": "± 6991585",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 908512654,
            "range": "± 10353450",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 118393,
            "range": "± 2733",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 119277,
            "range": "± 3030",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 120361,
            "range": "± 3011",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 540059244,
            "range": "± 5249574",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 491207189,
            "range": "± 6036494",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 455592830,
            "range": "± 6697061",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 458465120,
            "range": "± 7005862",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 32811096,
            "range": "± 2351834",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 447200513,
            "range": "± 6824038",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 445555326,
            "range": "± 6773065",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 470967377,
            "range": "± 5733794",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 135368551,
            "range": "± 1911173",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 89602975,
            "range": "± 2785325",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 163981589,
            "range": "± 6593201",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 237989671,
            "range": "± 10472747",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 390385373,
            "range": "± 17836691",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1130492,
            "range": "± 146969",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9756532,
            "range": "± 1171186",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 52202108,
            "range": "± 1477368",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 129045,
            "range": "± 2590",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 215440,
            "range": "± 2474",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 610797,
            "range": "± 4742",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 135667,
            "range": "± 3098",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 194941,
            "range": "± 3398",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 456431,
            "range": "± 3485",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "code@deepjoy.com",
            "name": "DJ Majumdar",
            "username": "deepjoy"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0c2c28b73173b37ac9a422ac60f09a3cd8a25c6f",
          "message": "perf: coalesce task completions into batched transactions (#59)\n\n- Introduce a completion coalescing channel (`CompletionMsg`) so spawned\ntasks send completions to an unbounded MPSC channel instead of\nindividually committing to SQLite\n- The run loop drains the channel before each dispatch cycle via\n`drain_completions`, processing all queued completions in a single\n`BEGIN IMMEDIATE` / `COMMIT` transaction through the new\n`TaskStore::complete_batch_with_resolve` method\n- Spawned tasks use a leader-election pattern (`try_lock` on the shared\nreceiver) to opportunistically drain and process the batch inline,\nreducing latency under high concurrency\n- Concurrency slots are freed eagerly (module counter decrement + active\nmap removal) before the batch commits, so new work can be dispatched\nwithout waiting for the transaction\n- A final `drain_completions` call during shutdown ensures no\ncompletions are lost before the store closes",
          "timestamp": "2026-03-19T06:41:25Z",
          "tree_id": "bd8543cea8dfe48c30309e1412dcb5396abb6681",
          "url": "https://github.com/deepjoy/taskmill/commit/0c2c28b73173b37ac9a422ac60f09a3cd8a25c6f"
        },
        "date": 1773904296959,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 2840338,
            "range": "± 183705",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 14925795,
            "range": "± 1105072",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 64644672,
            "range": "± 3948670",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 12893886,
            "range": "± 223612",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 31380208,
            "range": "± 356479",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 62255092,
            "range": "± 723192",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 10857729,
            "range": "± 79760",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 45853973,
            "range": "± 431465",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 88897882,
            "range": "± 1413721",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 401644552,
            "range": "± 11277131",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 456878113,
            "range": "± 7832491",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 449289399,
            "range": "± 8474615",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 452141778,
            "range": "± 7016291",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 453879364,
            "range": "± 7655074",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 454337470,
            "range": "± 7755367",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 680828,
            "range": "± 14029",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 667086,
            "range": "± 14130",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 704281,
            "range": "± 10283",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 124692,
            "range": "± 1020",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 314205,
            "range": "± 1278",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1172929,
            "range": "± 4578",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 1178847,
            "range": "± 43432",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 1157449,
            "range": "± 24404",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 1237662,
            "range": "± 31873",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 50,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/linear",
            "value": 82,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 217,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 388,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 406878483,
            "range": "± 6154970",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 193025262,
            "range": "± 3654736",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 191987344,
            "range": "± 3712011",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 194947677,
            "range": "± 4087169",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 193813071,
            "range": "± 4484937",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 156318740,
            "range": "± 7299412",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 204454978,
            "range": "± 7564263",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 793316542,
            "range": "± 17851967",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 105236,
            "range": "± 3559",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 106564,
            "range": "± 3939",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 106426,
            "range": "± 5092",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 475362718,
            "range": "± 5005847",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 400590118,
            "range": "± 9297318",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 397439692,
            "range": "± 8524841",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 399298066,
            "range": "± 8483271",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 29966974,
            "range": "± 2361865",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 399178009,
            "range": "± 9819594",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 399581638,
            "range": "± 9473604",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 429498933,
            "range": "± 4248862",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 118659137,
            "range": "± 2336466",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 77560719,
            "range": "± 3958934",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 141449750,
            "range": "± 6246413",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 206413132,
            "range": "± 7615158",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 339033558,
            "range": "± 10537290",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1324535,
            "range": "± 58455",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 11663426,
            "range": "± 198555",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 65320778,
            "range": "± 4824922",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 110242,
            "range": "± 4244",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 181568,
            "range": "± 4681",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 515177,
            "range": "± 5857",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 117416,
            "range": "± 3585",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 176580,
            "range": "± 5542",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 439444,
            "range": "± 23470",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "code@deepjoy.com",
            "name": "DJ Majumdar",
            "username": "deepjoy"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "2ab7b573b24b41fd70caa3d1a0d5484b08bd00b5",
          "message": "perf: reduce SQL round-trips and CPU overhead in scheduler hot paths (#60)\n\n## Summary\n\n- Eliminate unnecessary SQL round-trips across the task dispatch and\ncompletion hot paths, cutting `dispatch_no_groups_500` latency by\n**~23%** (166ms → 128ms)\n- Replace generic `chrono` datetime parser with a fixed-position byte\nparser for the known SQLite format\n- Add `has_hierarchy` fast-path flag (following existing `has_tags`\npattern) to skip the `active_children_count` query when no parent-child\ntasks exist\n- Batch dependency resolution from 2+2N queries down to 2 using `DELETE\n… RETURNING` + single `UPDATE … RETURNING`\n- Add `fast_dispatch` mode that uses `pop_next()` (1 SQL) instead of\n`peek_next()` + gate + `claim_task()` (2 SQL) when no groups, pressure\nsources, or module caps are configured\n\n## Details\n\n### 1. Inline `last_insert_rowid` (`store/lifecycle/mod.rs`)\n`insert_history` was issuing a separate `SELECT last_insert_rowid()`\nquery after every INSERT into `task_history`. The `SqliteQueryResult`\nalready carries this value — use `result.last_insert_rowid()` directly,\nmatching the existing pattern in `complete_inner`.\n\n### 2. Fast datetime parsing (`store/row_mapping.rs`)\n`parse_datetime` was calling `chrono::NaiveDateTime::parse_from_str`\nwith a fallback — a generic parser invoked 2-4× per\n`row_to_task_record`. Replaced with a fixed-position byte parser that\nhandles both `\"YYYY-MM-DD HH:MM:SS\"` and `\"YYYY-MM-DD HH:MM:SS.fff…\"`.\n\n### 3. `has_hierarchy` flag (`store/mod.rs`,\n`scheduler/spawn/completion.rs`)\n`handle_success` was calling `active_children_count` (a `SELECT\nCOUNT(*)` query) for every task completion, even when no tasks use\nparent-child hierarchy. Added an `Arc<AtomicBool>` flag to `TaskStore` —\nset to `true` when a task with `parent_id` is submitted — and skip the\nquery when `false`. Follows the existing `has_tags` pattern exactly.\n\n### 4. Batched dependency resolution (`store/dependencies.rs`)\n`resolve_dependents_inner` previously used SELECT + DELETE +\nper-dependent COUNT + UPDATE (2+2N queries). Now uses `DELETE FROM\ntask_deps … RETURNING task_id` followed by a single `UPDATE tasks …\nWHERE id IN (…) AND NOT EXISTS (SELECT 1 FROM task_deps …) RETURNING id`\n— always 2 queries regardless of fan-out. Applied the same `DELETE …\nRETURNING` optimization to `fail_dependents_inner`.\n\n### 5. Fast dispatch path (`scheduler/run_loop.rs`,\n`scheduler/builder.rs`)\nWhen no groups, pressure sources, resource monitoring, or module caps\nare configured, `try_dispatch` now uses `pop_next()` (atomic\nUPDATE+RETURNING, 1 SQL) instead of `peek_next()` + `gate.admit()` +\n`claim_task()` (2 SQL). Added an expiry filter to `pop_next()`'s inner\nSELECT so expired tasks are safely skipped. The slow path is preserved\nunchanged as a fallback.\n\n## Benchmark results\n\n| Benchmark | Before | After | Change |\n|-----------|--------|-------|--------|\n| `dispatch_no_groups_500` | 166ms | 128ms | **-23%** |\n| `dispatch_one_group_500` | 235ms | 170ms | **-28%** |\n| `dispatch_group_scaling/100` | 234ms | 164ms | **-30%** |\n| `dep_fan_in_dispatch/50` | 17.7ms | 14.3ms | **-19%** |\n| `dep_fan_in_dispatch/100` | 34.5ms | 27.4ms | **-19%** |",
          "timestamp": "2026-03-19T07:33:10Z",
          "tree_id": "402d7f7a04c4be7f8a23ba179d05b034e84069ee",
          "url": "https://github.com/deepjoy/taskmill/commit/2ab7b573b24b41fd70caa3d1a0d5484b08bd00b5"
        },
        "date": 1773907331846,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3269057,
            "range": "± 172524",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16454727,
            "range": "± 1199981",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 73556934,
            "range": "± 3599663",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11412155,
            "range": "± 209140",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 27455089,
            "range": "± 489745",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 54361380,
            "range": "± 1690241",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 9185373,
            "range": "± 88219",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 37325498,
            "range": "± 962745",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 72153203,
            "range": "± 848351",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 327996967,
            "range": "± 6728212",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 429610076,
            "range": "± 8832780",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 429990080,
            "range": "± 6116220",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 431495217,
            "range": "± 7289730",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 430389213,
            "range": "± 11888670",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 429182056,
            "range": "± 9115413",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 533100,
            "range": "± 19935",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 539469,
            "range": "± 5678",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 552733,
            "range": "± 7797",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 145625,
            "range": "± 1706",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 370005,
            "range": "± 3439",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1365439,
            "range": "± 12678",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 873502,
            "range": "± 17598",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 881753,
            "range": "± 14674",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 915874,
            "range": "± 15419",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 46,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/linear",
            "value": 84,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 203,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 448,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 393676820,
            "range": "± 7268101",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 174941400,
            "range": "± 3816474",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 174574612,
            "range": "± 4806309",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 175241679,
            "range": "± 6626090",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 174649349,
            "range": "± 3056876",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 176921274,
            "range": "± 6243536",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 228940238,
            "range": "± 10411523",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 656289675,
            "range": "± 12783954",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 117385,
            "range": "± 4482",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 117915,
            "range": "± 3726",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 117195,
            "range": "± 6062",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 407838367,
            "range": "± 6342558",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 330616681,
            "range": "± 8854916",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 331313205,
            "range": "± 8078390",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 330097049,
            "range": "± 8140746",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 33352659,
            "range": "± 2337550",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 327247970,
            "range": "± 8382679",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 324102191,
            "range": "± 6075312",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 340027941,
            "range": "± 7466266",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 100609157,
            "range": "± 4053898",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 88003460,
            "range": "± 6680553",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 162900826,
            "range": "± 8485850",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 236097174,
            "range": "± 10597757",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 387087119,
            "range": "± 19819566",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1200845,
            "range": "± 139101",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9655726,
            "range": "± 817067",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 49611417,
            "range": "± 4246724",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 125273,
            "range": "± 5634",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 213405,
            "range": "± 8292",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 630124,
            "range": "± 8862",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 129949,
            "range": "± 4680",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 187870,
            "range": "± 4172",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 444909,
            "range": "± 13725",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}