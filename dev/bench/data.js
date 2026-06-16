window.BENCHMARK_DATA = {
  "lastUpdate": 1781613158665,
  "repoUrl": "https://github.com/EmreErinc/envforge",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "emreerinc@emre-MacBook-Air.local",
            "name": "emre erinc"
          },
          "committer": {
            "email": "emreerinc@emre-MacBook-Air.local",
            "name": "emre erinc"
          },
          "distinct": true,
          "id": "1f0d866a2aa5396c30b683ca447aa2596e171345",
          "message": "fix cargo format issue",
          "timestamp": "2026-06-16T15:03:29+03:00",
          "tree_id": "8905201d63e5bf33d01fc55b8f8172e60d57dfea",
          "url": "https://github.com/EmreErinc/envforge/commit/1f0d866a2aa5396c30b683ca447aa2596e171345"
        },
        "date": 1781613158295,
        "tool": "cargo",
        "benches": [
          {
            "name": "parser/roundtrip_small",
            "value": 2069,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "parser/roundtrip_large_500_exports",
            "value": 789979,
            "range": "± 3387",
            "unit": "ns/iter"
          },
          {
            "name": "sync/compute_diff_small",
            "value": 381,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "security/redact_secrets_in_message",
            "value": 392,
            "range": "± 1",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}