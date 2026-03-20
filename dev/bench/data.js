window.BENCHMARK_DATA = {
  "lastUpdate": 1774039146610,
  "repoUrl": "https://github.com/sscargal/libslic3r-rs",
  "entries": {
    "libslic3r-rs Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "37674041+sscargal@users.noreply.github.com",
            "name": "Steve Scargall",
            "username": "sscargal"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "d78844c0c9a454b7580ac2c9658f78658deda125",
          "message": "Merge pull request #9 from sscargal/phase-40-cli-progress\n\nPhase 40 cli progress",
          "timestamp": "2026-03-19T17:10:42-06:00",
          "tree_id": "63752548ec07b3cdd7e5fe40bc99b49ecb335143",
          "url": "https://github.com/sscargal/libslic3r-rs/commit/d78844c0c9a454b7580ac2c9658f78658deda125"
        },
        "date": 1773962621071,
        "tool": "cargo",
        "benches": [
          {
            "name": "slice_calibration_cube",
            "value": 5973372,
            "range": "± 73032",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cylinder_64sides",
            "value": 98049411,
            "range": "± 1403865",
            "unit": "ns/iter"
          },
          {
            "name": "slice_dense_sphere_1280tri",
            "value": 3795749,
            "range": "± 116146",
            "unit": "ns/iter"
          },
          {
            "name": "slice_thin_wall_box",
            "value": 9896935,
            "range": "± 85755",
            "unit": "ns/iter"
          },
          {
            "name": "slice_multi_overhang",
            "value": 9305713,
            "range": "± 87397",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cube_full_config",
            "value": 657265540,
            "range": "± 2905681",
            "unit": "ns/iter"
          },
          {
            "name": "memory_estimate_cube",
            "value": 6070328,
            "range": "± 193135",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_union_overlapping",
            "value": 1489,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_intersection_overlapping",
            "value": 1103,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_difference_overlapping",
            "value": 1214,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_outward_2mm",
            "value": 10661,
            "range": "± 83",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_inward_2mm",
            "value": 10245,
            "range": "± 73",
            "unit": "ns/iter"
          },
          {
            "name": "offset_rect_collapse",
            "value": 1727,
            "range": "± 12",
            "unit": "ns/iter"
          },
          {
            "name": "point_in_polygon_inside",
            "value": 95,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "point_in_polygon_outside",
            "value": 90,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "point_in_polygon_boundary",
            "value": 95,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "slice_mesh_sphere_1280tri_0.2mm",
            "value": 1575365,
            "range": "± 6096",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_ray_intersect_100rays",
            "value": 50270,
            "range": "± 139",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/sequential/40mm_cube",
            "value": 9450997,
            "range": "± 62504",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_auto/40mm_cube",
            "value": 11016203,
            "range": "± 28794",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_4_threads/40mm_cube",
            "value": 10985680,
            "range": "± 94957",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/small_box_12tri",
            "value": 71112,
            "range": "± 1010",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/small_box_12tri",
            "value": 69258,
            "range": "± 497",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/small_box_12tri",
            "value": 66235,
            "range": "± 450",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/medium_sphere_16seg",
            "value": 1669095,
            "range": "± 7469",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/medium_sphere_16seg",
            "value": 1610694,
            "range": "± 32439",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/medium_sphere_16seg",
            "value": 1547664,
            "range": "± 8694",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/large_sphere_32seg",
            "value": 8038882,
            "range": "± 54040",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/large_sphere_32seg",
            "value": 7442547,
            "range": "± 60209",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/large_sphere_32seg",
            "value": 7062102,
            "range": "± 51610",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/xl_sphere_64seg",
            "value": 40533585,
            "range": "± 142876",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/xl_sphere_64seg",
            "value": 32930758,
            "range": "± 254793",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/xl_sphere_64seg",
            "value": 29431044,
            "range": "± 109363",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/box",
            "value": 198,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/sphere_32seg",
            "value": 20020,
            "range": "± 75",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/cylinder_32seg",
            "value": 2596,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/torus_32x16",
            "value": 20732,
            "range": "± 32",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/16seg",
            "value": 68055,
            "range": "± 177",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/32seg",
            "value": 261093,
            "range": "± 1122",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/64seg",
            "value": 1035401,
            "range": "± 3938",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/box",
            "value": 53541,
            "range": "± 133",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/sphere_32seg",
            "value": 7169005,
            "range": "± 38274",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/16seg",
            "value": 89664,
            "range": "± 954",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/32seg",
            "value": 514826,
            "range": "± 1505",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/64seg",
            "value": 2715642,
            "range": "± 6350",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "37674041+sscargal@users.noreply.github.com",
            "name": "Steve Scargall",
            "username": "sscargal"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ee30a62974778679cb23986d1a0e04f5d24fa81a",
          "message": "Merge pull request #10 from sscargal/phase-41-travel-move-optimization\n\nPhase 41 travel move optimization",
          "timestamp": "2026-03-20T13:48:54-06:00",
          "tree_id": "8472fa7cf5f5320dccaaacd7a79e9e998ecd01eb",
          "url": "https://github.com/sscargal/libslic3r-rs/commit/ee30a62974778679cb23986d1a0e04f5d24fa81a"
        },
        "date": 1774039146178,
        "tool": "cargo",
        "benches": [
          {
            "name": "slice_calibration_cube",
            "value": 7582949,
            "range": "± 283349",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cylinder_64sides",
            "value": 117134471,
            "range": "± 1080342",
            "unit": "ns/iter"
          },
          {
            "name": "slice_dense_sphere_1280tri",
            "value": 3770946,
            "range": "± 16360",
            "unit": "ns/iter"
          },
          {
            "name": "slice_thin_wall_box",
            "value": 16215933,
            "range": "± 173616",
            "unit": "ns/iter"
          },
          {
            "name": "slice_multi_overhang",
            "value": 12258643,
            "range": "± 199564",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cube_full_config",
            "value": 23383852089,
            "range": "± 408582218",
            "unit": "ns/iter"
          },
          {
            "name": "memory_estimate_cube",
            "value": 7747262,
            "range": "± 48860",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_union_overlapping",
            "value": 1484,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_intersection_overlapping",
            "value": 1111,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_difference_overlapping",
            "value": 1239,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_outward_2mm",
            "value": 10523,
            "range": "± 41",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_inward_2mm",
            "value": 10376,
            "range": "± 30",
            "unit": "ns/iter"
          },
          {
            "name": "offset_rect_collapse",
            "value": 1693,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "point_in_polygon_inside",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "point_in_polygon_outside",
            "value": 88,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "point_in_polygon_boundary",
            "value": 93,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "slice_mesh_sphere_1280tri_0.2mm",
            "value": 1599525,
            "range": "± 35905",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_ray_intersect_100rays",
            "value": 50816,
            "range": "± 693",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/sequential/40mm_cube",
            "value": 11367191,
            "range": "± 62863",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_auto/40mm_cube",
            "value": 13747820,
            "range": "± 45794",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_4_threads/40mm_cube",
            "value": 13806634,
            "range": "± 71795",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/small_box_12tri",
            "value": 71640,
            "range": "± 2711",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/small_box_12tri",
            "value": 69694,
            "range": "± 429",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/small_box_12tri",
            "value": 66618,
            "range": "± 294",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/medium_sphere_16seg",
            "value": 1696230,
            "range": "± 22348",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/medium_sphere_16seg",
            "value": 1629601,
            "range": "± 14494",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/medium_sphere_16seg",
            "value": 1568840,
            "range": "± 17270",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/large_sphere_32seg",
            "value": 8108252,
            "range": "± 33540",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/large_sphere_32seg",
            "value": 7527715,
            "range": "± 27280",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/large_sphere_32seg",
            "value": 7140453,
            "range": "± 61270",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/xl_sphere_64seg",
            "value": 40984863,
            "range": "± 1260067",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/xl_sphere_64seg",
            "value": 33460121,
            "range": "± 143612",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/xl_sphere_64seg",
            "value": 29897279,
            "range": "± 84906",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/box",
            "value": 196,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/sphere_32seg",
            "value": 20029,
            "range": "± 97",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/cylinder_32seg",
            "value": 2595,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/torus_32x16",
            "value": 20533,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/16seg",
            "value": 68758,
            "range": "± 270",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/32seg",
            "value": 262826,
            "range": "± 947",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/64seg",
            "value": 1043748,
            "range": "± 3120",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/box",
            "value": 54062,
            "range": "± 125",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/sphere_32seg",
            "value": 7287897,
            "range": "± 22217",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/16seg",
            "value": 87234,
            "range": "± 1131",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/32seg",
            "value": 512663,
            "range": "± 4435",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/64seg",
            "value": 2762645,
            "range": "± 28145",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}