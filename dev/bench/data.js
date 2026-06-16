window.BENCHMARK_DATA = {
  "lastUpdate": 1781615785011,
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
      },
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
          "id": "faa6eab0238d194fcb792db36a7a49bd611bba51",
          "message": "updated pipelines",
          "timestamp": "2026-06-16T16:06:41+03:00",
          "tree_id": "89288e380a9eef612ad7e2b92e930aea60aa9b03",
          "url": "https://github.com/EmreErinc/envforge/commit/faa6eab0238d194fcb792db36a7a49bd611bba51"
        },
        "date": 1781615784364,
        "tool": "cargo",
        "benches": [
          {
            "name": "parser/roundtrip_small",
            "value": 2060,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "parser/roundtrip_large_500_exports",
            "value": 794804,
            "range": "± 8971",
            "unit": "ns/iter"
          },
          {
            "name": "sync/compute_diff_small",
            "value": 380,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "security/redact_secrets_in_message",
            "value": 390,
            "range": "± 4",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}