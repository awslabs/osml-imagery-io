### Read Performance Comparison

| Dataset | Access Pattern | Native | Zarr Local | Zarr S3 |
| --- | --- | --- | --- | --- |
| Tiny NITF (1MB) | small roi | 6 | 8 | 66 |
| Synth Small NC | single tile | 6 | 6 | 46 |
| Tiny NITF (1MB) | single tile | 6 | 4 | 141 |
| Synth Medium C3 | single tile | 7 | 6 | 61 |
| Synth Small NC | small roi | 8 | 10 | 158 |
| Synth Medium C8 | single tile | 9 | 11 | 65 |
| Synth Medium C3 | small roi | 12 | 11 | 86 |
| Synth Large NC | small roi | 22 | 24 | 287 |
| Synth Large NC | single tile | 28 | 6 | 113 |
| Synth Medium C8 | small roi | 33 | 40 | 100 |
| Umbra SIDD | small roi | 35 | 59 | 3027 |
| Umbra SIDD | single tile | 49 | 50 | 3420 |
| WV Pan J2K (679MB) | single tile | 329 | 53 | 130 |
| WV 8-band J2K (354MB) | single tile | 395 | 277 | 534 |
| WV Pan J2K (679MB) | small roi | 529 | 366 | 478 |
| WV 8-band J2K (354MB) | small roi | 2907 | 2392 | 3095 |
| WV Pan J2K (679MB) | large roi | 4202 | 3884 | 4353 |

All times in milliseconds (ms).

### Tile Read Native

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| native_read | Tiny NITF (1MB) | small roi | 6 | 7 | 6 | 6 | 1 | 10 |
| native_read | Synth Small NC | single tile | 5 | 9 | 6 | 6 | 1 | 10 |
| native_read | Tiny NITF (1MB) | single tile | 6 | 8 | 6 | 6 | 1 | 10 |
| native_read | Synth Medium C3 | single tile | 6 | 11 | 7 | 7 | 1 | 10 |
| native_read | Synth Small NC | small roi | 6 | 8 | 8 | 8 | 1 | 10 |
| native_read | Synth Medium C8 | single tile | 8 | 10 | 9 | 9 | 1 | 10 |
| native_read | Synth Medium C3 | small roi | 11 | 12 | 12 | 12 | 0 | 10 |
| native_read | Synth Large NC | small roi | 21 | 23 | 22 | 22 | 1 | 10 |
| native_read | Synth Large NC | single tile | 19 | 102 | 28 | 20 | 26 | 10 |
| native_read | Synth Medium C8 | small roi | 32 | 34 | 33 | 33 | 1 | 10 |
| native_read | Umbra SIDD | small roi | 33 | 38 | 35 | 34 | 2 | 10 |
| native_read | Umbra SIDD | single tile | 33 | 158 | 49 | 37 | 39 | 10 |
| native_read | WV Pan J2K (679MB) | single tile | 208 | 1248 | 329 | 230 | 323 | 10 |
| native_read | WV 8-band J2K (354MB) | single tile | 385 | 412 | 395 | 393 | 8 | 10 |
| native_read | WV Pan J2K (679MB) | small roi | 519 | 562 | 529 | 523 | 13 | 10 |
| native_read | WV 8-band J2K (354MB) | small roi | 2839 | 3003 | 2907 | 2891 | 46 | 10 |
| native_read | WV Pan J2K (679MB) | large roi | 4157 | 4279 | 4202 | 4192 | 39 | 10 |

All times in milliseconds (ms).

### Index Generation

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| index_generation | Synth Medium C8 | 8 | 11 | 9 | 9 | 1 | 5 |
| index_generation | Tiny NITF (1MB) | 10 | 14 | 11 | 11 | 2 | 5 |
| index_generation | Synth Small NC | 11 | 18 | 13 | 12 | 3 | 5 |
| index_generation | Synth Medium C3 | 15 | 22 | 17 | 17 | 3 | 5 |
| index_generation | Synth Large NC | 23 | 25 | 24 | 24 | 1 | 5 |
| index_generation | Umbra SIDD | 28 | 31 | 29 | 29 | 1 | 5 |
| index_generation | WV 8-band J2K (354MB) | 109 | 542 | 199 | 113 | 192 | 5 |
| index_generation | WV Pan J2K (679MB) | 226 | 327 | 262 | 262 | 40 | 5 |

All times in milliseconds (ms).

### Metadata

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| metadata_read | Tiny NITF (1MB) | 5 | 7 | 6 | 6 | 1 | 10 |
| metadata_read | Synth Small NC | 6 | 8 | 7 | 7 | 1 | 10 |
| metadata_read | Synth Medium C8 | 6 | 8 | 7 | 7 | 0 | 10 |
| metadata_read | Synth Medium C3 | 7 | 8 | 7 | 8 | 0 | 10 |
| metadata_read | Synth Large NC | 13 | 16 | 15 | 15 | 1 | 10 |
| metadata_read | Umbra SIDD | 17 | 20 | 18 | 18 | 1 | 10 |
| metadata_read | WV 8-band J2K (354MB) | 70 | 80 | 74 | 73 | 3 | 10 |
| metadata_read | WV Pan J2K (679MB) | 118 | 124 | 121 | 121 | 2 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr Local

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Tiny NITF (1MB) | single tile | 3 | 5 | 4 | 4 | 1 | 10 |
| zarr_read | Synth Medium C3 | single tile | 4 | 9 | 6 | 5 | 1 | 10 |
| zarr_read | Synth Large NC | single tile | 5 | 9 | 6 | 5 | 1 | 10 |
| zarr_read | Synth Small NC | single tile | 3 | 17 | 6 | 5 | 4 | 10 |
| zarr_read | Tiny NITF (1MB) | small roi | 5 | 19 | 8 | 6 | 5 | 10 |
| zarr_read | Synth Small NC | small roi | 8 | 14 | 10 | 9 | 2 | 10 |
| zarr_read | Synth Medium C3 | small roi | 7 | 16 | 11 | 10 | 3 | 10 |
| zarr_read | Synth Medium C8 | single tile | 10 | 12 | 11 | 11 | 1 | 10 |
| zarr_read | Synth Large NC | small roi | 13 | 89 | 24 | 17 | 23 | 10 |
| zarr_read | Synth Medium C8 | small roi | 36 | 48 | 40 | 37 | 4 | 10 |
| zarr_read | Umbra SIDD | single tile | 42 | 88 | 50 | 45 | 14 | 10 |
| zarr_read | WV Pan J2K (679MB) | single tile | 48 | 81 | 53 | 50 | 10 | 10 |
| zarr_read | Umbra SIDD | small roi | 51 | 70 | 59 | 58 | 7 | 10 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 263 | 292 | 277 | 277 | 9 | 10 |
| zarr_read | WV Pan J2K (679MB) | small roi | 354 | 431 | 366 | 359 | 23 | 10 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 2374 | 2443 | 2392 | 2385 | 21 | 10 |
| zarr_read | WV Pan J2K (679MB) | large roi | 3835 | 4047 | 3884 | 3868 | 63 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr S3

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Synth Small NC | single tile | 37 | 56 | 46 | 45 | 6 | 10 |
| zarr_read | Synth Medium C3 | single tile | 38 | 150 | 61 | 52 | 33 | 10 |
| zarr_read | Synth Medium C8 | single tile | 44 | 229 | 65 | 47 | 58 | 10 |
| zarr_read | Tiny NITF (1MB) | small roi | 61 | 72 | 66 | 67 | 3 | 10 |
| zarr_read | Synth Medium C3 | small roi | 62 | 126 | 86 | 85 | 18 | 10 |
| zarr_read | Synth Medium C8 | small roi | 67 | 156 | 100 | 96 | 25 | 10 |
| zarr_read | Synth Large NC | single tile | 65 | 296 | 113 | 98 | 66 | 10 |
| zarr_read | WV Pan J2K (679MB) | single tile | 101 | 270 | 130 | 110 | 51 | 10 |
| zarr_read | Tiny NITF (1MB) | single tile | 60 | 790 | 141 | 71 | 228 | 10 |
| zarr_read | Synth Small NC | small roi | 66 | 296 | 158 | 140 | 71 | 10 |
| zarr_read | Synth Large NC | small roi | 224 | 395 | 287 | 268 | 55 | 10 |
| zarr_read | WV Pan J2K (679MB) | small roi | 424 | 636 | 478 | 453 | 62 | 10 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 412 | 705 | 534 | 525 | 71 | 10 |
| zarr_read | Umbra SIDD | small roi | 2649 | 3375 | 3027 | 3001 | 256 | 10 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 3023 | 3217 | 3095 | 3091 | 57 | 10 |
| zarr_read | Umbra SIDD | single tile | 2787 | 4042 | 3420 | 3551 | 466 | 10 |
| zarr_read | WV Pan J2K (679MB) | large roi | 4215 | 4848 | 4353 | 4299 | 192 | 10 |

All times in milliseconds (ms).
