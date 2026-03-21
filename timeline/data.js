window.BENCHMARK_DATA = {
  "lastUpdate": 1774085916355,
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
      }
    ]
  }
}