window.BENCHMARK_DATA = {
  "lastUpdate": 1774405221709,
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
          "id": "c415ee41c6d865b29b89450093e667fdd330e7fe",
          "message": "chore: release v0.5.2 (#58)\n\n## 🤖 New release\n\n* `taskmill`: 0.5.1 -> 0.5.2 (✓ API compatible changes)\n\n<details><summary><i><b>Changelog</b></i></summary><p>\n\n<blockquote>\n\n## [0.5.2](https://github.com/deepjoy/taskmill/compare/v0.5.1...v0.5.2)\n- 2026-03-19\n\n### Other\n\n- reduce SQL round-trips and CPU overhead in scheduler hot paths\n([#60](https://github.com/deepjoy/taskmill/pull/60))\n- coalesce task completions into batched transactions\n([#59](https://github.com/deepjoy/taskmill/pull/59))\n- reduce SQL round-trips in scheduler hot paths\n([#57](https://github.com/deepjoy/taskmill/pull/57))\n</blockquote>\n\n\n</p></details>\n\n---\nThis PR was generated with\n[release-plz](https://github.com/release-plz/release-plz/).\n\nCo-authored-by: github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>",
          "timestamp": "2026-03-19T07:37:19Z",
          "tree_id": "e1d42eda08d157f012a88959d22e0163e4321c5b",
          "url": "https://github.com/deepjoy/taskmill/commit/c415ee41c6d865b29b89450093e667fdd330e7fe"
        },
        "date": 1773907560250,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3250025,
            "range": "± 147886",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16698137,
            "range": "± 751122",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 74436341,
            "range": "± 3652639",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11518792,
            "range": "± 182356",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 27514295,
            "range": "± 451643",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 54356039,
            "range": "± 1121166",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 8919558,
            "range": "± 132326",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 36054992,
            "range": "± 482802",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 69217751,
            "range": "± 621001",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 315641229,
            "range": "± 4559686",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 419101816,
            "range": "± 5276121",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 425674853,
            "range": "± 6036850",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 430246698,
            "range": "± 4796778",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 429592929,
            "range": "± 4358310",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 429894456,
            "range": "± 5445198",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 534046,
            "range": "± 13938",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 540736,
            "range": "± 7763",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 554550,
            "range": "± 11134",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 143987,
            "range": "± 470",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 354228,
            "range": "± 2918",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1279432,
            "range": "± 4332",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 876410,
            "range": "± 24508",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 887594,
            "range": "± 44712",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 897749,
            "range": "± 29162",
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
            "value": 190,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 408,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 382749483,
            "range": "± 6907308",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 164370292,
            "range": "± 3126223",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 167377570,
            "range": "± 3199468",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 167469288,
            "range": "± 3360741",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 166525568,
            "range": "± 3470194",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 174605471,
            "range": "± 5944640",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 232278841,
            "range": "± 8337907",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 636898971,
            "range": "± 9503098",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 118546,
            "range": "± 2997",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 118747,
            "range": "± 3675",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 120024,
            "range": "± 2836",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 399151752,
            "range": "± 3991731",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 320736859,
            "range": "± 4519689",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 321336486,
            "range": "± 5081495",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 317623759,
            "range": "± 4368021",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 32322082,
            "range": "± 2407148",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 316990822,
            "range": "± 4759400",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 318292785,
            "range": "± 4969385",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 337645721,
            "range": "± 4424748",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 101218857,
            "range": "± 1834221",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 86820174,
            "range": "± 3067914",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 157848266,
            "range": "± 5925647",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 231204937,
            "range": "± 11825440",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 379932217,
            "range": "± 18582209",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1126763,
            "range": "± 138618",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9631227,
            "range": "± 1220023",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 49641551,
            "range": "± 5402233",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 125700,
            "range": "± 2640",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 216800,
            "range": "± 3794",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 607591,
            "range": "± 3796",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 130558,
            "range": "± 3728",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 191125,
            "range": "± 3966",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 459024,
            "range": "± 3825",
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
          "id": "659c2d52f3fbd7f21614ace75fbdb8e61568069a",
          "message": "perf: batch dispatch and completion SQL to reduce round-trips (~56% faster) (#61)\n\n## Summary\n\n- Add `pop_next_batch(limit)` to `TaskStore` that claims up to N pending\ntasks in a single `UPDATE…RETURNING…LIMIT` statement, replacing the\nsequential one-at-a-time `pop_next()` loop when filling concurrency\nslots\n- Add `dispatch_pending()` fast path in the scheduler run loop that\nfills all available concurrency slots in one SQL round-trip (falls back\nto per-task gate-checked dispatch on the slow path)\n- Thread a `skip_tags` flag through `insert_history()` and\n`complete_inner()` so that when the store's `has_tags` flag is `false`,\nthe no-op history-tag copy INSERT and `delete_task_tags` DELETE are\nskipped entirely (2 SQL round-trips saved per task completion)\n- Restructure `complete_batch_with_resolve` for non-recurring batches to\nuse single batched `DELETE FROM tasks WHERE id IN (…)`, `DELETE FROM\ntask_tags`, and `DELETE FROM task_deps` statements instead of per-task\nloops, reducing SQL from ~5N to N+4 per batch\n\n## Benchmark results\n\n| Benchmark | Before | After | Change |\n|---|---|---|---|\n| `dispatch_and_complete_1000` | ~450 ms | ~200 ms | **-56%** |\n| `concurrency_scaling/1` | ~310 ms | ~146 ms | **-50%** |\n| `concurrency_scaling/2` | ~227 ms | ~113 ms | **-51%** |\n| `concurrency_scaling/4` | ~240 ms | ~95 ms | **-59%** |\n| `concurrency_scaling/8` | ~274 ms | ~88 ms | **-68%** |\n| `mixed_priority_dispatch_500` | ~241 ms | ~93 ms | **-60%** |\n| `submit_1000_tasks` | ~60 ms | ~60 ms | no change |\n| `batch_submit_1000` | ~13 ms | ~13 ms | no change |",
          "timestamp": "2026-03-19T08:09:26Z",
          "tree_id": "d6909d856eadafa873365af64a0c165ebbee8f5e",
          "url": "https://github.com/deepjoy/taskmill/commit/659c2d52f3fbd7f21614ace75fbdb8e61568069a"
        },
        "date": 1773909487777,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3659094,
            "range": "± 650810",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 18635003,
            "range": "± 1895345",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 84904226,
            "range": "± 8091225",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 12269617,
            "range": "± 1006727",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 29958230,
            "range": "± 1584318",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 58771757,
            "range": "± 2869834",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7941395,
            "range": "± 543185",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 30635679,
            "range": "± 1960676",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 58974806,
            "range": "± 4403933",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 268771371,
            "range": "± 20109097",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 489113876,
            "range": "± 34566160",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 463029981,
            "range": "± 26385673",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 462963278,
            "range": "± 27187584",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 463040296,
            "range": "± 26543090",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 462044228,
            "range": "± 25052627",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 556879,
            "range": "± 14523",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 579000,
            "range": "± 9970",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 578087,
            "range": "± 11029",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 159027,
            "range": "± 4961",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 372924,
            "range": "± 8447",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1328324,
            "range": "± 13028",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 912058,
            "range": "± 34638",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 904220,
            "range": "± 31210",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 938193,
            "range": "± 31346",
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
            "value": 78,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 190,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 408,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 400252124,
            "range": "± 25999820",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 161836229,
            "range": "± 13470753",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 161106600,
            "range": "± 10115506",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 161310975,
            "range": "± 9800715",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 161907796,
            "range": "± 11307360",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 194165396,
            "range": "± 16804122",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 255267796,
            "range": "± 19160428",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 530339738,
            "range": "± 32829533",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 127346,
            "range": "± 17569",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 125634,
            "range": "± 12464",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 126452,
            "range": "± 9087",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 403871546,
            "range": "± 22413519",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 337522521,
            "range": "± 22485900",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 294169796,
            "range": "± 19265549",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 265082644,
            "range": "± 19223367",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 36883721,
            "range": "± 4596435",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 296951856,
            "range": "± 20123382",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 266287161,
            "range": "± 17358558",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 268816509,
            "range": "± 19827790",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 89843788,
            "range": "± 7387370",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 96931138,
            "range": "± 7712612",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 177548351,
            "range": "± 14198076",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 256971170,
            "range": "± 17064649",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 428473289,
            "range": "± 29723601",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1161909,
            "range": "± 94068",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9716630,
            "range": "± 1489515",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 53379583,
            "range": "± 2413290",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 139895,
            "range": "± 13361",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 226914,
            "range": "± 15455",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 633715,
            "range": "± 16421",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 142296,
            "range": "± 10304",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 205663,
            "range": "± 16754",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 473852,
            "range": "± 13186",
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
          "id": "b1f081d69a78891ef702181554bbfa9bd06ac2ae",
          "message": "chore: release v0.5.3 (#62)\n\n## 🤖 New release\n\n* `taskmill`: 0.5.2 -> 0.5.3 (✓ API compatible changes)\n\n<details><summary><i><b>Changelog</b></i></summary><p>\n\n<blockquote>\n\n## [0.5.3](https://github.com/deepjoy/taskmill/compare/v0.5.2...v0.5.3)\n- 2026-03-19\n\n### Other\n\n- batch dispatch and completion SQL to reduce round-trips (~56% faster)\n([#61](https://github.com/deepjoy/taskmill/pull/61))\n</blockquote>\n\n\n</p></details>\n\n---\nThis PR was generated with\n[release-plz](https://github.com/release-plz/release-plz/).\n\nCo-authored-by: github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>",
          "timestamp": "2026-03-19T08:12:40Z",
          "tree_id": "b55db17f307426b2de410ea5e12ccbcd40707370",
          "url": "https://github.com/deepjoy/taskmill/commit/b1f081d69a78891ef702181554bbfa9bd06ac2ae"
        },
        "date": 1773909515407,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 2979296,
            "range": "± 161286",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 15290396,
            "range": "± 848926",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 67554891,
            "range": "± 3131005",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 10053133,
            "range": "± 64663",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 24456719,
            "range": "± 472698",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 48179658,
            "range": "± 704260",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 6463722,
            "range": "± 92779",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 24619410,
            "range": "± 337042",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 47013915,
            "range": "± 657175",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 218371178,
            "range": "± 3687026",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 374008726,
            "range": "± 7704895",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 372241551,
            "range": "± 6672779",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 375665561,
            "range": "± 5616099",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 375190571,
            "range": "± 6538234",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 373400604,
            "range": "± 7953727",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 639292,
            "range": "± 9434",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 640453,
            "range": "± 8108",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 704263,
            "range": "± 7581",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 132886,
            "range": "± 1104",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 322348,
            "range": "± 1166",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1174716,
            "range": "± 4521",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 1079533,
            "range": "± 21465",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 1098319,
            "range": "± 18921",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 1168210,
            "range": "± 21698",
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
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 334996341,
            "range": "± 4893334",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 137221610,
            "range": "± 1519722",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 137342686,
            "range": "± 1492449",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 139285096,
            "range": "± 3020000",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 138263647,
            "range": "± 1896184",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 167627670,
            "range": "± 6375363",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 215295003,
            "range": "± 6033853",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 441781452,
            "range": "± 9291320",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 114883,
            "range": "± 2978",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 116583,
            "range": "± 3598",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 115454,
            "range": "± 4229",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 352845515,
            "range": "± 4316886",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 283148826,
            "range": "± 3481013",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 254003238,
            "range": "± 4265243",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 220517275,
            "range": "± 3528238",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 30045721,
            "range": "± 1755685",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 251280646,
            "range": "± 4456784",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 219849099,
            "range": "± 4166518",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 222123065,
            "range": "± 4147664",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 74027505,
            "range": "± 2017743",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 82888434,
            "range": "± 3740515",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 146647469,
            "range": "± 6074731",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 213560677,
            "range": "± 8067614",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 344716569,
            "range": "± 12063889",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1366819,
            "range": "± 77506",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 11695321,
            "range": "± 1165100",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 63567687,
            "range": "± 5115497",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 118548,
            "range": "± 4392",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 197555,
            "range": "± 16133",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 528316,
            "range": "± 5904",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 126832,
            "range": "± 4848",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 186052,
            "range": "± 5172",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 456849,
            "range": "± 3854",
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
          "id": "b14b5b6d0c4ae6df579bb3faf27420543ea74576",
          "message": "refactor: migrate all tests/benches from TaskExecutor to TypedExecutor (#66)\n\n## Summary\n\n- Replace all usages of the untyped `TaskExecutor` trait with\n`TypedExecutor<T>` across every test, benchmark, and example, enforcing\ncompile-time type safety for task payloads\n- Introduce a `define_task!` macro in integration test helpers to reduce\nboilerplate when declaring zero-payload `TypedTask` types\n- Update `TaskContext::payload()` to fall back to deserializing from\nJSON `null`, allowing unit-struct typed tasks (e.g. `struct Noop;`)\nsubmitted via raw `TaskSubmission::new(...)` to still resolve correctly\n- Migrate scheduler setup in tests from manual `Scheduler::new()` +\n`TaskTypeRegistry` construction to the `Scheduler::builder()` +\n`Domain::task::<T>()` API, matching the recommended public API surface\n\n## Changed files\n\n- **`src/registry/context.rs`** — `payload<T>()` now falls back to\n`serde_json::from_value(Null)` when no payload blob is stored\n- **`src/registry/mod.rs`** — unit tests use `Domain::task()` →\n`into_module()` to produce erased executors instead of directly\nconstructing `Arc<dyn ErasedExecutor>` from `TaskExecutor`\n- **`src/module.rs`** — module-level test updated to use `Domain` +\n`TypedExecutor`\n- **`src/scheduler/tests.rs`** — all ~40 scheduler tests rewritten:\ntyped domain/task structs, `Scheduler::builder()`, fully-qualified task\ntype strings (`\"test::test\"`, `\"parent::parent\"`, etc.)\n- **`tests/integration/common.rs`** — `define_task!` macro + ~20 shared\ntyped task definitions; `NoopExecutor`, `DelayExecutor`,\n`CountingExecutor` now implement `TypedExecutor<T>` generically\n- **`tests/integration/*.rs`** — all integration tests migrated to typed\nAPI\n- **`benches/*.rs`** — all benchmarks migrated to typed API\n- **`examples/profile_dep_chain.rs`** — migrated to typed API",
          "timestamp": "2026-03-21T00:05:21Z",
          "tree_id": "ff01de71eb5a742e67f72a5561eb865dddbd0657",
          "url": "https://github.com/deepjoy/taskmill/commit/b14b5b6d0c4ae6df579bb3faf27420543ea74576"
        },
        "date": 1774053216126,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3481882,
            "range": "± 174932",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17665646,
            "range": "± 1118171",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 78244123,
            "range": "± 4911237",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11896212,
            "range": "± 133445",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 28842423,
            "range": "± 473955",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 56869098,
            "range": "± 1063012",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7770376,
            "range": "± 109054",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 29425995,
            "range": "± 723365",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 56160811,
            "range": "± 1101660",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 258142442,
            "range": "± 5037696",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 439462097,
            "range": "± 6748312",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 446093718,
            "range": "± 7423741",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 444659237,
            "range": "± 8817834",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 444829425,
            "range": "± 7620337",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 445538822,
            "range": "± 7233689",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 544775,
            "range": "± 15877",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 562187,
            "range": "± 11440",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 577244,
            "range": "± 10616",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 150856,
            "range": "± 1733",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 364500,
            "range": "± 1330",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1298362,
            "range": "± 2479",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 880926,
            "range": "± 16752",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 913007,
            "range": "± 27523",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 928561,
            "range": "± 36535",
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
            "value": 78,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 190,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 407,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 390025367,
            "range": "± 5036470",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 158870284,
            "range": "± 1765171",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 159443263,
            "range": "± 1840191",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 159854120,
            "range": "± 2225937",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 161002688,
            "range": "± 2230747",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 193607225,
            "range": "± 7681800",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 249149133,
            "range": "± 12399466",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 519555348,
            "range": "± 9707047",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 126500,
            "range": "± 5529",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 125194,
            "range": "± 4780",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 126301,
            "range": "± 3914",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 398478634,
            "range": "± 7684428",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 335101716,
            "range": "± 5089061",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 288082630,
            "range": "± 6180326",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 259139825,
            "range": "± 5422784",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 35479621,
            "range": "± 3604394",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 289655567,
            "range": "± 5715566",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 261277533,
            "range": "± 5832386",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 263888918,
            "range": "± 6095671",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 87159589,
            "range": "± 2667995",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 92361035,
            "range": "± 4866380",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 170853791,
            "range": "± 10122552",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 246432275,
            "range": "± 16106955",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 408291043,
            "range": "± 23400740",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1142271,
            "range": "± 158465",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9642783,
            "range": "± 1483963",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 48197334,
            "range": "± 5616191",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 132532,
            "range": "± 5919",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 220817,
            "range": "± 6131",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 609630,
            "range": "± 6859",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 134223,
            "range": "± 5275",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 194192,
            "range": "± 5903",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 459707,
            "range": "± 6886",
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
          "id": "0a608a5dd64b03c4d95678327ba7ef7e3958fa31",
          "message": "feat!: replace `&TaskContext` with `DomainTaskContext<D>` for type-safe child spawning (#68)\n\n## Summary\n\n- Introduce `DomainTaskContext<'a, D>`, a zero-cost wrapper\n(`&TaskContext` + `PhantomData<D>`) that carries domain identity as a\ntype parameter, enabling compile-time–safe child spawning\n- Add `ChildSpawnBuilder` with `.key()` / `.priority()` / `.ttl()` /\n`.group()` overrides and `IntoFuture` support\n- Add `DomainSubmitBuilder::child_of(&ctx)` for ergonomic cross-domain\nchild creation\n- Change `TypedExecutor<T>` trait: `ctx` parameter is now\n`DomainTaskContext<'a, T::Domain>` instead of `&'a TaskContext`\n- Remove `TaskExecutor`, `TaskContext`, `Domain::raw_executor()`, and\n`DomainHandle::submit_raw()` from the public API\n- Fix `DomainSubmitBuilder::on_dependency_failure` which was previously\na no-op\n- Migrate all executor impls, `spawn_child` call sites, and `submit_raw`\ncall sites across tests, benches, and examples\n\n## Breaking changes\n\n| Before | After |\n|---|---|\n| `ctx: &'a TaskContext` | `ctx: DomainTaskContext<'a, T::Domain>` |\n| `ctx.spawn_child(TaskSubmission::new(\"name\").payload_json(&t)?)` |\n`ctx.spawn_child_with(t).key(\"name\").await?` |\n| `ctx.domain::<D>().submit_with(t).parent(ctx.record().id)` |\n`ctx.domain::<D>().submit_with(t).child_of(&ctx)` |\n| `domain.raw_executor(\"name\", exec)` | removed — use\n`Domain::task::<T>(exec)` |\n| `handle.submit_raw(TaskSubmission::new(...))` | removed — use\n`handle.submit_with(typed_task)` |\n| `pub use registry::{TaskContext, TaskExecutor}` | removed from public\nAPI |",
          "timestamp": "2026-03-21T09:07:52Z",
          "tree_id": "242aaeaad3783d2dad6f4b9cc74e0baa035031a7",
          "url": "https://github.com/deepjoy/taskmill/commit/0a608a5dd64b03c4d95678327ba7ef7e3958fa31"
        },
        "date": 1774085768377,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3520225,
            "range": "± 189662",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17863963,
            "range": "± 1053160",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 78259500,
            "range": "± 5343532",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11982430,
            "range": "± 113537",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 28742390,
            "range": "± 410722",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 57131580,
            "range": "± 1182298",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7799467,
            "range": "± 94692",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 29491448,
            "range": "± 721133",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 55979239,
            "range": "± 1183895",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 257702583,
            "range": "± 5710680",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 444206056,
            "range": "± 7204841",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 441453551,
            "range": "± 6827763",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 443312936,
            "range": "± 8028652",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 441401140,
            "range": "± 8764572",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 441528806,
            "range": "± 7732239",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 545411,
            "range": "± 15497",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 556487,
            "range": "± 5590",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 567372,
            "range": "± 7186",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 151294,
            "range": "± 1118",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 359314,
            "range": "± 774",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1265682,
            "range": "± 2539",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 896198,
            "range": "± 25071",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 904578,
            "range": "± 25687",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 915303,
            "range": "± 25373",
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
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 391133687,
            "range": "± 5544136",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 160000235,
            "range": "± 1760580",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 160102782,
            "range": "± 2032905",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 160143793,
            "range": "± 2323703",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 160132194,
            "range": "± 7522613",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 189480655,
            "range": "± 7945796",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 246452581,
            "range": "± 14261794",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 514960577,
            "range": "± 8981621",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 124564,
            "range": "± 4635",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 124654,
            "range": "± 4644",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 125314,
            "range": "± 4410",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 394751042,
            "range": "± 7012438",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 330530342,
            "range": "± 4689849",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 287048681,
            "range": "± 6155237",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 256741697,
            "range": "± 5878643",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 32436458,
            "range": "± 3581487",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 286694806,
            "range": "± 6133647",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 257291312,
            "range": "± 5885293",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 259659258,
            "range": "± 5108606",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 85980583,
            "range": "± 2559593",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 92821102,
            "range": "± 5573161",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 170292347,
            "range": "± 11000516",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 241654588,
            "range": "± 16621746",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 404634903,
            "range": "± 26314051",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1105372,
            "range": "± 164865",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9613345,
            "range": "± 1446579",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 48278236,
            "range": "± 1241193",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 130284,
            "range": "± 4472",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 219811,
            "range": "± 13432",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 615398,
            "range": "± 8794",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 133874,
            "range": "± 4529",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 196400,
            "range": "± 5943",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 479185,
            "range": "± 6121",
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
          "id": "d903234106329f842ed8d8c79224ceb5d584bed7",
          "message": "chore: release v0.6.0 (#67)\n\n## 🤖 New release\n\n* `taskmill`: 0.5.3 -> 0.6.0 (⚠ API breaking changes)\n\n### ⚠ `taskmill` breaking changes\n\n```text\n--- failure inherent_method_missing: pub method removed or renamed ---\n\nDescription:\nA publicly-visible method or associated fn is no longer available under its prior name. It may have been renamed or removed entirely.\n        ref: https://doc.rust-lang.org/cargo/reference/semver.html#item-remove\n       impl: https://github.com/obi1kenobi/cargo-semver-checks/tree/v0.46.0/src/lints/inherent_method_missing.ron\n\nFailed in:\n  DomainHandle::submit_raw, previously in file /tmp/.tmpZpSjOB/taskmill/src/domain.rs:502\n  DomainHandle::submit_raw, previously in file /tmp/.tmpZpSjOB/taskmill/src/domain.rs:502\n  Domain::raw_executor, previously in file /tmp/.tmpZpSjOB/taskmill/src/domain.rs:349\n  Domain::raw_executor, previously in file /tmp/.tmpZpSjOB/taskmill/src/domain.rs:349\n  TaskTypeRegistry::register, previously in file /tmp/.tmpZpSjOB/taskmill/src/registry/mod.rs:170\n  TaskTypeRegistry::register_with_ttl, previously in file /tmp/.tmpZpSjOB/taskmill/src/registry/mod.rs:179\n  TaskTypeRegistry::register_with_retry_policy, previously in file /tmp/.tmpZpSjOB/taskmill/src/registry/mod.rs:190\n\n--- failure struct_missing: pub struct removed or renamed ---\n\nDescription:\nA publicly-visible struct cannot be imported by its prior path. A `pub use` may have been removed, or the struct itself may have been renamed or removed entirely.\n        ref: https://doc.rust-lang.org/cargo/reference/semver.html#item-remove\n       impl: https://github.com/obi1kenobi/cargo-semver-checks/tree/v0.46.0/src/lints/struct_missing.ron\n\nFailed in:\n  struct taskmill::registry::TaskContext, previously in file /tmp/.tmpZpSjOB/taskmill/src/registry/context.rs:29\n  struct taskmill::TaskContext, previously in file /tmp/.tmpZpSjOB/taskmill/src/registry/context.rs:29\n\n--- failure trait_missing: pub trait removed or renamed ---\n\nDescription:\nA publicly-visible trait cannot be imported by its prior path. A `pub use` may have been removed, or the trait itself may have been renamed or removed entirely.\n        ref: https://doc.rust-lang.org/cargo/reference/semver.html#item-remove\n       impl: https://github.com/obi1kenobi/cargo-semver-checks/tree/v0.46.0/src/lints/trait_missing.ron\n\nFailed in:\n  trait taskmill::registry::TaskExecutor, previously in file /tmp/.tmpZpSjOB/taskmill/src/registry/mod.rs:56\n  trait taskmill::TaskExecutor, previously in file /tmp/.tmpZpSjOB/taskmill/src/registry/mod.rs:56\n```\n\n<details><summary><i><b>Changelog</b></i></summary><p>\n\n<blockquote>\n\n## [0.6.0](https://github.com/deepjoy/taskmill/compare/v0.5.3...v0.6.0)\n- 2026-03-21\n\n### Added\n\n- [**breaking**] replace `&TaskContext` with `DomainTaskContext<D>` for\ntype-safe child spawning\n([#68](https://github.com/deepjoy/taskmill/pull/68))\n\n### Other\n\n- migrate all tests/benches from TaskExecutor to TypedExecutor\n([#66](https://github.com/deepjoy/taskmill/pull/66))\n</blockquote>\n\n\n</p></details>\n\n---\nThis PR was generated with\n[release-plz](https://github.com/release-plz/release-plz/).\n\nCo-authored-by: github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>",
          "timestamp": "2026-03-21T09:11:23Z",
          "tree_id": "b6efd12e44c9f62fb1b4c0c5b70742745052944a",
          "url": "https://github.com/deepjoy/taskmill/commit/d903234106329f842ed8d8c79224ceb5d584bed7"
        },
        "date": 1774085916135,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3232100,
            "range": "± 174241",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16670135,
            "range": "± 869559",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 75394709,
            "range": "± 3689572",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 10996259,
            "range": "± 157769",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 26285183,
            "range": "± 354378",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 51792789,
            "range": "± 944713",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7228156,
            "range": "± 121540",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 26774800,
            "range": "± 604548",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 51846611,
            "range": "± 754315",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 236538466,
            "range": "± 4872407",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 406109623,
            "range": "± 5637235",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 407118758,
            "range": "± 5388169",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 405386319,
            "range": "± 5476970",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 408778258,
            "range": "± 5040551",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 409643525,
            "range": "± 4425581",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 540633,
            "range": "± 14757",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 539950,
            "range": "± 7524",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 556274,
            "range": "± 6548",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 144813,
            "range": "± 937",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 361230,
            "range": "± 1112",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1279219,
            "range": "± 2396",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 893060,
            "range": "± 27276",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 895366,
            "range": "± 47886",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 912017,
            "range": "± 35274",
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
            "value": 76,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 190,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 450,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 357957031,
            "range": "± 4092302",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 146461882,
            "range": "± 2150114",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 145686272,
            "range": "± 1796027",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 145778682,
            "range": "± 2455153",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 145403204,
            "range": "± 1830661",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 177910866,
            "range": "± 4850782",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 235374241,
            "range": "± 7826342",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 479187274,
            "range": "± 6033621",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 117658,
            "range": "± 3233",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 118270,
            "range": "± 2321",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 117720,
            "range": "± 2498",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 370900621,
            "range": "± 3923257",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 308772532,
            "range": "± 3672208",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 268516888,
            "range": "± 4944504",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 239877843,
            "range": "± 3682469",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 33193905,
            "range": "± 2550092",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 267880130,
            "range": "± 4662252",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 240308096,
            "range": "± 3855162",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 242512241,
            "range": "± 4248055",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 79819275,
            "range": "± 1799482",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 88945268,
            "range": "± 3173921",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 162911140,
            "range": "± 7189280",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 230047955,
            "range": "± 9604707",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 384588779,
            "range": "± 17344426",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1111779,
            "range": "± 143983",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9497959,
            "range": "± 728050",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 49686846,
            "range": "± 6643680",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 126918,
            "range": "± 3126",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 213984,
            "range": "± 4034",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 617921,
            "range": "± 4956",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 130157,
            "range": "± 3156",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 190049,
            "range": "± 3969",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 445695,
            "range": "± 4464",
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
          "id": "e91f356eb0422e44b1569deac4e5cd87fc5fbba2",
          "message": "feat!: support passing state from execute() to finalize() via typed Memo (#69)\n\n## Summary\n\n- Add a default type parameter `Memo = ()` to `TypedExecutor<T>` so\nexecutors can return typed state from `execute()` that is persisted to\nSQLite and delivered to `finalize()` after children complete\n- New `Domain::task_memo()` and `Domain::task_with_memo()` registration\nmethods for memo-producing executors; existing `Domain::task()` is\nunchanged\n- Add `memo BLOB` column to `tasks` and `task_history` tables (migration\n009); `TypeId::of::<()>()` check avoids DB writes for non-memo tasks\n- Update all doc examples (`lib.rs`, `quick-start.md`,\n`migrating-to-0.5.md`) to include the new `_memo: ()` parameter in\n`finalize()` signatures\n\nCloses #64 \n## Details\n\nThe memo is serialized via `serde_json` at the `set_waiting` transition\n(the single correct write point after execute returns and children are\ndetected). On finalize dispatch, the memo bytes are deserialized back\ninto the concrete `Memo` type. When `Memo = ()`, no serialization or DB\nwrite occurs — the `TypeId` guard short-circuits to `Ok(None)`.\n\n### Public API changes\n\n| Before | After |\n|---|---|\n| `TypedExecutor<T>` | `TypedExecutor<T, Memo = ()>` |\n| `execute() -> Result<(), TaskError>` | `execute() -> Result<Memo,\nTaskError>` |\n| `finalize(payload, ctx)` | `finalize(payload, memo, ctx)` |\n| `Domain::task()` | `Domain::task()` (unchanged) +\n`Domain::task_memo()` |\n| `Domain::task_with()` | `Domain::task_with()` (unchanged) +\n`Domain::task_with_memo()` |\n\n### Migration\n\nExisting executors with `Memo = ()` only need to add `_memo: ()` to\ntheir `finalize()` override (if any). Executors that don't override\n`finalize()` require zero changes.\n\n## BREAKING CHANGE\n\n`TypedExecutor::finalize()` signature adds a `Memo` parameter between\n`payload` and `ctx`.",
          "timestamp": "2026-03-22T00:45:04Z",
          "tree_id": "3f1bb5b047d38ce80f7921e0c4ca5531703a4e62",
          "url": "https://github.com/deepjoy/taskmill/commit/e91f356eb0422e44b1569deac4e5cd87fc5fbba2"
        },
        "date": 1774141989401,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3475850,
            "range": "± 191900",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 18131980,
            "range": "± 1037419",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 79040904,
            "range": "± 5364693",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11889308,
            "range": "± 133492",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 28436585,
            "range": "± 348030",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 56671935,
            "range": "± 1086520",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7767984,
            "range": "± 88298",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 29325052,
            "range": "± 454065",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 56518160,
            "range": "± 754235",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 256881234,
            "range": "± 6018400",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 434832010,
            "range": "± 7645258",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 437828062,
            "range": "± 6997957",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 438509087,
            "range": "± 6664157",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 438088709,
            "range": "± 6073109",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 437882846,
            "range": "± 8633484",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 567559,
            "range": "± 17249",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 563967,
            "range": "± 7980",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 586185,
            "range": "± 11066",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 151633,
            "range": "± 1855",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 362760,
            "range": "± 1437",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1299585,
            "range": "± 4205",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 934621,
            "range": "± 33272",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 963717,
            "range": "± 32429",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 1003967,
            "range": "± 33215",
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
            "value": 79,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 194,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 408,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 389150961,
            "range": "± 4692900",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 159063846,
            "range": "± 2117649",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 160037383,
            "range": "± 2108894",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 159518143,
            "range": "± 2420121",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 159779294,
            "range": "± 2046420",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 188464477,
            "range": "± 8255258",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 246698050,
            "range": "± 10331238",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 513917795,
            "range": "± 9085770",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 123857,
            "range": "± 4923",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 126614,
            "range": "± 4452",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 125549,
            "range": "± 4253",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 393891030,
            "range": "± 6028434",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 330732732,
            "range": "± 6761711",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 286891590,
            "range": "± 6425088",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 256041749,
            "range": "± 4569078",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 36014058,
            "range": "± 3076814",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 287094484,
            "range": "± 5829736",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 256597766,
            "range": "± 5482870",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 260251820,
            "range": "± 5383194",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 85953986,
            "range": "± 2424677",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 93251663,
            "range": "± 5090522",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 168016070,
            "range": "± 9856516",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 247659466,
            "range": "± 13194372",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 407407663,
            "range": "± 23807801",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1123803,
            "range": "± 161886",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9659133,
            "range": "± 1617025",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 48477094,
            "range": "± 5204503",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 130852,
            "range": "± 4635",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 233192,
            "range": "± 5905",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 607013,
            "range": "± 7180",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 135806,
            "range": "± 6039",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 199299,
            "range": "± 6071",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 458530,
            "range": "± 7878",
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
          "id": "61ce1ca1a34e51beea7cd126015c438bc85c0c6e",
          "message": "fix: use distinct critcmp group names for PR benchmark comparison (#72)\n\n## Summary\n\n- The cached main baseline and the PR benchmark results were both saved\nunder the critcmp group name `current`, causing `critcmp` to merge them\ninto a single column instead of producing a two-column comparison with\nratios.\n- Import the cached baseline under the group name `main` before\ncomparing, so `critcmp` sees two distinct groups (`main` vs `current`)\nand renders a proper side-by-side diff.",
          "timestamp": "2026-03-22T02:39:26Z",
          "tree_id": "b02e22a2c141e6df5c13b925d7a5bf5bd6d4d501",
          "url": "https://github.com/deepjoy/taskmill/commit/61ce1ca1a34e51beea7cd126015c438bc85c0c6e"
        },
        "date": 1774148863970,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3501397,
            "range": "± 240171",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 18294590,
            "range": "± 1237482",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 78670327,
            "range": "± 4966783",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11771885,
            "range": "± 153928",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 28560460,
            "range": "± 447241",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 56838309,
            "range": "± 1233869",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7744411,
            "range": "± 129279",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 29354580,
            "range": "± 482984",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 56389982,
            "range": "± 1263792",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 257833605,
            "range": "± 5390594",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 440304163,
            "range": "± 6462702",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 438823262,
            "range": "± 6407058",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 441528191,
            "range": "± 7200077",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 440252466,
            "range": "± 6427872",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 442205787,
            "range": "± 6549021",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 558031,
            "range": "± 15034",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 561638,
            "range": "± 7850",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 581028,
            "range": "± 10358",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 151299,
            "range": "± 1525",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 363868,
            "range": "± 911",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1311933,
            "range": "± 3626",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 916639,
            "range": "± 35263",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 946070,
            "range": "± 24259",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 968180,
            "range": "± 33326",
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
            "value": 80,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 193,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 451,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 392820122,
            "range": "± 5278810",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 160409624,
            "range": "± 2008784",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 160665518,
            "range": "± 1734192",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 160671036,
            "range": "± 1838892",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 161233900,
            "range": "± 2201473",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 189786342,
            "range": "± 7562802",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 249366626,
            "range": "± 10916942",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 515646386,
            "range": "± 8567213",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 125057,
            "range": "± 4328",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 125336,
            "range": "± 4974",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 124703,
            "range": "± 6396",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 393933317,
            "range": "± 7103147",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 331761869,
            "range": "± 5230295",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 286298427,
            "range": "± 7069046",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 256391029,
            "range": "± 4989636",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 35180026,
            "range": "± 3244345",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 288181281,
            "range": "± 5640847",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 257817574,
            "range": "± 5209899",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 260348003,
            "range": "± 5367053",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 86534071,
            "range": "± 2703929",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 92876901,
            "range": "± 4889180",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 167018416,
            "range": "± 10058378",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 243535154,
            "range": "± 14786986",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 411155421,
            "range": "± 25084306",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1118495,
            "range": "± 147100",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9614719,
            "range": "± 1628845",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 54129790,
            "range": "± 6680166",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 132002,
            "range": "± 4912",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 224043,
            "range": "± 4168",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 608101,
            "range": "± 6348",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 136755,
            "range": "± 6907",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 196195,
            "range": "± 6591",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 464025,
            "range": "± 7231",
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
          "id": "fc98428673505917fe137e8e653030018f305554",
          "message": "feat: add tag key prefix queries for namespace-scoped discovery (#71)\n\n## Summary\n\n- Add four new query methods (`tag_keys_by_prefix`,\n`tasks_by_tag_key_prefix`, `count_by_tag_key_prefix`,\n`cancel_by_tag_key_prefix`) across `TaskStore`, `Scheduler`,\n`ModuleHandle`, and `DomainHandle` for discovering and operating on\ntasks by tag key prefix (e.g. `\"billing.\"`)\n- Add `escape_like_prefix()` helper that escapes SQL LIKE wildcards\n(`%`, `_`, `\\`) in user-supplied input before appending the trailing\n`%`, ensuring only true prefix matching\n- Add domain-scoped `_with_prefix` store variants that additionally\nfilter by `task_type LIKE 'domain%'`, consistent with the existing tag\nquery pattern\n- Document the new APIs in crate-level docs, `docs/query-apis.md`, and\n`docs/multi-module-apps.md`\n\n## Motivation\n\nWhen multiple independent libraries share a single scheduler, each\nnaturally namespaces its tags with a prefix (`billing.customer_id`,\n`media.pipeline`, etc.). The existing exact-match tag API cannot answer\nnamespace-scoped questions like \"how many billing tasks are active?\"\nwithout knowing every possible `billing.*` key upfront. The new prefix\nqueries fill this gap.\n\nCloses #41",
          "timestamp": "2026-03-22T02:41:19Z",
          "tree_id": "cae8ddefd0d67cf49817069c7100472bb2f01549",
          "url": "https://github.com/deepjoy/taskmill/commit/fc98428673505917fe137e8e653030018f305554"
        },
        "date": 1774149001930,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3555374,
            "range": "± 236870",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 18045292,
            "range": "± 1231953",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 79546121,
            "range": "± 5400497",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 12264488,
            "range": "± 94438",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 29423863,
            "range": "± 463244",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 58631573,
            "range": "± 1228541",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7912593,
            "range": "± 112064",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 30388491,
            "range": "± 546821",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 57769460,
            "range": "± 1201560",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 262214264,
            "range": "± 5077962",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 451489834,
            "range": "± 6352709",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 454225029,
            "range": "± 6301212",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 455509736,
            "range": "± 6188615",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 455046193,
            "range": "± 6253687",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 454680326,
            "range": "± 7024147",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 565542,
            "range": "± 12694",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 572789,
            "range": "± 8246",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 588480,
            "range": "± 8384",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 152235,
            "range": "± 1656",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 366219,
            "range": "± 1021",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1303435,
            "range": "± 5751",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 930932,
            "range": "± 23713",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 959905,
            "range": "± 22005",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 984092,
            "range": "± 28534",
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
            "value": 76,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 224,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 407,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 400919412,
            "range": "± 6117815",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 163868620,
            "range": "± 2597860",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 163996674,
            "range": "± 2130026",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 164238937,
            "range": "± 2432278",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 164151142,
            "range": "± 2029261",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 191338880,
            "range": "± 7764867",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 248135528,
            "range": "± 12904460",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 527574041,
            "range": "± 8874624",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 128361,
            "range": "± 6986",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 131604,
            "range": "± 6681",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 126188,
            "range": "± 4656",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 405741024,
            "range": "± 7907388",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 338373578,
            "range": "± 5159935",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 294424107,
            "range": "± 6937568",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 268969390,
            "range": "± 9978156",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 35593143,
            "range": "± 3552389",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 304410376,
            "range": "± 12731472",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 275989937,
            "range": "± 24913566",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 272019529,
            "range": "± 13284934",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 90801750,
            "range": "± 8012199",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 100482858,
            "range": "± 7903958",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 178829545,
            "range": "± 13590776",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 260093994,
            "range": "± 15933684",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 430427478,
            "range": "± 30860107",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1152851,
            "range": "± 156683",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 10629019,
            "range": "± 1805458",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 59573873,
            "range": "± 6614117",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 135437,
            "range": "± 5841",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 239208,
            "range": "± 21299",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 626568,
            "range": "± 41782",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 140139,
            "range": "± 5734",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 199038,
            "range": "± 7029",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 526912,
            "range": "± 30592",
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
          "id": "84a09bbdc26f950775cf9995802849c88fa7062b",
          "message": "feat: expose `fail_fast()` on `SubmitBuilder` and `DomainSubmitBuilder` (#73)\n\n## Summary\n\n- Add `fail_fast()` builder method to `SubmitBuilder` and\n`DomainSubmitBuilder`, allowing users to disable fail-fast behavior\nthrough the typed domain API instead of dropping down to raw\n`TaskSubmission`\n- Wire `override_fail_fast` through the `SubmitBuilder` override/resolve\nchain, matching the existing pattern used by `priority()`, `group()`,\n`parent()`, etc.\n- Update module docs (`lib.rs`), quick-start guide, and glossary to\ndocument the new method with usage examples\n\nCloses #63",
          "timestamp": "2026-03-22T02:57:49Z",
          "tree_id": "712473bc5abb4bc590c46a0f3e42cd068d3f6f26",
          "url": "https://github.com/deepjoy/taskmill/commit/84a09bbdc26f950775cf9995802849c88fa7062b"
        },
        "date": 1774149958856,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3520736,
            "range": "± 171107",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17749474,
            "range": "± 1072075",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 77993611,
            "range": "± 5678039",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 12087625,
            "range": "± 136355",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 29122911,
            "range": "± 359980",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 57763911,
            "range": "± 831499",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7877955,
            "range": "± 105309",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 29832398,
            "range": "± 509826",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 56690856,
            "range": "± 960091",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 258827792,
            "range": "± 4948939",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 444126277,
            "range": "± 7143023",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 443164502,
            "range": "± 7105480",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 443735809,
            "range": "± 5798423",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 446541483,
            "range": "± 7219516",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 445171515,
            "range": "± 7050442",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 541705,
            "range": "± 13272",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 564309,
            "range": "± 8121",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 573647,
            "range": "± 14591",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 152924,
            "range": "± 1199",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 363726,
            "range": "± 1354",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1296050,
            "range": "± 3943",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 940289,
            "range": "± 25694",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 943264,
            "range": "± 31215",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 953438,
            "range": "± 28589",
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
            "value": 76,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 189,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 407,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 392992894,
            "range": "± 6293284",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 159765771,
            "range": "± 1880656",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 160145392,
            "range": "± 1904557",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 159938706,
            "range": "± 1766189",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 159560599,
            "range": "± 1870496",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 188624980,
            "range": "± 7049064",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 247865302,
            "range": "± 10380126",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 514606317,
            "range": "± 8371470",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 124037,
            "range": "± 3743",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 123470,
            "range": "± 4677",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 124517,
            "range": "± 3978",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 397515740,
            "range": "± 6597308",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 331493205,
            "range": "± 5825246",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 286046363,
            "range": "± 5995546",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 256539676,
            "range": "± 4921543",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 34853511,
            "range": "± 3333346",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 288663168,
            "range": "± 5897188",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 257296880,
            "range": "± 5480914",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 260029167,
            "range": "± 5790166",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 86771288,
            "range": "± 2427407",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 93055816,
            "range": "± 4718518",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 168181691,
            "range": "± 8936029",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 243240087,
            "range": "± 14079067",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 406280568,
            "range": "± 26549537",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1132705,
            "range": "± 167331",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9698554,
            "range": "± 1347188",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 49235350,
            "range": "± 1601023",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 133534,
            "range": "± 5428",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 221439,
            "range": "± 4780",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 603808,
            "range": "± 9159",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 139197,
            "range": "± 7224",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 197478,
            "range": "± 6142",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 456655,
            "range": "± 8056",
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
          "id": "d3f9935f2f5cd61bd80deedddb19f48d87ed37e8",
          "message": "fix: use distinct critcmp group names for PR benchmark comparison (#74)\n\n- PR #72 introduced `critcmp --import` which doesn't exist, causing the\n\"Compare benchmarks\" step to fail on every PR\n- Pass the exported JSON files directly as positional arguments\n(`critcmp baseline.json pr.json`), which is the supported way to compare\ntwo baselines",
          "timestamp": "2026-03-22T03:38:30Z",
          "tree_id": "81438568c0b75c72d5202b122ae1c1792fab26df",
          "url": "https://github.com/deepjoy/taskmill/commit/d3f9935f2f5cd61bd80deedddb19f48d87ed37e8"
        },
        "date": 1774152373956,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3308538,
            "range": "± 166405",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16925542,
            "range": "± 804681",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 76501969,
            "range": "± 4300143",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11142100,
            "range": "± 172689",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 27016107,
            "range": "± 563024",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 53075532,
            "range": "± 823292",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7406243,
            "range": "± 119412",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 27428944,
            "range": "± 533089",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 52761176,
            "range": "± 756182",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 242779348,
            "range": "± 3679049",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 411664227,
            "range": "± 6294864",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 413776090,
            "range": "± 5502700",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 414986073,
            "range": "± 5232427",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 413955411,
            "range": "± 5214040",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 412369380,
            "range": "± 5423660",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 540313,
            "range": "± 12716",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 564780,
            "range": "± 8246",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 563795,
            "range": "± 13107",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 145287,
            "range": "± 612",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 359747,
            "range": "± 1414",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1294663,
            "range": "± 6772",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 928501,
            "range": "± 22458",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 948972,
            "range": "± 44601",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 960553,
            "range": "± 23238",
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
            "value": 450,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 363824728,
            "range": "± 4174006",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 148913739,
            "range": "± 1907430",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 148740637,
            "range": "± 2104123",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 149152053,
            "range": "± 1938101",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 149420794,
            "range": "± 2131060",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 182054620,
            "range": "± 4724735",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 243352197,
            "range": "± 7848225",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 492580845,
            "range": "± 6963575",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 118909,
            "range": "± 2500",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 121206,
            "range": "± 3504",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 119541,
            "range": "± 2455",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 383372039,
            "range": "± 4182435",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 315089363,
            "range": "± 3927757",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 271238299,
            "range": "± 5192171",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 244646168,
            "range": "± 4782222",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 32962321,
            "range": "± 3139113",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 271639260,
            "range": "± 4887035",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 243967477,
            "range": "± 3967931",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 245933231,
            "range": "± 5213079",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 79725505,
            "range": "± 2260285",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 89689616,
            "range": "± 2708848",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 164050240,
            "range": "± 6704702",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 240625047,
            "range": "± 11089942",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 388952794,
            "range": "± 16997064",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1153583,
            "range": "± 155154",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 10045959,
            "range": "± 1491205",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 56044610,
            "range": "± 5197727",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 127448,
            "range": "± 3107",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 218287,
            "range": "± 5040",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 606565,
            "range": "± 5565",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 132002,
            "range": "± 3894",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 192806,
            "range": "± 3157",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 454673,
            "range": "± 5413",
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
          "id": "4d1073bbf75cd7dd6e806b3e3dd5bfe96a864138",
          "message": "feat!: normalize timestamps from TEXT to epoch millisecond INTEGER (#75)\n\n## Summary\n\n- Convert all 9 timestamp columns (`created_at`, `started_at`,\n`completed_at`, `expires_at`, `run_after`) across `tasks` and\n`task_history` tables from `TEXT` to `INTEGER` epoch milliseconds\n- Replace all `datetime('now')` / `strftime(...)` SQL expressions with\nbound `i64` parameters computed in Rust, and all TEXT formatting\n(`format(\"%Y-%m-%d %H:%M:%S\")`) with `.timestamp_millis()` calls\n- Replace the hand-rolled `parse_datetime` / `parse_2` / `parse_4` /\n`parse_frac_nanos` TEXT parser with `epoch_ms()` / `from_epoch_ms()`\nhelpers\n- Add explicit `completed_at` bind to `insert_history` (previously\nrelied on `DEFAULT (datetime('now'))` which is removed)\n- Fix `idx_tasks_expires` partial index to include `'blocked'` status\n(pre-existing bug — `expire_tasks()` queries `status IN ('pending',\n'paused', 'blocked')` but the index only covered `('pending',\n'paused')`)\n- TTL arithmetic now uses integer math (`? + (ttl_seconds * 1000)`)\ninstead of `datetime('now', '+' || ttl_seconds || ' seconds')`\n\n## Motivation\n\n- **SQL arithmetic**: integer comparisons and addition replace\n`strftime('%s', col)` conversions — simpler, faster, less error-prone\n- **Index efficiency**: INTEGER range scans outperform TEXT string\ncomparisons for expiry, `run_after` gating, and pruning\n- **Format consistency**: submission `run_after` used second precision\n(`%H:%M:%S`) while retry backoff used millisecond precision\n(`%H:%M:%S%.3f`) — epoch millis unify both\n- **Forward compatibility**: plan 042 (pause-by-group) will add new\ncolumns as epoch INTEGER; converting existing columns now avoids a\ntwo-format codebase\n\n## Breaking change\n\nStorage-level only — public Rust types remain `DateTime<Utc>`. Requires\n**database recreation** (no live migration). The active `tasks` table is\ntransient (work queue drains) and `task_history` is diagnostic, so data\nloss on recreation is acceptable.",
          "timestamp": "2026-03-22T17:55:52Z",
          "tree_id": "dd7131d7fe93e570b0145a18432468cacdb38a2d",
          "url": "https://github.com/deepjoy/taskmill/commit/4d1073bbf75cd7dd6e806b3e3dd5bfe96a864138"
        },
        "date": 1774203809086,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3406219,
            "range": "± 182941",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17198729,
            "range": "± 811413",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 76920449,
            "range": "± 3254468",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11168122,
            "range": "± 101678",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 26743384,
            "range": "± 484102",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 52949513,
            "range": "± 944402",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7356106,
            "range": "± 108015",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 27327146,
            "range": "± 410070",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 52330120,
            "range": "± 780302",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 243226569,
            "range": "± 4169076",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 414780457,
            "range": "± 6138447",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 407659871,
            "range": "± 5349442",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 415546806,
            "range": "± 5938493",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 416108566,
            "range": "± 6212515",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 414856843,
            "range": "± 6504703",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 536509,
            "range": "± 10678",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 541767,
            "range": "± 7309",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 552776,
            "range": "± 5094",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 146196,
            "range": "± 1409",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 352950,
            "range": "± 1890",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1268829,
            "range": "± 5238",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 881470,
            "range": "± 24089",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 909499,
            "range": "± 29596",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 941053,
            "range": "± 36330",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 49,
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
            "value": 186,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 268,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 359837344,
            "range": "± 7820402",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 146764433,
            "range": "± 2789852",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 146657879,
            "range": "± 2313958",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 146868762,
            "range": "± 1945512",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 146254952,
            "range": "± 2003760",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 181227941,
            "range": "± 5353753",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 241098640,
            "range": "± 6899457",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 488988229,
            "range": "± 8281654",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 121004,
            "range": "± 3547",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 123836,
            "range": "± 4885",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 123259,
            "range": "± 3525",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 377310390,
            "range": "± 5608108",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 311733165,
            "range": "± 3670096",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 269211109,
            "range": "± 5595147",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 243194929,
            "range": "± 3738256",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 32725589,
            "range": "± 2473420",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 269618164,
            "range": "± 5474591",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 243156314,
            "range": "± 4079246",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 243177859,
            "range": "± 5027925",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 80692310,
            "range": "± 3595928",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 89375013,
            "range": "± 3585871",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 163827083,
            "range": "± 6733907",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 240714728,
            "range": "± 12292290",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 388803147,
            "range": "± 16121505",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1156190,
            "range": "± 145708",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9678745,
            "range": "± 776143",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 55561779,
            "range": "± 7957407",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 128450,
            "range": "± 2809",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 217084,
            "range": "± 2945",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 596866,
            "range": "± 7692",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 133243,
            "range": "± 3094",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 194201,
            "range": "± 2977",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 459625,
            "range": "± 4124",
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
          "id": "a36f033e5de17c217270cf75b197e3b1a066f684",
          "message": "refactor!: consolidate migrations from 9 chronological files into 4 object-oriented files (#76)\n\n## Summary\n\n- Reorganize 9 chronological migration files into 4 files grouped by\ndatabase object: `tasks`, `task_history`, `task_deps`, and\n`task_tags`/`task_history_tags`\n- Fold all `ALTER TABLE ADD COLUMN` statements (label, net IO, groups,\nTTL, scheduling, dependencies, retries, memo) into their respective\n`CREATE TABLE` definitions, eliminating every ALTER statement\n- Simplify `migrate()` in `store/mod.rs` — all migrations now use\nidempotent `CREATE TABLE/INDEX IF NOT EXISTS`, removing the\n`run_alter_migration` helper\n\n### New migration layout\n\n| File | Object(s) | Indexes |\n|---|---|---|\n| `001_tasks.sql` | `tasks` (32 cols) | 6 partial indexes |\n| `002_task_history.sql` | `task_history` (30 cols) | 5 indexes |\n| `003_task_deps.sql` | `task_deps` | 1 index |\n| `004_task_tags.sql` | `task_tags`, `task_history_tags` | 2 indexes |\n\nBREAKING CHANGE: Existing SQLite databases must be recreated. This is\nacceptable as the project is pre-1.0.",
          "timestamp": "2026-03-22T13:25:32-07:00",
          "tree_id": "fdd50731d34b954e289d397621d213ec59c8e673",
          "url": "https://github.com/deepjoy/taskmill/commit/a36f033e5de17c217270cf75b197e3b1a066f684"
        },
        "date": 1774212808050,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3093775,
            "range": "± 176448",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17057564,
            "range": "± 1042052",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 81001254,
            "range": "± 4750484",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11643176,
            "range": "± 187171",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 28380713,
            "range": "± 440948",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 56867219,
            "range": "± 670063",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7351872,
            "range": "± 162615",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 28948987,
            "range": "± 755971",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 56101340,
            "range": "± 804779",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 260068825,
            "range": "± 4573074",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 453475856,
            "range": "± 6055684",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 451172961,
            "range": "± 6154912",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 455341140,
            "range": "± 5698051",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 454836205,
            "range": "± 4758110",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 453391533,
            "range": "± 5893960",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 546600,
            "range": "± 9533",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 562947,
            "range": "± 9651",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 570463,
            "range": "± 4231",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 152986,
            "range": "± 1355",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 385933,
            "range": "± 918",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1346276,
            "range": "± 5012",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 901054,
            "range": "± 14412",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 915157,
            "range": "± 24805",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 954760,
            "range": "± 22271",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 50,
            "range": "± 1",
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
            "value": 187,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 268,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 394984811,
            "range": "± 4876842",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 160302384,
            "range": "± 2197626",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 159504844,
            "range": "± 1846502",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 159405219,
            "range": "± 1723292",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 159650292,
            "range": "± 2061818",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 191764855,
            "range": "± 7018234",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 253453409,
            "range": "± 11083088",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 522992972,
            "range": "± 7692931",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 127086,
            "range": "± 4405",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 128926,
            "range": "± 3879",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 129219,
            "range": "± 3674",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 399029862,
            "range": "± 6737863",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 335895336,
            "range": "± 4937398",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 290088457,
            "range": "± 6705191",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 259855943,
            "range": "± 4982020",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 36061896,
            "range": "± 2806370",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 290042647,
            "range": "± 5791368",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 259970085,
            "range": "± 4798763",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 264655291,
            "range": "± 5032761",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 88137695,
            "range": "± 2631293",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 93907129,
            "range": "± 4188157",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 175027494,
            "range": "± 8684742",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 253161564,
            "range": "± 14407424",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 412859585,
            "range": "± 22533385",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1138010,
            "range": "± 145027",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9875266,
            "range": "± 1147382",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 59761127,
            "range": "± 2910334",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 137732,
            "range": "± 6583",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 228644,
            "range": "± 6425",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 619590,
            "range": "± 7634",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 140617,
            "range": "± 6197",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 199347,
            "range": "± 5559",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 463293,
            "range": "± 7567",
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
          "id": "ca4b3d5ed3e9c35483be42f473abe94aa5e429d4",
          "message": "fix: use distinct critcmp group names so PR benchmark diff shows two columns (#77)\n\n## Summary\n\n- Both the main baseline and PR benchmark results were exported under\nthe same\ncritcmp group name (`\"current\"`), causing `critcmp baseline.json\npr.json` to\nmerge them into a single column instead of producing a side-by-side\ncomparison.\n- Rename the cached baseline group to `\"main\"` and the PR export group\nto `\"pr\"`\n  so critcmp can diff them as two distinct columns.",
          "timestamp": "2026-03-22T20:27:23Z",
          "tree_id": "187b5effc1951e2b8bc88f115cbcbf3c1766ee23",
          "url": "https://github.com/deepjoy/taskmill/commit/ca4b3d5ed3e9c35483be42f473abe94aa5e429d4"
        },
        "date": 1774212865331,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3069552,
            "range": "± 128713",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16906126,
            "range": "± 764154",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 77126528,
            "range": "± 3757803",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 10839670,
            "range": "± 98415",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 26413372,
            "range": "± 622663",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 52950666,
            "range": "± 879073",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7027202,
            "range": "± 87923",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 26769386,
            "range": "± 638105",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 51769935,
            "range": "± 1036922",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups_500",
            "value": 243410446,
            "range": "± 3654689",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group_500",
            "value": 416166821,
            "range": "± 6923105",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 417902540,
            "range": "± 5308018",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 417164559,
            "range": "± 5013603",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 416821798,
            "range": "± 5432471",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 415825416,
            "range": "± 4470948",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 540858,
            "range": "± 8115",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 544652,
            "range": "± 7172",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 558736,
            "range": "± 6386",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 148085,
            "range": "± 729",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 367252,
            "range": "± 1280",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1329035,
            "range": "± 7618",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 889851,
            "range": "± 19511",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 926594,
            "range": "± 38921",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 917644,
            "range": "± 26389",
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
            "value": 76,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 186,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 269,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure_500",
            "value": 361992073,
            "range": "± 3707751",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 147253790,
            "range": "± 2172305",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 147218045,
            "range": "± 1972228",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 147120336,
            "range": "± 1896874",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 146240171,
            "range": "± 1726854",
            "unit": "ns/iter"
          },
          {
            "name": "submit_1000_tasks",
            "value": 179525700,
            "range": "± 4513927",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit_1000",
            "value": 237962404,
            "range": "± 7442951",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete_1000",
            "value": 485220558,
            "range": "± 5968677",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 119627,
            "range": "± 2978",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 120339,
            "range": "± 2976",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 119663,
            "range": "± 2770",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 373779859,
            "range": "± 4283149",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 310292949,
            "range": "± 3400743",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 267527603,
            "range": "± 4599690",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 241387244,
            "range": "± 4122037",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit_1000",
            "value": 34112671,
            "range": "± 2399386",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch_500",
            "value": 268259589,
            "range": "± 4557453",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 240752104,
            "range": "± 4075173",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 242219862,
            "range": "± 6146464",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot_100_tasks",
            "value": 80132918,
            "range": "± 1886878",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 89417071,
            "range": "± 3361735",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 165559244,
            "range": "± 7402512",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 238815796,
            "range": "± 8144266",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 395376106,
            "range": "± 17264769",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1152206,
            "range": "± 140880",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9659142,
            "range": "± 1295141",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 54360958,
            "range": "± 1961428",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 127745,
            "range": "± 2850",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 215368,
            "range": "± 2855",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 615826,
            "range": "± 7042",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 132370,
            "range": "± 3836",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 194858,
            "range": "± 2891",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 470582,
            "range": "± 4603",
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
          "id": "773c31d22618e36fadbff5b46738c75261fcb614",
          "message": "feat: add Throughput to all criterion benchmarks so critcmp shows ops/sec (#78)\n\n- All criterion benchmarks now declare `Throughput::Elements(N)` so\ncritcmp\n  reports real throughput (tasks/sec, queries/sec) instead of `? ?/sec`.\n- Standalone `bench_function` calls are converted to `BenchmarkGroup`s\nsince\ncriterion only exposes throughput on groups. This changes benchmark\nnames\nfrom flat (e.g. `submit_1000_tasks`) to structured (e.g.\n`submit_tasks/1000`).\n- Parameterized benchmarks (dependency chains, fan-in) set throughput\nper\n  parameter to reflect the actual element count at each size.",
          "timestamp": "2026-03-22T21:09:54Z",
          "tree_id": "63a748233f0a5af6203a22e93b1af4edc6fc3254",
          "url": "https://github.com/deepjoy/taskmill/commit/773c31d22618e36fadbff5b46738c75261fcb614"
        },
        "date": 1774215409955,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3091468,
            "range": "± 153467",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16835350,
            "range": "± 783936",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 75874896,
            "range": "± 2996448",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 10990007,
            "range": "± 167059",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 26986269,
            "range": "± 778638",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 53127108,
            "range": "± 1631008",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7106566,
            "range": "± 144482",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 27288843,
            "range": "± 719541",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 52335883,
            "range": "± 1330488",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 245965295,
            "range": "± 4712311",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 411168023,
            "range": "± 5691380",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 402737039,
            "range": "± 5637780",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 402328093,
            "range": "± 5418766",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 410553038,
            "range": "± 8181701",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 412561844,
            "range": "± 5603290",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 531902,
            "range": "± 7692",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 538399,
            "range": "± 6685",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 553210,
            "range": "± 6127",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 151384,
            "range": "± 1344",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 370347,
            "range": "± 2789",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1326282,
            "range": "± 3383",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 885744,
            "range": "± 35010",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 922793,
            "range": "± 44650",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 923298,
            "range": "± 23019",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 48,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/linear",
            "value": 80,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 187,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 268,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 361858102,
            "range": "± 6355511",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 145907316,
            "range": "± 1969170",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 146379010,
            "range": "± 2410550",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 146620944,
            "range": "± 2503667",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 146041606,
            "range": "± 2089836",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 182372607,
            "range": "± 4424981",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 239154462,
            "range": "± 8105235",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 487859410,
            "range": "± 9506638",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 121425,
            "range": "± 2892",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 122069,
            "range": "± 2226",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 121038,
            "range": "± 3022",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 375715315,
            "range": "± 4376029",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 312011165,
            "range": "± 4662502",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 268725079,
            "range": "± 5062616",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 242313107,
            "range": "± 4803801",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 33304864,
            "range": "± 2562151",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 269601784,
            "range": "± 5085499",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 242851853,
            "range": "± 5371187",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 245820221,
            "range": "± 5728465",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 81801265,
            "range": "± 2418603",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 89461003,
            "range": "± 2805621",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 165210870,
            "range": "± 6057840",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 242168207,
            "range": "± 11282125",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 395302331,
            "range": "± 15958851",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1180446,
            "range": "± 139523",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9460485,
            "range": "± 1368033",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 49734588,
            "range": "± 1787678",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 130350,
            "range": "± 3172",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 220145,
            "range": "± 4119",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 601904,
            "range": "± 6504",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 140008,
            "range": "± 3845",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 198987,
            "range": "± 3385",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 455838,
            "range": "± 7387",
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
          "id": "bac4602c1ad92b2d641ab9663c6d6e2889845903",
          "message": "perf: skip transaction for retry requeue and tune slow benchmarks (#79)\n\n## Summary\n\n- Add `requeue_for_retry` to `TaskStore` that executes a bare `UPDATE`\non the pool instead of `BEGIN IMMEDIATE` → `UPDATE` → `COMMIT`, cutting\nconnection hold time by 2/3 for the retry path. With `open_memory()`'s\nsingle-connection pool, this lets `pop_next_batch` dispatch interleave\nsooner with concurrent failure writes.\n- Split `handle_failure` into a fast path (`requeue_for_retry` for\nwill-retry) and the existing transactional `fail_with_record` for\nterminal failures that require multi-statement atomicity (INSERT history\n+ DELETE task).\n- Tune criterion config on `query_by_tags` and `retryable_dead_letter`\nbenchmark groups (`sample_size(10)`, `warm_up_time(1s)`,\n`measurement_time(3s)`) to reduce wall-clock time ~50%.\n\n## Benchmark results\n\n`retryable_dead_letter` throughput improved **~2.5x** across all four\nbackoff strategies:\n\n| Strategy | Before | After | Change |\n|---|---|---|---|\n| constant | ~900 elem/s | 2,305 elem/s | **+154%** |\n| linear | ~900 elem/s | 2,325 elem/s | **+167%** |\n| exponential | ~900 elem/s | 2,316 elem/s | **+162%** |\n| exponential_jitter | ~900 elem/s | 2,320 elem/s | **+154%** |",
          "timestamp": "2026-03-23T15:52:59Z",
          "tree_id": "a951df11d50151fea2eee6540d6687a4c60814ac",
          "url": "https://github.com/deepjoy/taskmill/commit/bac4602c1ad92b2d641ab9663c6d6e2889845903"
        },
        "date": 1774282783927,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3059325,
            "range": "± 153670",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17397159,
            "range": "± 912622",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 79177810,
            "range": "± 5471697",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11413622,
            "range": "± 118663",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 28101417,
            "range": "± 441652",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 56543963,
            "range": "± 825576",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7273119,
            "range": "± 68582",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 28531837,
            "range": "± 1015181",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 55634045,
            "range": "± 1004436",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 257057523,
            "range": "± 5587493",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 451420950,
            "range": "± 6511443",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 442344548,
            "range": "± 9287250",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 445299924,
            "range": "± 6234566",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 445738599,
            "range": "± 6139200",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 443225079,
            "range": "± 6061199",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 541568,
            "range": "± 11454",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 551630,
            "range": "± 8163",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 558849,
            "range": "± 8977",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 154832,
            "range": "± 1936",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 369386,
            "range": "± 1276",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 1331181,
            "range": "± 6857",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 883698,
            "range": "± 22615",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 927588,
            "range": "± 49473",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 913121,
            "range": "± 28288",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 46,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/linear",
            "value": 81,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 181,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 266,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 390659048,
            "range": "± 5025881",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 138294342,
            "range": "± 685213",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 137318443,
            "range": "± 1342137",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 137387492,
            "range": "± 1329813",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 138344909,
            "range": "± 2099760",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 186832972,
            "range": "± 7850090",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 247023115,
            "range": "± 12711623",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 509984698,
            "range": "± 11817469",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 125442,
            "range": "± 4471",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 124996,
            "range": "± 5194",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 126370,
            "range": "± 4201",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 391817095,
            "range": "± 7228829",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 328538479,
            "range": "± 5366435",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 286858385,
            "range": "± 6455474",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 256622181,
            "range": "± 4449187",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 36144734,
            "range": "± 2847967",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 285732751,
            "range": "± 5471788",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 255221723,
            "range": "± 6361128",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 258325314,
            "range": "± 5508583",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 86756658,
            "range": "± 2440873",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 92230633,
            "range": "± 4226730",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 172337499,
            "range": "± 9918721",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 256262884,
            "range": "± 15157487",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 411061149,
            "range": "± 22491704",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/100",
            "value": 1235513,
            "range": "± 132826",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/1000",
            "value": 9595887,
            "range": "± 422342",
            "unit": "ns/iter"
          },
          {
            "name": "query_by_tags/5000",
            "value": 52470499,
            "range": "± 3246582",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 132133,
            "range": "± 4518",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 221485,
            "range": "± 7485",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 614871,
            "range": "± 6610",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 138322,
            "range": "± 5515",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 199577,
            "range": "± 6467",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 477574,
            "range": "± 6808",
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
          "id": "dc7823ad4dffbcc5a063c31a7a72659f173e1392",
          "message": "perf!: improve benchmark throughput across submit, dispatch, retry, and failure paths (#80)\n\n## Summary\n\nReduces per-task overhead across every scheduler hot path — submit,\ndispatch, retry, completion, and failure — through transaction\ncoalescing, lazy data population, inline zero-delay retries, and\nquery-only-what-you-need optimizations. Also brings all markdown docs\nand rustdoc up to date with the 0.6 API.\n\n### Breaking change\n\n- `tasks_by_tags()` → `task_ids_by_tags()` and\n`tasks_by_tag_key_prefix()` → `task_ids_by_tag_key_prefix()` — both now\nreturn `Vec<i64>` instead of `Vec<TaskRecord>`. Callers that need full\nrecords must follow up with `task_by_id()`.\n\n### Performance improvements\n\n- **Submit coalescing** — concurrent `submit()` calls are batched into a\nsingle SQLite transaction via leader election; uncontended callers take\na zero-overhead fast path\n- **Skip requeue on fresh stores** — dedup-hit path elides the requeue\nUPDATE when no task has ever been dispatched (`has_running` flag)\n- **Batch terminal failures** — parentless terminal failures are\ncoalesced through an unbounded channel (mirroring completion\ncoalescing), amortizing WAL sync; parent failures still process inline\nto preserve fail-fast cascade ordering\n- **Inline zero-delay retries** — retries with zero delay re-execute in\nthe same spawned task instead of requeueing through SQLite\n- **Lazy tag population** — `pop_next`/`peek_next`/history list queries\nno longer JOIN tags by default; callers opt in via `populate_tags()` /\n`populate_history_tags()`\n- **Covering index for history** — new `idx_history_type(task_type,\ncompleted_at DESC)` speeds up `history_stats`, `history_by_type`, and\n`avg_throughput`\n- **Widen completion coalescing window** — `yield_now()` before\nleader-election drain lets more completions accumulate per batch\n- **Skip no-subscriber broadcasts** — gate `event_tx.send()` behind\n`receiver_count() > 0`\n\n\n### Documentation\n\n- Updated all code examples across 13 markdown files and `lib.rs`\nrustdoc for the 0.6 API (`DomainTaskContext`, `spawn_child_with`,\n`child_of`)\n- Documented tag query rename, covering index, dead_letter history\nstatus, and inline retry flow\n- Bumped stale version strings (`0.3`/`0.4`/`0.5` → `0.6`)",
          "timestamp": "2026-03-23T22:01:16-07:00",
          "tree_id": "474c3971fd1b05bb8b209e0000416052b314ea1f",
          "url": "https://github.com/deepjoy/taskmill/commit/dc7823ad4dffbcc5a063c31a7a72659f173e1392"
        },
        "date": 1774330003902,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3055555,
            "range": "± 145260",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16918185,
            "range": "± 843470",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 78789890,
            "range": "± 5302579",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11152139,
            "range": "± 251269",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 27297994,
            "range": "± 475836",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 55099551,
            "range": "± 1020479",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 6063840,
            "range": "± 99067",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 22918033,
            "range": "± 905684",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 43552123,
            "range": "± 988715",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 195194107,
            "range": "± 4912234",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 433229413,
            "range": "± 7737225",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 432806487,
            "range": "± 8296772",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 431680331,
            "range": "± 6763747",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 434174553,
            "range": "± 6788779",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 431067070,
            "range": "± 6248630",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 414625,
            "range": "± 24310",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 427941,
            "range": "± 17345",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 430089,
            "range": "± 18882",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 130921,
            "range": "± 2209",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 197108,
            "range": "± 1772",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 487046,
            "range": "± 3224",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 226527,
            "range": "± 7104",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 785112,
            "range": "± 50078",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 792749,
            "range": "± 42875",
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
            "value": 449,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 372802194,
            "range": "± 6533715",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 115911901,
            "range": "± 1335786",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 115474673,
            "range": "± 685012",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 115031184,
            "range": "± 936680",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 116013170,
            "range": "± 1799846",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 187881972,
            "range": "± 5992540",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 215888986,
            "range": "± 7865200",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 390693984,
            "range": "± 6968065",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 129856,
            "range": "± 6013",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 126627,
            "range": "± 3894",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 125154,
            "range": "± 3973",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 394084951,
            "range": "± 5700299",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 288221201,
            "range": "± 6392553",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 239363364,
            "range": "± 14434679",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 193654029,
            "range": "± 5015997",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 35149795,
            "range": "± 3076868",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 241159796,
            "range": "± 8215041",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 193245046,
            "range": "± 4409207",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 198748802,
            "range": "± 4549005",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 74193349,
            "range": "± 2868955",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 92401390,
            "range": "± 4091587",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 168140127,
            "range": "± 9199659",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 245574274,
            "range": "± 12875377",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 405241526,
            "range": "± 23096714",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 191380,
            "range": "± 6309",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 824845,
            "range": "± 20142",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3505662,
            "range": "± 137818",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 133165,
            "range": "± 4968",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 221428,
            "range": "± 5594",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 601900,
            "range": "± 7046",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 137710,
            "range": "± 5726",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 197972,
            "range": "± 6534",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 463937,
            "range": "± 8146",
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
          "id": "614f8318a5dde21a12c8d9bbb20b6f883b55c57a",
          "message": "feat!: implement group-level pause and resume (#81)\n\n## Summary\n\n- Add `PauseReasons` bitmask (`PREEMPTION | MODULE | GLOBAL | GROUP`) so\nmultiple\npause sources can coexist without stranding tasks — a task only resumes\nwhen\n*all* reasons are cleared. Retrofit every existing pause/resume path\n(preemption,\nmodule, global) to use bitmask operations instead of bare status flips.\n- Add `paused_groups` SQLite table and `pause_reasons` column on\n`tasks`, with\n  in-memory `HashSet` mirror for fast gate checks during dispatch.\n- Implement `Scheduler::pause_group` / `resume_group` /\n`pause_group_until` with\nfull lifecycle: persist state → update in-memory set → pause pending\ntasks →\ncancel running tasks → emit `GroupPaused`/`GroupResumed` events.\nTime-boxed\n  pauses auto-resume via a throttled (5 s) run-loop check.\n- Gate admission rejects tasks from paused groups; new submissions are\naccepted\nbut inserted directly as paused with the GROUP bit. Recurring\nnext-instances and\nblocked→pending transitions also check `paused_groups` and downgrade\naccordingly.\n- Expose group pause API on `DomainHandle` and `ModuleHandle` (delegates\nto scheduler).\n- Add 420-line integration test suite covering submit-to-paused-group,\nrecurring\ndowngrade, blocked→paused transition, multi-reason interaction, and\nhandle delegation.\n\n## Breaking changes\n\n- `SubmitOutcome::Inserted(id)` → `SubmitOutcome::Inserted { id,\ngroup_paused }` —\n  callers must update pattern matches.\n- `TaskStore::pause(id)` now requires a `PauseReasons` argument:\n`pause(id, reason)`.\n- `PauseReasons` and `PausedGroupInfo` added to public re-exports.",
          "timestamp": "2026-03-23T22:05:31-07:00",
          "tree_id": "bdeb7b3306a60c70f4ae88d2853657380202da07",
          "url": "https://github.com/deepjoy/taskmill/commit/614f8318a5dde21a12c8d9bbb20b6f883b55c57a"
        },
        "date": 1774330269005,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3093527,
            "range": "± 166333",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17246195,
            "range": "± 921934",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 78125109,
            "range": "± 5964905",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 11640265,
            "range": "± 110600",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 28554224,
            "range": "± 484735",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 57334632,
            "range": "± 990637",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 6178963,
            "range": "± 109306",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 22661572,
            "range": "± 734413",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 43384499,
            "range": "± 1171646",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 197191516,
            "range": "± 5378111",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 447131624,
            "range": "± 7358615",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 447250134,
            "range": "± 6943465",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 448758457,
            "range": "± 7053428",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 448873630,
            "range": "± 7770420",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 448962001,
            "range": "± 7733849",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 413951,
            "range": "± 21558",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 426352,
            "range": "± 25205",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 426482,
            "range": "± 21005",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 130526,
            "range": "± 1491",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 197640,
            "range": "± 2106",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 473935,
            "range": "± 2543",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 222738,
            "range": "± 6275",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 804090,
            "range": "± 47090",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 793027,
            "range": "± 80121",
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
            "value": 76,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 187,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 451,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 376277394,
            "range": "± 5968727",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 116295831,
            "range": "± 789264",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 116013677,
            "range": "± 1042543",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 116102340,
            "range": "± 1391829",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 115202514,
            "range": "± 717566",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 187749557,
            "range": "± 7334986",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 217691618,
            "range": "± 8790773",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 392932273,
            "range": "± 6595448",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 127307,
            "range": "± 5350",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 123267,
            "range": "± 3208",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 124613,
            "range": "± 4969",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 390062815,
            "range": "± 5181018",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 285246546,
            "range": "± 5521081",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 241722155,
            "range": "± 6508677",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 193600081,
            "range": "± 4156443",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 34642112,
            "range": "± 3044486",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 240534575,
            "range": "± 5990612",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 195343137,
            "range": "± 4777855",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 200919508,
            "range": "± 5450310",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 86378597,
            "range": "± 2403453",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 93132086,
            "range": "± 4423369",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 171226283,
            "range": "± 9290178",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 250507926,
            "range": "± 14002082",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 408635952,
            "range": "± 23482759",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 188804,
            "range": "± 10517",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 821557,
            "range": "± 32741",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3561047,
            "range": "± 68106",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 132455,
            "range": "± 5271",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 220794,
            "range": "± 7054",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 613781,
            "range": "± 9266",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 135933,
            "range": "± 5738",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 198235,
            "range": "± 5625",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 469920,
            "range": "± 7781",
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
          "id": "3d9e158ba554c9f6ce54677babc29a1a5cb2ad46",
          "message": "feat!: implement token-bucket rate limiting per task type and group (#82)\n\n## Summary\n\n- Add in-memory token-bucket rate limiting that caps task **start rate**\nindependently of concurrency, scoped by task type and/or group key\n- Rate limit checks are gate-native (acquire-last in\n`DefaultDispatchGate::admit()`) so tokens are never wasted on tasks\nrejected by earlier checks (backpressure, IO budget, concurrency)\n- Rate-limited tasks get `run_after` set to the next token availability,\npreventing head-of-line blocking — other task types dispatch freely\nwhile the throttled type waits\n\nCloses #35 \n\n## Breaking changes\n\n- `DispatchGate::admit()` returns `Admission` enum (`Admit` / `Deny` /\n`RateLimited(Instant)`) instead of `bool` — custom gate implementations\nmust update `Ok(true)` → `Admission::Admit` and `Ok(false)` →\n`Admission::Deny`\n- `SchedulerSnapshot` gains a `rate_limits: Vec<RateLimitInfo>` field\n\n## Public API additions\n\n- **Types:** `RateLimit` (config with `per_second`, `per_minute`,\n`with_burst`), `RateLimitInfo` (snapshot), `Admission` (gate result)\n- **Builder:** `SchedulerBuilder::rate_limit()`, `group_rate_limit()`\n- **Runtime:** `Scheduler::set_rate_limit()`, `remove_rate_limit()`,\n`set_group_rate_limit()`, `remove_group_rate_limit()`\n- **Handles:** same four methods on `DomainHandle` and `ModuleHandle`\n(auto-prefixed)\n- **Store:** `TaskStore::set_run_after()` for deferred requeue\n- **Snapshot:** `SchedulerSnapshot::rate_limits` with per-bucket\nutilization\n\n## Files changed\n\n| Area | Files |\n|------|-------|\n| Core | `rate_limit.rs` (new), `gate.rs`, `mod.rs`, `run_loop.rs` |\n| API surface | `builder.rs`, `control.rs`, `event.rs`, `queries.rs` |\n| Delegation | `module.rs`, `domain.rs`, `lib.rs` |\n| Store | `store/query/scheduling.rs` |\n| Tests | `tests/integration/rate_limit.rs` (new, 9 tests),\n`tests/integration.rs` |\n| Docs | `README.md`, `configuration.md`,\n`priorities-and-preemption.md`, `glossary.md` |",
          "timestamp": "2026-03-24T06:09:52Z",
          "tree_id": "b4a622bee871ded375d6b425be64daafa085dd09",
          "url": "https://github.com/deepjoy/taskmill/commit/3d9e158ba554c9f6ce54677babc29a1a5cb2ad46"
        },
        "date": 1774334135870,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3099685,
            "range": "± 154403",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17643030,
            "range": "± 1041697",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 85094862,
            "range": "± 4820661",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 12350567,
            "range": "± 131138",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 30815800,
            "range": "± 453660",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 61517102,
            "range": "± 998942",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 6559856,
            "range": "± 84850",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 23881112,
            "range": "± 546177",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 45422772,
            "range": "± 1123675",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 202655270,
            "range": "± 5959593",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 445327413,
            "range": "± 9761002",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 446497572,
            "range": "± 8528553",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 447174165,
            "range": "± 9723743",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 449190109,
            "range": "± 9473331",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 478456548,
            "range": "± 7029993",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 437327,
            "range": "± 18281",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 430436,
            "range": "± 15862",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 423334,
            "range": "± 18329",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 131815,
            "range": "± 1282",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 197841,
            "range": "± 2504",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 487746,
            "range": "± 4508",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 225772,
            "range": "± 7283",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 854243,
            "range": "± 44664",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 820750,
            "range": "± 49591",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 50,
            "range": "± 1",
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
            "value": 189,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 268,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 379735737,
            "range": "± 5978470",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 116632224,
            "range": "± 848452",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 117097070,
            "range": "± 1048692",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 117534898,
            "range": "± 1268243",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 116555781,
            "range": "± 1595033",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 187915346,
            "range": "± 7871784",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 217466245,
            "range": "± 9072899",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 391091614,
            "range": "± 9933310",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 132683,
            "range": "± 5447",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 133384,
            "range": "± 5692",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 134064,
            "range": "± 6206",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 420912177,
            "range": "± 7361730",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 290109863,
            "range": "± 8277990",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 241650810,
            "range": "± 7301911",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 194723738,
            "range": "± 5141282",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 35718276,
            "range": "± 3450383",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 244924784,
            "range": "± 7506588",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 194820064,
            "range": "± 5261680",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 200060369,
            "range": "± 5637226",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 86440843,
            "range": "± 3745761",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 96977334,
            "range": "± 5197495",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 186590730,
            "range": "± 9140889",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 267629599,
            "range": "± 13236655",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 422970223,
            "range": "± 23182114",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 189517,
            "range": "± 3383",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 773626,
            "range": "± 8410",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3430669,
            "range": "± 65852",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 132086,
            "range": "± 5004",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 225729,
            "range": "± 5062",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 625117,
            "range": "± 17989",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 141578,
            "range": "± 6557",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 201300,
            "range": "± 7220",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 464339,
            "range": "± 9897",
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
          "id": "cd2a5aceb4886645e00d7b7c42e3e6ac826a6e4b",
          "message": "refactor: consolidate paused_groups migration into standard include_str! pattern (#83)\n\n## Summary\n\n- Rename `migrations/010_paused_groups.sql` to `005_paused_groups.sql`\nto follow the sequential numbering of existing migrations (001–004)\n- Replace inline SQL in `TaskStore::migrate()` with `include_str!()`,\nmatching the pattern used by all other migrations\n- Remove the now-unnecessary `ALTER TABLE` and backfill `UPDATE`\nstatements since the DB is always created fresh with the `pause_reasons`\ncolumn already present in `001_tasks.sql`",
          "timestamp": "2026-03-24T06:22:38Z",
          "tree_id": "5ff89f1805a9c463f260ba604980ad3cdbecef5a",
          "url": "https://github.com/deepjoy/taskmill/commit/cd2a5aceb4886645e00d7b7c42e3e6ac826a6e4b"
        },
        "date": 1774334873496,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 2957500,
            "range": "± 105996",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16266351,
            "range": "± 720057",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 74974397,
            "range": "± 3595610",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 10834906,
            "range": "± 215809",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 26396809,
            "range": "± 371227",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 52605406,
            "range": "± 740631",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 5839606,
            "range": "± 269723",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 21306586,
            "range": "± 429129",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 40732058,
            "range": "± 1048568",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 179325186,
            "range": "± 5061539",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 415865365,
            "range": "± 6620458",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 415783232,
            "range": "± 7801042",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 412522901,
            "range": "± 6552935",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 413494184,
            "range": "± 7379412",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 414039201,
            "range": "± 11161602",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 434761,
            "range": "± 18146",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 433574,
            "range": "± 19663",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 425452,
            "range": "± 23757",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 126384,
            "range": "± 1004",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 191606,
            "range": "± 2489",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 474754,
            "range": "± 2090",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 219599,
            "range": "± 5585",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 814499,
            "range": "± 30522",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 807335,
            "range": "± 59748",
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
            "value": 80,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 187,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 269,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 347036965,
            "range": "± 7665078",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 105371192,
            "range": "± 1029343",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 104596915,
            "range": "± 877762",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 105200242,
            "range": "± 802908",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 105613757,
            "range": "± 929309",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 180934870,
            "range": "± 4458690",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 206503229,
            "range": "± 8199384",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 357347563,
            "range": "± 4707317",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 119279,
            "range": "± 7858",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 119962,
            "range": "± 2799",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 119685,
            "range": "± 2246",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 373821558,
            "range": "± 3880033",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 276291363,
            "range": "± 6302260",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 231804264,
            "range": "± 9351427",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 177657017,
            "range": "± 4770549",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 32802434,
            "range": "± 2336705",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 228251896,
            "range": "± 8531101",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 178590714,
            "range": "± 5227900",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 193031288,
            "range": "± 3352971",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 80720455,
            "range": "± 2156471",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 89305284,
            "range": "± 3343060",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 163816277,
            "range": "± 6835477",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 235701322,
            "range": "± 10801111",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 393077622,
            "range": "± 18558080",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 186429,
            "range": "± 1879",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 808388,
            "range": "± 4976",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3544956,
            "range": "± 50744",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 126629,
            "range": "± 2981",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 215260,
            "range": "± 2974",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 651521,
            "range": "± 27321",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 133091,
            "range": "± 3093",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 192995,
            "range": "± 3449",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 454163,
            "range": "± 4962",
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
          "id": "cb48f0c1256236d6df0447d921be5e0e844cbab7",
          "message": "feat!: implement priority aging and weighted fair scheduling (#84)\n\n## Summary\n\n- **Priority aging (phase 1):** Tasks waiting longer than a configurable\ngrace period are gradually promoted in effective priority at dispatch\ntime, preventing starvation of low-priority work under sustained\nhigh-priority load. The stored priority is never mutated — effective\npriority is computed in SQL via the aging formula. Pause duration is\nexcluded from the aging clock so paused tasks don't get unfair\npromotion.\n- **Weighted fair scheduling (phase 2):** Three-pass dispatch loop\nallocates slots proportional to per-group weights, fills remaining\ncapacity greedily, and dispatches urgently-aged tasks as a safety valve.\nUngrouped tasks compete as a virtual group with the default weight.\nMin-slots guarantees and concurrency caps are respected. Work-conserving\nby construction.\n- **Composes with existing features:** Rate limits, group pause,\nconcurrency caps, preemption, and backpressure all interact correctly.\nFast dispatch is disabled when aging or weights are configured.\n\nCloses #37 \n### New public API\n\n| Builder | Runtime | Event |\n|---------|---------|-------|\n| `priority_aging(AgingConfig)` | — | `base_priority` /\n`effective_priority` on `TaskEventHeader` |\n| `group_weight(group, weight)` | `set_group_weight(group, weight)` |\n`GroupWeightChanged` |\n| `default_group_weight(weight)` | `remove_group_weight(group)` | — |\n| `group_minimum_slots(group, slots)` | `reset_group_weights()` | — |\n| — | `set_group_minimum_slots(group, slots)` | — |\n\n### New store queries\n\n`peek_next_in_group()`, `peek_next_ungrouped()`,\n`running_counts_per_group()`, `pending_counts_per_group()`,\n`peek_next_urgent()`\n\n### Schema changes (pre-1.0, inline)\n\n`pause_duration_ms INTEGER NOT NULL DEFAULT 0` and `paused_at_ms INTEGER\nDEFAULT NULL` added to `tasks` table for aging clock management.",
          "timestamp": "2026-03-24T13:54:06Z",
          "tree_id": "a69875db55277aa2cb4d1b03279410877097011e",
          "url": "https://github.com/deepjoy/taskmill/commit/cb48f0c1256236d6df0447d921be5e0e844cbab7"
        },
        "date": 1774361959752,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 2994629,
            "range": "± 119635",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16539712,
            "range": "± 679589",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 75527340,
            "range": "± 3502260",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 10776768,
            "range": "± 158516",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 26595138,
            "range": "± 555128",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 53066602,
            "range": "± 801684",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 5967263,
            "range": "± 140772",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 21480621,
            "range": "± 322893",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 40740166,
            "range": "± 871724",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 181985892,
            "range": "± 3897421",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 411250431,
            "range": "± 8533573",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 418532107,
            "range": "± 7604186",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 418351315,
            "range": "± 7432948",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 418128609,
            "range": "± 7525560",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 420578345,
            "range": "± 8042104",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 415911,
            "range": "± 21284",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 430931,
            "range": "± 22588",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 446065,
            "range": "± 21436",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 125870,
            "range": "± 1062",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 191592,
            "range": "± 967",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 484888,
            "range": "± 2224",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 219848,
            "range": "± 5061",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 843324,
            "range": "± 71721",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 846639,
            "range": "± 48931",
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
            "value": 190,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 408,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 350350482,
            "range": "± 5801110",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 107101225,
            "range": "± 994467",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 107288635,
            "range": "± 971965",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 107591356,
            "range": "± 1734611",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 107255551,
            "range": "± 940668",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 180619749,
            "range": "± 5683567",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 207273115,
            "range": "± 5836832",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 368755468,
            "range": "± 6608829",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 120388,
            "range": "± 2861",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 120783,
            "range": "± 2931",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 121991,
            "range": "± 2889",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 376794935,
            "range": "± 4709907",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 276716111,
            "range": "± 4924905",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 230008840,
            "range": "± 5801206",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 182572966,
            "range": "± 3863321",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 33971129,
            "range": "± 2607801",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 232861594,
            "range": "± 5448178",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 182488155,
            "range": "± 4177663",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 194353213,
            "range": "± 3974322",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 82493414,
            "range": "± 2215694",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 90179341,
            "range": "± 3487763",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 162849878,
            "range": "± 6498020",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 237357368,
            "range": "± 10016994",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 389023485,
            "range": "± 16790958",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 184795,
            "range": "± 3165",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 811340,
            "range": "± 11141",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3703318,
            "range": "± 69905",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 126476,
            "range": "± 3125",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 216543,
            "range": "± 4583",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 610454,
            "range": "± 7087",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 131804,
            "range": "± 3024",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 195314,
            "range": "± 3396",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 459783,
            "range": "± 4947",
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
          "id": "88dd2aa097d400af719c33d840382d89ba30eea9",
          "message": "docs: add migration guide for 0.7.0 (#85)\n\n## Summary\n- Adds `docs/migrating-to-0.7.md` covering all breaking changes and new\nfeatures since 0.6 (15 commits from `d903234..HEAD`)\n- Documents 5 breaking changes: database recreation (timestamp\nnormalization + migration consolidation), typed Memo on `TypedExecutor`,\n`SubmitOutcome::Inserted` struct variant, `DispatchGate::admit()`\nreturning `Admission` enum, and tag query method renames\n- Documents 6 new features: group pause/resume, token-bucket rate\nlimiting, priority aging, weighted fair scheduling, `fail_fast()`\nbuilder method, and tag key prefix queries\n- Includes before/after Rust code examples and import guidance, matching\nexisting migration doc style",
          "timestamp": "2026-03-24T14:11:08Z",
          "tree_id": "c04f3b6c666f5818a23480bc4509b6228b8ee40d",
          "url": "https://github.com/deepjoy/taskmill/commit/88dd2aa097d400af719c33d840382d89ba30eea9"
        },
        "date": 1774362982485,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 2996443,
            "range": "± 106484",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16299341,
            "range": "± 646925",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 74990741,
            "range": "± 4127266",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 10775726,
            "range": "± 139930",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 26177083,
            "range": "± 369228",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 52748727,
            "range": "± 724978",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 5855652,
            "range": "± 81610",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 21264970,
            "range": "± 245895",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 40405794,
            "range": "± 884016",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 178402661,
            "range": "± 4436413",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 409237878,
            "range": "± 17428081",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 410967123,
            "range": "± 5741915",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 412420460,
            "range": "± 6347128",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 412229133,
            "range": "± 7116491",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 414414569,
            "range": "± 5783592",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 420058,
            "range": "± 21360",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 420385,
            "range": "± 17019",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 423719,
            "range": "± 26793",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 125765,
            "range": "± 864",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 190021,
            "range": "± 901",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 477663,
            "range": "± 3262",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 220241,
            "range": "± 6442",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 796504,
            "range": "± 44943",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 799534,
            "range": "± 55972",
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
            "value": 190,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 449,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 345917170,
            "range": "± 3846552",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 106500305,
            "range": "± 1237868",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 106021839,
            "range": "± 1155961",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 104886559,
            "range": "± 1147002",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 105720788,
            "range": "± 792761",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 177820552,
            "range": "± 5016736",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 206806280,
            "range": "± 5501078",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 357682688,
            "range": "± 6014056",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 121155,
            "range": "± 3007",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 118020,
            "range": "± 2937",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 121771,
            "range": "± 3362",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 366720229,
            "range": "± 3864539",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 272218306,
            "range": "± 3855248",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 228201954,
            "range": "± 7566091",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 177945561,
            "range": "± 3611909",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 33697943,
            "range": "± 2060362",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 224940248,
            "range": "± 7140988",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 176666518,
            "range": "± 3163807",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 190004100,
            "range": "± 3411555",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 80569778,
            "range": "± 2267935",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 89092018,
            "range": "± 2649644",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 161327809,
            "range": "± 6592636",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 235731717,
            "range": "± 9506364",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 383071238,
            "range": "± 15258602",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 185273,
            "range": "± 2029",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 812492,
            "range": "± 10040",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3657405,
            "range": "± 26044",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 127750,
            "range": "± 2692",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 215511,
            "range": "± 3090",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 599177,
            "range": "± 5027",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 132360,
            "range": "± 3538",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 192981,
            "range": "± 3087",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 455213,
            "range": "± 5023",
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
          "id": "685f93f0f504257cdd8f355669de07d2b1d5ce22",
          "message": "feat: implement observability metrics export (#20) (#88)\n\n## Summary\n\n- Add always-on internal `AtomicU64` counters and a public\n`MetricsSnapshot` struct for consumers who don't use the `metrics` crate\n(CLIs, TUI dashboards)\n- Add optional `metrics` crate integration (behind the `metrics` Cargo\nfeature) that emits ~30 counters, gauges, and histograms via the\nstandard facade — consumers choose their exporter (Prometheus, StatsD,\nDatadog)\n- Add `SchedulerBuilder` methods for customizing metric names\n(`metrics_prefix`), global labels (`metrics_label`), and suppressing\nspecific metrics (`disable_metric`)\n- Instrument all scheduler code paths: submit, dispatch, completion,\nfailure, retry, dead-letter, gate denial, rate-limit throttle, group\npause/resume, expiry, and dependency failure\n- Add duration tracking through `CompletionMsg`/`FailureMsg` for\nexecution time histograms and queue wait histograms at dispatch time\n\nCloses #20 \n## New files\n\n| File | Purpose |\n|------|---------|\n| `src/scheduler/counters.rs` | `SchedulerCounters` (always-on atomics)\n+ `MetricsSnapshot` public struct |\n| `src/scheduler/metrics_bridge.rs` | `MetricsEmitter` — feature-gated\n`metrics` crate facade wrapper |\n| `tests/integration/metrics.rs` | 8 integration tests for counter\ncorrectness |\n| `docs/metrics.md` | User-facing guide: metric reference, dashboard\nlayout, alert rules, builder API |\n\n## Modified files (13)\n\n| File | Change |\n|------|--------|\n| `Cargo.toml` | `metrics = { version = \"0.24\", optional = true }`,\n`metrics` feature flag |\n| `src/lib.rs` | Re-export `MetricsSnapshot`, feature flag + metrics\ncrate docs |\n| `src/scheduler/mod.rs` | `counters`/`metrics_bridge` modules,\n`MetricsConfig`, `duration` on coalescing messages, new fields on\n`SchedulerInner` |\n| `src/scheduler/builder.rs` | `metrics_prefix()`, `metrics_label()`,\n`disable_metric()` builder methods; `describe_metrics()` at build time |\n| `src/scheduler/queries.rs` | `Scheduler::metrics_snapshot()` method |\n| `src/scheduler/gate.rs` | `counters` in `GateContext`; `gate_denials`\n+ `rate_limit_throttles` at each denial path |\n| `src/scheduler/run_loop.rs` | Gauge updates in `poll_and_dispatch()`;\nexpired counter; rate limit token gauges |\n| `src/scheduler/spawn.rs` | Dispatch counter + queue wait histogram;\nduration capture; inline retry counters |\n| `src/scheduler/spawn/context.rs` | `counters` + `emitter` in\n`SpawnContext` |\n| `src/scheduler/spawn/completion.rs` | Completion counter + duration\nhistogram |\n| `src/scheduler/spawn/failure.rs` |\nFailure/retry/dead-letter/dependency counters + duration histogram |\n| `src/scheduler/submit.rs` | Submit/supersede/batch counters + emitter\ncalls |\n| `src/scheduler/control.rs` | Group pause/resume counters + emitter\ncalls |\n\n## Design decisions\n\n- **Dual-emit**: each instrumentation point increments an `AtomicU64`\n(always) AND calls the `MetricsEmitter` (only with `#[cfg(feature =\n\"metrics\")]`). The atomics serve non-`metrics` consumers; the `metrics`\ncrate adds labels, histograms, and gauge semantics.\n- **Zero-cost when unused**: all `metrics::*` calls are behind\n`#[cfg(feature = \"metrics\")]`. Internal counters cost a few cache lines\nof atomics with `Relaxed` ordering.\n- **Bounded label cardinality**: only `type`, `module`, `group`, and\n`reason` appear as labels. Never `task_id`, `key`, or user-provided\n`tags`.\n- **Inline retry coverage**: the zero-delay inline retry path in\n`spawn.rs` now increments `failed`, `failed_retryable`, and `retried`\ncounters (previously this fast path bypassed failure accounting).",
          "timestamp": "2026-03-24T14:46:16Z",
          "tree_id": "96edc9075366f2006e4f19b91206e7aa66420a87",
          "url": "https://github.com/deepjoy/taskmill/commit/685f93f0f504257cdd8f355669de07d2b1d5ce22"
        },
        "date": 1774365168742,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3101969,
            "range": "± 172907",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17140077,
            "range": "± 1012599",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 80586446,
            "range": "± 4946503",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 16241984,
            "range": "± 162008",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 40776006,
            "range": "± 586873",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 80645858,
            "range": "± 1310160",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7333527,
            "range": "± 134323",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 24342201,
            "range": "± 532833",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 45320357,
            "range": "± 1240829",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 209133478,
            "range": "± 4902807",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 456429885,
            "range": "± 8339581",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 457235175,
            "range": "± 8271837",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 455774382,
            "range": "± 6208380",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 455636032,
            "range": "± 7624812",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 454363401,
            "range": "± 7398033",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 413757,
            "range": "± 19695",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 417034,
            "range": "± 24517",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 437292,
            "range": "± 21724",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 130093,
            "range": "± 1118",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 198083,
            "range": "± 2361",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 490453,
            "range": "± 3257",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 220892,
            "range": "± 7494",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 796318,
            "range": "± 65181",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 795880,
            "range": "± 44733",
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
            "value": 76,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential",
            "value": 186,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 406,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 380337311,
            "range": "± 6194071",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 120050486,
            "range": "± 1734743",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 118760292,
            "range": "± 1354642",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 118606415,
            "range": "± 1518623",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 117923373,
            "range": "± 1720858",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 186004144,
            "range": "± 7120715",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 210812349,
            "range": "± 10333060",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 416894931,
            "range": "± 7994559",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 122485,
            "range": "± 4178",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 124476,
            "range": "± 4862",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 123904,
            "range": "± 5141",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 637391895,
            "range": "± 7867014",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 418558779,
            "range": "± 8061913",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 253652600,
            "range": "± 6096857",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 209207552,
            "range": "± 5671262",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 34971088,
            "range": "± 3566660",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 253685972,
            "range": "± 5274443",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 208164916,
            "range": "± 5060692",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 206778106,
            "range": "± 5552537",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 90845698,
            "range": "± 2914081",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 90956666,
            "range": "± 4209331",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 168993502,
            "range": "± 11226772",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 246196371,
            "range": "± 14318157",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 403365427,
            "range": "± 24182833",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 189577,
            "range": "± 5219",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 805530,
            "range": "± 9057",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3499381,
            "range": "± 35632",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 131786,
            "range": "± 3659",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 220195,
            "range": "± 5528",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 608710,
            "range": "± 6889",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 134886,
            "range": "± 5980",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 195942,
            "range": "± 6310",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 465081,
            "range": "± 6490",
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
          "id": "449e79e3ec46855d8465fc7ed0f2814800f57cba",
          "message": "chore: release v0.7.0 (#70)",
          "timestamp": "2026-03-24T14:51:27Z",
          "tree_id": "37c00540f3325727351f5712074664af4f6a78c0",
          "url": "https://github.com/deepjoy/taskmill/commit/449e79e3ec46855d8465fc7ed0f2814800f57cba"
        },
        "date": 1774365483032,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 2952924,
            "range": "± 89034",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16223104,
            "range": "± 740487",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 75016349,
            "range": "± 3021326",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 14946818,
            "range": "± 308804",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 37017066,
            "range": "± 622659",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 73265642,
            "range": "± 2244279",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 6913618,
            "range": "± 133371",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 22463464,
            "range": "± 582525",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 42322361,
            "range": "± 825320",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 195514424,
            "range": "± 3393891",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 417138934,
            "range": "± 7569684",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 411178312,
            "range": "± 9096896",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 419108745,
            "range": "± 5951292",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 418499371,
            "range": "± 5229329",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 418726193,
            "range": "± 5423097",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 429557,
            "range": "± 19344",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 423712,
            "range": "± 21091",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 432981,
            "range": "± 25303",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 124419,
            "range": "± 727",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 189676,
            "range": "± 974",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 472564,
            "range": "± 1653",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 219777,
            "range": "± 5834",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 770706,
            "range": "± 50629",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 778710,
            "range": "± 37165",
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
            "value": 196,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 408,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 347025403,
            "range": "± 4580671",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 108777142,
            "range": "± 1024056",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 108521332,
            "range": "± 1295391",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 108155138,
            "range": "± 985146",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 108048870,
            "range": "± 848166",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 177781457,
            "range": "± 5762893",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 202757794,
            "range": "± 5809938",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 392371999,
            "range": "± 5674280",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 118095,
            "range": "± 1830",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 117492,
            "range": "± 2720",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 118063,
            "range": "± 2773",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 602643586,
            "range": "± 8937519",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 384289734,
            "range": "± 9106625",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 233639795,
            "range": "± 4105002",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 193994300,
            "range": "± 3465989",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 32652171,
            "range": "± 2460194",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 234714781,
            "range": "± 4070588",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 196046518,
            "range": "± 3458480",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 196010047,
            "range": "± 3423344",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 85201836,
            "range": "± 2755875",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 87602396,
            "range": "± 3206733",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 163535327,
            "range": "± 7469570",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 236554207,
            "range": "± 12403380",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 381600162,
            "range": "± 19004194",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 181164,
            "range": "± 3807",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 812934,
            "range": "± 10905",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3586378,
            "range": "± 18101",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 124301,
            "range": "± 2865",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 213691,
            "range": "± 3398",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 599037,
            "range": "± 4826",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 129630,
            "range": "± 3167",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 189966,
            "range": "± 4312",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 453763,
            "range": "± 4830",
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
          "id": "99af25b697a7308ff0c155a13bf45db70aa7d67b",
          "message": "feat: implement sibling task spawning with automatic parent-ID inheritance (#89)\n\n## Summary\n\n- Add `DomainTaskContext::spawn_sibling_with()` and\n`spawn_siblings_with()` so child tasks can spawn peer tasks under the\nsame parent without manually threading the orchestrator's task ID\n- Add `DomainSubmitBuilder::sibling_of()` for cross-domain sibling\nspawning\n- Add `ModuleHandle::submit_batch()` for single-transaction batch\nsubmissions, and wire `spawn_children_with` through it as a perf\nimprovement\n- Add `TaskRecord::remaining_ttl()` for correct TTL inheritance from\nparent records\n- Re-export `SiblingSpawnBuilder` from crate root\n- Add quick-start docs section covering sibling spawning patterns\n\nCloses #87 \n\n## Motivation\n\nWhen child executors need to spawn peer tasks (e.g. BFS directory\nscans), they previously had to manually extract `parent_id` from the\ncontext record and pass it to `submit_with().parent(id)`. This was\nerror-prone and verbose. The new `spawn_sibling_with()` API mirrors the\nexisting `spawn_child_with()` ergonomics and returns\n`StoreError::InvalidState` if called from a root task, preventing silent\ncreation of unparented tasks.\n\n## Changes\n\n| File | Change |\n|---|---|\n| `src/registry/domain_context.rs` | `SiblingSpawnBuilder`,\n`spawn_sibling_with()`, `spawn_siblings_with()` |\n| `src/domain.rs` | `DomainSubmitBuilder::sibling_of()` for cross-domain\nsiblings |\n| `src/module.rs` | `ModuleHandle::submit_batch()` for\nsingle-transaction batch path |\n| `src/registry/context.rs` | Wire `spawn_children_with` through\n`submit_batch` |\n| `src/task/mod.rs` | `TaskRecord::remaining_ttl()` helper |\n| `src/task/submit_builder.rs` | `SubmitBuilder::resolve_only()`\n(crate-internal) |\n| `src/lib.rs` | Re-export `SiblingSpawnBuilder`, update module docs |\n| `docs/quick-start.md` | Sibling tasks section with examples and\nparent-relationship table |\n| `examples/test_sibling.rs` | Runnable example demonstrating orc →\nchild → sibling flow |\n| `tests/integration/sibling_spawn.rs` | 685-line integration test suite\ncovering inheritance, error cases, batch, cross-domain, priority aging,\nTTL, tags, dedup, and finalize paths |",
          "timestamp": "2026-03-25T01:50:37Z",
          "tree_id": "7b660d8b4738bc08db2bf3ddb04e7b29d4bafe67",
          "url": "https://github.com/deepjoy/taskmill/commit/99af25b697a7308ff0c155a13bf45db70aa7d67b"
        },
        "date": 1774405003686,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 3196482,
            "range": "± 224770",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 17556534,
            "range": "± 1031332",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 80989965,
            "range": "± 4890004",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 16322448,
            "range": "± 314379",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 39655115,
            "range": "± 2016137",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 77381430,
            "range": "± 1316433",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 7330141,
            "range": "± 186010",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 24947065,
            "range": "± 773603",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 45613370,
            "range": "± 2048396",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 206534482,
            "range": "± 4342028",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 445288574,
            "range": "± 13617956",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 444792439,
            "range": "± 10989181",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 448514943,
            "range": "± 11051408",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 450013659,
            "range": "± 11566843",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 444918884,
            "range": "± 9289587",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 449391,
            "range": "± 15776",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 431241,
            "range": "± 19381",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 463889,
            "range": "± 13714",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 130093,
            "range": "± 1803",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 199931,
            "range": "± 1694",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 498668,
            "range": "± 4921",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 226094,
            "range": "± 5004",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 841152,
            "range": "± 58237",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 806348,
            "range": "± 32937",
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
            "value": 407,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 365117231,
            "range": "± 8527638",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 114104887,
            "range": "± 1656149",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 113756424,
            "range": "± 2027337",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 116099295,
            "range": "± 2664131",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 114753882,
            "range": "± 2031870",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 189481816,
            "range": "± 6637027",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 213186274,
            "range": "± 6901478",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 415086162,
            "range": "± 11116496",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 125044,
            "range": "± 4562",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 123393,
            "range": "± 3644",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 122265,
            "range": "± 5645",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 633250226,
            "range": "± 12711153",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 410090252,
            "range": "± 13554770",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 244164662,
            "range": "± 6236489",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 206299558,
            "range": "± 6318001",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 33890825,
            "range": "± 2260118",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 249557402,
            "range": "± 5036234",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 205641678,
            "range": "± 6191285",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 210291027,
            "range": "± 7272187",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 91376890,
            "range": "± 3638516",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 93000101,
            "range": "± 4074280",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 169938311,
            "range": "± 8058010",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 250352539,
            "range": "± 12226109",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 401412714,
            "range": "± 17924959",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 198470,
            "range": "± 5681",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 832166,
            "range": "± 34632",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3740144,
            "range": "± 57461",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 133845,
            "range": "± 3237",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 221152,
            "range": "± 7822",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 604193,
            "range": "± 14333",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 137583,
            "range": "± 4338",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 202881,
            "range": "± 7001",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 459068,
            "range": "± 7047",
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
          "id": "59ee1bf6210d1d10a2d188aeefb5ac933344e48a",
          "message": "chore: release v0.7.1 (#90)\n\n## 🤖 New release\n\n* `taskmill`: 0.7.0 -> 0.7.1 (✓ API compatible changes)\n\n<details><summary><i><b>Changelog</b></i></summary><p>\n\n<blockquote>\n\n## [0.7.1](https://github.com/deepjoy/taskmill/compare/v0.7.0...v0.7.1)\n- 2026-03-25\n\n### Added\n\n- implement sibling task spawning with automatic parent-ID inheritance\n([#89](https://github.com/deepjoy/taskmill/pull/89))\n</blockquote>\n\n\n</p></details>\n\n---\nThis PR was generated with\n[release-plz](https://github.com/release-plz/release-plz/).\n\nCo-authored-by: github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>",
          "timestamp": "2026-03-25T01:54:41Z",
          "tree_id": "e03d6cc32db0da25badf072d958b2b80c1155756",
          "url": "https://github.com/deepjoy/taskmill/commit/59ee1bf6210d1d10a2d188aeefb5ac933344e48a"
        },
        "date": 1774405221483,
        "tool": "cargo",
        "benches": [
          {
            "name": "dep_chain_submit/10",
            "value": 2980684,
            "range": "± 106383",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/50",
            "value": 16394942,
            "range": "± 769157",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_submit/200",
            "value": 74878269,
            "range": "± 4333080",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/10",
            "value": 14865215,
            "range": "± 378591",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/25",
            "value": 36706531,
            "range": "± 813581",
            "unit": "ns/iter"
          },
          {
            "name": "dep_chain_dispatch/50",
            "value": 73556786,
            "range": "± 1116844",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/10",
            "value": 6969440,
            "range": "± 58501",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/50",
            "value": 22765207,
            "range": "± 513774",
            "unit": "ns/iter"
          },
          {
            "name": "dep_fan_in_dispatch/100",
            "value": 42691042,
            "range": "± 751606",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_no_groups/500",
            "value": 198584383,
            "range": "± 3666976",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_one_group/500",
            "value": 428970617,
            "range": "± 6006062",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/1",
            "value": 427542952,
            "range": "± 5950529",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/10",
            "value": 427712215,
            "range": "± 6138513",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/50",
            "value": 430899731,
            "range": "± 6275250",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_group_scaling/100",
            "value": 431597259,
            "range": "± 5602802",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/100",
            "value": 427861,
            "range": "± 20880",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/1000",
            "value": 420891,
            "range": "± 27708",
            "unit": "ns/iter"
          },
          {
            "name": "history_query/5000",
            "value": 419287,
            "range": "± 17751",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/100",
            "value": 124893,
            "range": "± 1049",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/1000",
            "value": 192963,
            "range": "± 1145",
            "unit": "ns/iter"
          },
          {
            "name": "history_stats/5000",
            "value": 492720,
            "range": "± 1686",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/100",
            "value": 218486,
            "range": "± 5092",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/1000",
            "value": 782290,
            "range": "± 36542",
            "unit": "ns/iter"
          },
          {
            "name": "history_by_type/5000",
            "value": 828046,
            "range": "± 36675",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/constant",
            "value": 44,
            "range": "± 1",
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
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "backoff_delay/exponential_jitter",
            "value": 407,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_permanent_failure/500",
            "value": 352527170,
            "range": "± 6837741",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/constant",
            "value": 111973147,
            "range": "± 1246660",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/linear",
            "value": 109610312,
            "range": "± 1312241",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential",
            "value": 110183709,
            "range": "± 1599958",
            "unit": "ns/iter"
          },
          {
            "name": "retryable_dead_letter/exponential_jitter",
            "value": 110195926,
            "range": "± 1723905",
            "unit": "ns/iter"
          },
          {
            "name": "submit_tasks/1000",
            "value": 178713028,
            "range": "± 4980349",
            "unit": "ns/iter"
          },
          {
            "name": "submit_dedup_hit/1000",
            "value": 202526012,
            "range": "± 5594123",
            "unit": "ns/iter"
          },
          {
            "name": "dispatch_and_complete/1000",
            "value": 394578324,
            "range": "± 6270995",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/100",
            "value": 117201,
            "range": "± 3659",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/1000",
            "value": 117793,
            "range": "± 2908",
            "unit": "ns/iter"
          },
          {
            "name": "peek_next/5000",
            "value": 118521,
            "range": "± 5107",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/1",
            "value": 596476421,
            "range": "± 11365683",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/2",
            "value": 378165490,
            "range": "± 11241403",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/4",
            "value": 234280468,
            "range": "± 4674083",
            "unit": "ns/iter"
          },
          {
            "name": "concurrency_scaling/8",
            "value": 195583611,
            "range": "± 3301953",
            "unit": "ns/iter"
          },
          {
            "name": "batch_submit/1000",
            "value": 32956325,
            "range": "± 3235434",
            "unit": "ns/iter"
          },
          {
            "name": "mixed_priority_dispatch/500",
            "value": 235132038,
            "range": "± 5030263",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/noop_500",
            "value": 195848016,
            "range": "± 5007678",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress/byte_reporting_500",
            "value": 195822669,
            "range": "± 4066965",
            "unit": "ns/iter"
          },
          {
            "name": "byte_progress_snapshot/100_tasks",
            "value": 84999414,
            "range": "± 2942659",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/0",
            "value": 87934786,
            "range": "± 2644356",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/5",
            "value": 160218149,
            "range": "± 8980575",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/10",
            "value": 234727538,
            "range": "± 10858562",
            "unit": "ns/iter"
          },
          {
            "name": "submit_with_tags/20",
            "value": 380565908,
            "range": "± 19485144",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/100",
            "value": 182108,
            "range": "± 2210",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/1000",
            "value": 807216,
            "range": "± 7371",
            "unit": "ns/iter"
          },
          {
            "name": "query_ids_by_tags/5000",
            "value": 3736999,
            "range": "± 95495",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/100",
            "value": 124304,
            "range": "± 6765",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/1000",
            "value": 211922,
            "range": "± 3373",
            "unit": "ns/iter"
          },
          {
            "name": "count_by_tags/5000",
            "value": 598705,
            "range": "± 3912",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/100",
            "value": 130068,
            "range": "± 3950",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/1000",
            "value": 188033,
            "range": "± 4153",
            "unit": "ns/iter"
          },
          {
            "name": "tag_values/5000",
            "value": 451974,
            "range": "± 7906",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}