window.BENCHMARK_DATA = {
  "lastUpdate": 1773847116414,
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
      }
    ]
  }
}