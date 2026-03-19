window.BENCHMARK_DATA = {
  "lastUpdate": 1773899141290,
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
      }
    ]
  }
}