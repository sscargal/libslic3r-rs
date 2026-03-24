window.BENCHMARK_DATA = {
  "lastUpdate": 1774387021827,
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
          "id": "49410133bc39b043166a063dc726aa620829f214",
          "message": "Merge pull request #11 from sscargal/add-profile-subcommands\n\nAdd profile subcommands",
          "timestamp": "2026-03-20T17:38:08-06:00",
          "tree_id": "734f527875b632e269a106980f8f344313f86be3",
          "url": "https://github.com/sscargal/libslic3r-rs/commit/49410133bc39b043166a063dc726aa620829f214"
        },
        "date": 1774052809212,
        "tool": "cargo",
        "benches": [
          {
            "name": "slice_calibration_cube",
            "value": 7881454,
            "range": "± 72341",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cylinder_64sides",
            "value": 117343780,
            "range": "± 717217",
            "unit": "ns/iter"
          },
          {
            "name": "slice_dense_sphere_1280tri",
            "value": 3784089,
            "range": "± 122086",
            "unit": "ns/iter"
          },
          {
            "name": "slice_thin_wall_box",
            "value": 16256717,
            "range": "± 125762",
            "unit": "ns/iter"
          },
          {
            "name": "slice_multi_overhang",
            "value": 12303087,
            "range": "± 103502",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cube_full_config",
            "value": 22960705675,
            "range": "± 216121343",
            "unit": "ns/iter"
          },
          {
            "name": "memory_estimate_cube",
            "value": 7780849,
            "range": "± 84098",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_union_overlapping",
            "value": 1457,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_intersection_overlapping",
            "value": 1083,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_difference_overlapping",
            "value": 1225,
            "range": "± 57",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_outward_2mm",
            "value": 10536,
            "range": "± 24",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_inward_2mm",
            "value": 10328,
            "range": "± 36",
            "unit": "ns/iter"
          },
          {
            "name": "offset_rect_collapse",
            "value": 1728,
            "range": "± 47",
            "unit": "ns/iter"
          },
          {
            "name": "point_in_polygon_inside",
            "value": 93,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "point_in_polygon_outside",
            "value": 88,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "point_in_polygon_boundary",
            "value": 93,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "slice_mesh_sphere_1280tri_0.2mm",
            "value": 1571655,
            "range": "± 5492",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_ray_intersect_100rays",
            "value": 50029,
            "range": "± 184",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/sequential/40mm_cube",
            "value": 11513840,
            "range": "± 91036",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_auto/40mm_cube",
            "value": 13718220,
            "range": "± 69115",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_4_threads/40mm_cube",
            "value": 13708589,
            "range": "± 67333",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/small_box_12tri",
            "value": 72036,
            "range": "± 521",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/small_box_12tri",
            "value": 70310,
            "range": "± 2429",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/small_box_12tri",
            "value": 67215,
            "range": "± 473",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/medium_sphere_16seg",
            "value": 1711614,
            "range": "± 9228",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/medium_sphere_16seg",
            "value": 1644336,
            "range": "± 30618",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/medium_sphere_16seg",
            "value": 1581093,
            "range": "± 7319",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/large_sphere_32seg",
            "value": 8166308,
            "range": "± 50969",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/large_sphere_32seg",
            "value": 7578792,
            "range": "± 72495",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/large_sphere_32seg",
            "value": 7191575,
            "range": "± 63840",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/xl_sphere_64seg",
            "value": 40880129,
            "range": "± 99025",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/xl_sphere_64seg",
            "value": 33383105,
            "range": "± 810562",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/xl_sphere_64seg",
            "value": 29853896,
            "range": "± 84614",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/box",
            "value": 202,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/sphere_32seg",
            "value": 20038,
            "range": "± 47",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/cylinder_32seg",
            "value": 2606,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/torus_32x16",
            "value": 20520,
            "range": "± 46",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/16seg",
            "value": 68434,
            "range": "± 864",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/32seg",
            "value": 262097,
            "range": "± 1117",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/64seg",
            "value": 1040984,
            "range": "± 21819",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/box",
            "value": 54546,
            "range": "± 1728",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/sphere_32seg",
            "value": 7249635,
            "range": "± 42148",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/16seg",
            "value": 92786,
            "range": "± 1605",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/32seg",
            "value": 517056,
            "range": "± 3800",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/64seg",
            "value": 2710262,
            "range": "± 5029",
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
          "id": "f46b2af6db4aacfccd667e6e83069136240ff43b",
          "message": "Merge pull request #13 from sscargal/quick-260321-1s9-qa-tests\n\nAdd more QA tests",
          "timestamp": "2026-03-23T11:25:33-06:00",
          "tree_id": "88dee65e4b3f40ee962985b25aa61049886aec6c",
          "url": "https://github.com/sscargal/libslic3r-rs/commit/f46b2af6db4aacfccd667e6e83069136240ff43b"
        },
        "date": 1774289684112,
        "tool": "cargo",
        "benches": [
          {
            "name": "slice_calibration_cube",
            "value": 7746886,
            "range": "± 106814",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cylinder_64sides",
            "value": 118060369,
            "range": "± 2890284",
            "unit": "ns/iter"
          },
          {
            "name": "slice_dense_sphere_1280tri",
            "value": 3777249,
            "range": "± 17243",
            "unit": "ns/iter"
          },
          {
            "name": "slice_thin_wall_box",
            "value": 16225392,
            "range": "± 374888",
            "unit": "ns/iter"
          },
          {
            "name": "slice_multi_overhang",
            "value": 12432247,
            "range": "± 93995",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cube_full_config",
            "value": 23387996397,
            "range": "± 232749486",
            "unit": "ns/iter"
          },
          {
            "name": "memory_estimate_cube",
            "value": 7726294,
            "range": "± 67212",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_union_overlapping",
            "value": 1461,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_intersection_overlapping",
            "value": 1100,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_difference_overlapping",
            "value": 1238,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_outward_2mm",
            "value": 10921,
            "range": "± 52",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_inward_2mm",
            "value": 10500,
            "range": "± 62",
            "unit": "ns/iter"
          },
          {
            "name": "offset_rect_collapse",
            "value": 1726,
            "range": "± 11",
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
            "range": "± 1",
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
            "value": 1596404,
            "range": "± 9487",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_ray_intersect_100rays",
            "value": 51279,
            "range": "± 348",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/sequential/40mm_cube",
            "value": 11485980,
            "range": "± 43124",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_auto/40mm_cube",
            "value": 13754952,
            "range": "± 62872",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_4_threads/40mm_cube",
            "value": 13798741,
            "range": "± 46409",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/small_box_12tri",
            "value": 71567,
            "range": "± 1307",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/small_box_12tri",
            "value": 69578,
            "range": "± 1699",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/small_box_12tri",
            "value": 66485,
            "range": "± 516",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/medium_sphere_16seg",
            "value": 1674810,
            "range": "± 6949",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/medium_sphere_16seg",
            "value": 1606596,
            "range": "± 18858",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/medium_sphere_16seg",
            "value": 1547812,
            "range": "± 12467",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/large_sphere_32seg",
            "value": 8052870,
            "range": "± 33101",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/large_sphere_32seg",
            "value": 7454959,
            "range": "± 24398",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/large_sphere_32seg",
            "value": 7063836,
            "range": "± 126671",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/xl_sphere_64seg",
            "value": 40574733,
            "range": "± 110443",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/xl_sphere_64seg",
            "value": 33206946,
            "range": "± 138280",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/xl_sphere_64seg",
            "value": 29665761,
            "range": "± 132618",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/box",
            "value": 195,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/sphere_32seg",
            "value": 20125,
            "range": "± 270",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/cylinder_32seg",
            "value": 2602,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/torus_32x16",
            "value": 20676,
            "range": "± 89",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/16seg",
            "value": 68441,
            "range": "± 658",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/32seg",
            "value": 261408,
            "range": "± 918",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/64seg",
            "value": 1040050,
            "range": "± 11001",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/box",
            "value": 53762,
            "range": "± 245",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/sphere_32seg",
            "value": 7155365,
            "range": "± 26927",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/16seg",
            "value": 87333,
            "range": "± 366",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/32seg",
            "value": 518272,
            "range": "± 8979",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/64seg",
            "value": 2720193,
            "range": "± 14280",
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
          "id": "8f6d67089c6e54b38de7284c1c7623394e588422",
          "message": "Merge pull request #12 from sscargal/dependabot/cargo/cargo-64b2a50fd2\n\nchore(deps): bump rustls-webpki from 0.103.9 to 0.103.10 in the cargo group across 1 directory",
          "timestamp": "2026-03-23T11:25:54-06:00",
          "tree_id": "836b4fe579d3d5b2194b8969936cc3486ae65427",
          "url": "https://github.com/sscargal/libslic3r-rs/commit/8f6d67089c6e54b38de7284c1c7623394e588422"
        },
        "date": 1774289726696,
        "tool": "cargo",
        "benches": [
          {
            "name": "slice_calibration_cube",
            "value": 7617551,
            "range": "± 87928",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cylinder_64sides",
            "value": 117599622,
            "range": "± 1691779",
            "unit": "ns/iter"
          },
          {
            "name": "slice_dense_sphere_1280tri",
            "value": 3750738,
            "range": "± 15867",
            "unit": "ns/iter"
          },
          {
            "name": "slice_thin_wall_box",
            "value": 16268210,
            "range": "± 149795",
            "unit": "ns/iter"
          },
          {
            "name": "slice_multi_overhang",
            "value": 12395389,
            "range": "± 74624",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cube_full_config",
            "value": 22856234326,
            "range": "± 340610005",
            "unit": "ns/iter"
          },
          {
            "name": "memory_estimate_cube",
            "value": 7856106,
            "range": "± 66703",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_union_overlapping",
            "value": 1455,
            "range": "± 19",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_intersection_overlapping",
            "value": 1111,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_difference_overlapping",
            "value": 1236,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_outward_2mm",
            "value": 10769,
            "range": "± 30",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_inward_2mm",
            "value": 10506,
            "range": "± 24",
            "unit": "ns/iter"
          },
          {
            "name": "offset_rect_collapse",
            "value": 1700,
            "range": "± 5",
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
            "value": 1605873,
            "range": "± 3903",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_ray_intersect_100rays",
            "value": 51276,
            "range": "± 264",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/sequential/40mm_cube",
            "value": 11487104,
            "range": "± 67338",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_auto/40mm_cube",
            "value": 13688986,
            "range": "± 100873",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_4_threads/40mm_cube",
            "value": 13722181,
            "range": "± 102904",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/small_box_12tri",
            "value": 71530,
            "range": "± 537",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/small_box_12tri",
            "value": 69642,
            "range": "± 491",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/small_box_12tri",
            "value": 66564,
            "range": "± 507",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/medium_sphere_16seg",
            "value": 1687794,
            "range": "± 18435",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/medium_sphere_16seg",
            "value": 1617662,
            "range": "± 12264",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/medium_sphere_16seg",
            "value": 1552573,
            "range": "± 8523",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/large_sphere_32seg",
            "value": 8059715,
            "range": "± 30898",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/large_sphere_32seg",
            "value": 7480246,
            "range": "± 28980",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/large_sphere_32seg",
            "value": 7061107,
            "range": "± 35999",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/xl_sphere_64seg",
            "value": 40733482,
            "range": "± 119595",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/xl_sphere_64seg",
            "value": 33040784,
            "range": "± 107445",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/xl_sphere_64seg",
            "value": 29600125,
            "range": "± 195748",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/box",
            "value": 195,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/sphere_32seg",
            "value": 20081,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/cylinder_32seg",
            "value": 2631,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/torus_32x16",
            "value": 20560,
            "range": "± 33",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/16seg",
            "value": 68779,
            "range": "± 197",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/32seg",
            "value": 261561,
            "range": "± 660",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/64seg",
            "value": 1060661,
            "range": "± 4296",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/box",
            "value": 53766,
            "range": "± 324",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/sphere_32seg",
            "value": 7193866,
            "range": "± 42744",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/16seg",
            "value": 89395,
            "range": "± 368",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/32seg",
            "value": 523737,
            "range": "± 2607",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/64seg",
            "value": 2719810,
            "range": "± 10721",
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
          "id": "431364831642b3e5fce9f08642b1f44543ff2aa2",
          "message": "Merge pull request #14 from sscargal/phase-44-profile-sets-cli\n\nPhase 44 profile sets cli",
          "timestamp": "2026-03-23T17:47:20-06:00",
          "tree_id": "0c8798bda84e816d793d36847cc6eb3077c66fdb",
          "url": "https://github.com/sscargal/libslic3r-rs/commit/431364831642b3e5fce9f08642b1f44543ff2aa2"
        },
        "date": 1774312499716,
        "tool": "cargo",
        "benches": [
          {
            "name": "slice_calibration_cube",
            "value": 7664303,
            "range": "± 55074",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cylinder_64sides",
            "value": 117442805,
            "range": "± 3569812",
            "unit": "ns/iter"
          },
          {
            "name": "slice_dense_sphere_1280tri",
            "value": 3695795,
            "range": "± 93311",
            "unit": "ns/iter"
          },
          {
            "name": "slice_thin_wall_box",
            "value": 16079688,
            "range": "± 1032264",
            "unit": "ns/iter"
          },
          {
            "name": "slice_multi_overhang",
            "value": 12244273,
            "range": "± 109629",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cube_full_config",
            "value": 22185692475,
            "range": "± 210953684",
            "unit": "ns/iter"
          },
          {
            "name": "memory_estimate_cube",
            "value": 7837038,
            "range": "± 39679",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_union_overlapping",
            "value": 1467,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_intersection_overlapping",
            "value": 1118,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_difference_overlapping",
            "value": 1229,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_outward_2mm",
            "value": 10658,
            "range": "± 102",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_inward_2mm",
            "value": 10193,
            "range": "± 19",
            "unit": "ns/iter"
          },
          {
            "name": "offset_rect_collapse",
            "value": 1745,
            "range": "± 14",
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
            "value": 1585342,
            "range": "± 3273",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_ray_intersect_100rays",
            "value": 50140,
            "range": "± 441",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/sequential/40mm_cube",
            "value": 11234460,
            "range": "± 37594",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_auto/40mm_cube",
            "value": 13594452,
            "range": "± 57181",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_4_threads/40mm_cube",
            "value": 13579560,
            "range": "± 78085",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/small_box_12tri",
            "value": 71212,
            "range": "± 904",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/small_box_12tri",
            "value": 69310,
            "range": "± 906",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/small_box_12tri",
            "value": 66333,
            "range": "± 392",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/medium_sphere_16seg",
            "value": 1666391,
            "range": "± 6684",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/medium_sphere_16seg",
            "value": 1591797,
            "range": "± 14263",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/medium_sphere_16seg",
            "value": 1528374,
            "range": "± 18201",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/large_sphere_32seg",
            "value": 8036165,
            "range": "± 223704",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/large_sphere_32seg",
            "value": 7476290,
            "range": "± 24667",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/large_sphere_32seg",
            "value": 7073867,
            "range": "± 23674",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/xl_sphere_64seg",
            "value": 40527191,
            "range": "± 148822",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/xl_sphere_64seg",
            "value": 32873789,
            "range": "± 68100",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/xl_sphere_64seg",
            "value": 29386155,
            "range": "± 171697",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/box",
            "value": 194,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/sphere_32seg",
            "value": 20040,
            "range": "± 52",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/cylinder_32seg",
            "value": 2603,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/torus_32x16",
            "value": 20568,
            "range": "± 57",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/16seg",
            "value": 68114,
            "range": "± 485",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/32seg",
            "value": 261124,
            "range": "± 1046",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/64seg",
            "value": 1036124,
            "range": "± 3596",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/box",
            "value": 54510,
            "range": "± 217",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/sphere_32seg",
            "value": 7178235,
            "range": "± 22462",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/16seg",
            "value": 87612,
            "range": "± 266",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/32seg",
            "value": 517554,
            "range": "± 2755",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/64seg",
            "value": 2711345,
            "range": "± 8766",
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
          "id": "379c88b6ab14dd56e8e65ab3c8e21775ab5fdefa",
          "message": "Merge pull request #15 from sscargal/phase-45-plan-05-engine-integration\n\nPhase 45 plan 05 engine integration",
          "timestamp": "2026-03-24T14:27:59-06:00",
          "tree_id": "6a45b0914aff7270c2006b89982682109448b5ab",
          "url": "https://github.com/sscargal/libslic3r-rs/commit/379c88b6ab14dd56e8e65ab3c8e21775ab5fdefa"
        },
        "date": 1774387021374,
        "tool": "cargo",
        "benches": [
          {
            "name": "slice_calibration_cube",
            "value": 7633874,
            "range": "± 755943",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cylinder_64sides",
            "value": 117953092,
            "range": "± 3415069",
            "unit": "ns/iter"
          },
          {
            "name": "slice_dense_sphere_1280tri",
            "value": 3794082,
            "range": "± 16237",
            "unit": "ns/iter"
          },
          {
            "name": "slice_thin_wall_box",
            "value": 15992425,
            "range": "± 343834",
            "unit": "ns/iter"
          },
          {
            "name": "slice_multi_overhang",
            "value": 12205329,
            "range": "± 441959",
            "unit": "ns/iter"
          },
          {
            "name": "slice_cube_full_config",
            "value": 22747517106,
            "range": "± 321811367",
            "unit": "ns/iter"
          },
          {
            "name": "memory_estimate_cube",
            "value": 7804813,
            "range": "± 294643",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_union_overlapping",
            "value": 1469,
            "range": "± 18",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_intersection_overlapping",
            "value": 1089,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "polygon_difference_overlapping",
            "value": 1225,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_outward_2mm",
            "value": 10521,
            "range": "± 32",
            "unit": "ns/iter"
          },
          {
            "name": "offset_star_12pt_inward_2mm",
            "value": 10340,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "offset_rect_collapse",
            "value": 1689,
            "range": "± 7",
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
            "value": 1624866,
            "range": "± 17384",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_ray_intersect_100rays",
            "value": 51284,
            "range": "± 375",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/sequential/40mm_cube",
            "value": 11225033,
            "range": "± 97462",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_auto/40mm_cube",
            "value": 13446516,
            "range": "± 43386",
            "unit": "ns/iter"
          },
          {
            "name": "parallel_vs_sequential/parallel_4_threads/40mm_cube",
            "value": 13517041,
            "range": "± 87526",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/small_box_12tri",
            "value": 71104,
            "range": "± 506",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/small_box_12tri",
            "value": 69378,
            "range": "± 320",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/small_box_12tri",
            "value": 66454,
            "range": "± 302",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/medium_sphere_16seg",
            "value": 1656866,
            "range": "± 14880",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/medium_sphere_16seg",
            "value": 1585453,
            "range": "± 8125",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/medium_sphere_16seg",
            "value": 1531187,
            "range": "± 33387",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/large_sphere_32seg",
            "value": 8037988,
            "range": "± 27713",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/large_sphere_32seg",
            "value": 7456484,
            "range": "± 73072",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/large_sphere_32seg",
            "value": 7058303,
            "range": "± 34348",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/union/xl_sphere_64seg",
            "value": 40459490,
            "range": "± 208415",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/difference/xl_sphere_64seg",
            "value": 32891227,
            "range": "± 125532",
            "unit": "ns/iter"
          },
          {
            "name": "boolean_ops/intersection/xl_sphere_64seg",
            "value": 29576138,
            "range": "± 101961",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/box",
            "value": 194,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/sphere_32seg",
            "value": 20004,
            "range": "± 69",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/cylinder_32seg",
            "value": 2614,
            "range": "± 43",
            "unit": "ns/iter"
          },
          {
            "name": "primitives/torus_32x16",
            "value": 20617,
            "range": "± 62",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/16seg",
            "value": 68388,
            "range": "± 130",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/32seg",
            "value": 261096,
            "range": "± 706",
            "unit": "ns/iter"
          },
          {
            "name": "plane_split/equator_split/64seg",
            "value": 1036901,
            "range": "± 8254",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/box",
            "value": 53884,
            "range": "± 155",
            "unit": "ns/iter"
          },
          {
            "name": "hollow/hollow/sphere_32seg",
            "value": 7157020,
            "range": "± 45993",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/16seg",
            "value": 87185,
            "range": "± 414",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/32seg",
            "value": 518673,
            "range": "± 2621",
            "unit": "ns/iter"
          },
          {
            "name": "bvh_build/build/64seg",
            "value": 2716893,
            "range": "± 5636",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}