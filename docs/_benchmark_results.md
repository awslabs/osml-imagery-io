### Read Performance Comparison

| Dataset | Access Pattern | Native | Zarr Local | Zarr S3 |
| --- | --- | --- | --- | --- |
| Synth COG Pyramid | single tile | 1 | 4 | 180 |
| Synth Small TIFF | single tile | 2 | 4 | 253 |
| Synth Small TIFF | small roi | 3 | 6 | 216 |
| Synth COG Pyramid | small roi | 3 | 7 | 184 |
| Tiny NITF (1MB) | single tile | 3 | 5 | 411 |
| Synth NITF R-set Pyramid | small roi | 4 | 8 | 199 |
| Synth Small NC | small roi | 4 | 7 | 508 |
| Synth Small NC | single tile | 5 | 4 | 196 |
| Synth Medium C3 | single tile | 5 | 6 | 246 |
| Synth NITF R-set Pyramid | single tile | 5 | 21 | 207 |
| Tiny NITF (1MB) | small roi | 6 | 6 | 219 |
| Synth Medium C3 | small roi | 6 | 10 | 213 |
| Synth Medium C8 | single tile | 8 | 9 | 182 |
| Synth Large NC | small roi | 19 | 30 | 568 |
| Synth Medium C8 | small roi | 21 | 29 | 244 |
| Synth Large NC | single tile | 23 | 5 | 257 |
| Umbra SIDD | small roi | 32 | 45 | 4912 |
| Umbra SIDD | single tile | 44 | 47 | 5062 |
| WV 8-band J2K (354MB) | single tile | 186 | 50 | 683 |
| WV Pan J2K (679MB) | single tile | 198 | 33 | 284 |
| WV Pan J2K (679MB) | small roi | 295 | 125 | 677 |
| WV 8-band J2K (354MB) | small roi | 918 | 417 | 2144 |
| WV Pan J2K (679MB) | large roi | 1452 | 1147 | 3178 |

All times in milliseconds (ms).

### Tile Read Native

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| native_read | Synth COG Pyramid | single tile | 1 | 2 | 1 | 1 | 0 | 10 |
| native_read | Synth Small TIFF | single tile | 1 | 5 | 2 | 1 | 1 | 10 |
| native_read | Synth Small TIFF | small roi | 3 | 4 | 3 | 3 | 0 | 10 |
| native_read | Synth COG Pyramid | small roi | 3 | 4 | 3 | 3 | 0 | 10 |
| native_read | Tiny NITF (1MB) | single tile | 3 | 4 | 3 | 3 | 0 | 10 |
| native_read | Synth NITF R-set Pyramid | small roi | 4 | 5 | 4 | 4 | 0 | 10 |
| native_read | Synth Small NC | small roi | 4 | 5 | 4 | 4 | 0 | 10 |
| native_read | Synth Small NC | single tile | 4 | 7 | 5 | 4 | 1 | 10 |
| native_read | Synth Medium C3 | single tile | 4 | 7 | 5 | 4 | 1 | 10 |
| native_read | Synth NITF R-set Pyramid | single tile | 4 | 11 | 5 | 4 | 2 | 10 |
| native_read | Tiny NITF (1MB) | small roi | 4 | 19 | 6 | 4 | 5 | 10 |
| native_read | Synth Medium C3 | small roi | 6 | 7 | 6 | 6 | 0 | 10 |
| native_read | Synth Medium C8 | single tile | 6 | 20 | 8 | 7 | 4 | 10 |
| native_read | Synth Large NC | small roi | 16 | 21 | 19 | 19 | 2 | 10 |
| native_read | Synth Medium C8 | small roi | 19 | 26 | 21 | 20 | 2 | 10 |
| native_read | Synth Large NC | single tile | 15 | 88 | 23 | 16 | 23 | 10 |
| native_read | Umbra SIDD | small roi | 31 | 35 | 32 | 32 | 1 | 10 |
| native_read | Umbra SIDD | single tile | 30 | 150 | 44 | 33 | 37 | 10 |
| native_read | WV 8-band J2K (354MB) | single tile | 172 | 229 | 186 | 181 | 16 | 10 |
| native_read | WV Pan J2K (679MB) | single tile | 190 | 209 | 198 | 198 | 6 | 10 |
| native_read | WV Pan J2K (679MB) | small roi | 280 | 306 | 295 | 295 | 7 | 10 |
| native_read | WV 8-band J2K (354MB) | small roi | 902 | 946 | 918 | 914 | 15 | 10 |
| native_read | WV Pan J2K (679MB) | large roi | 1407 | 1512 | 1452 | 1445 | 37 | 10 |

All times in milliseconds (ms).

### Dted Parse

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| dted_open_and_parse | test_bench_dted_open_and_parse | 1 | 1 | 1 | 1 | 0 | 20 |

All times in milliseconds (ms).

### Dted Full Read

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| dted_full_read | test_bench_dted_full_read | 3 | 4 | 3 | 3 | 0 | 10 |

All times in milliseconds (ms).

### Index Generation

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| index_generation | Synth Small TIFF | 3 | 4 | 3 | 3 | 1 | 5 |
| index_generation | Synth COG Pyramid | 4 | 6 | 5 | 5 | 1 | 5 |
| index_generation | Synth Small NC | 5 | 7 | 6 | 6 | 1 | 5 |
| index_generation | Synth Medium C8 | 5 | 7 | 7 | 7 | 1 | 5 |
| index_generation | Synth Medium C3 | 7 | 8 | 7 | 8 | 1 | 5 |
| index_generation | Synth NITF R-set Pyramid | 7 | 8 | 8 | 8 | 1 | 5 |
| index_generation | Tiny NITF (1MB) | 6 | 14 | 10 | 10 | 4 | 5 |
| index_generation | Synth Large NC | 19 | 27 | 22 | 21 | 3 | 5 |
| index_generation | Umbra SIDD | 23 | 27 | 25 | 24 | 2 | 5 |
| index_generation | WV 8-band J2K (354MB) | 92 | 106 | 97 | 95 | 6 | 5 |
| index_generation | WV Pan J2K (679MB) | 228 | 246 | 236 | 237 | 7 | 5 |

All times in milliseconds (ms).

### Metadata

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| metadata_read | Synth Small TIFF | 1 | 1 | 1 | 1 | 0 | 10 |
| metadata_read | Synth COG Pyramid | 1 | 2 | 1 | 1 | 0 | 10 |
| metadata_read | Synth Medium C3 | 3 | 5 | 4 | 4 | 1 | 10 |
| metadata_read | Tiny NITF (1MB) | 3 | 4 | 4 | 4 | 0 | 10 |
| metadata_read | Synth NITF R-set Pyramid | 3 | 5 | 4 | 4 | 1 | 10 |
| metadata_read | Synth Small NC | 3 | 6 | 4 | 4 | 1 | 10 |
| metadata_read | Synth Medium C8 | 3 | 6 | 4 | 4 | 1 | 10 |
| metadata_read | Synth Large NC | 10 | 14 | 12 | 11 | 1 | 10 |
| metadata_read | Umbra SIDD | 13 | 28 | 16 | 15 | 4 | 10 |
| metadata_read | WV 8-band J2K (354MB) | 56 | 80 | 63 | 61 | 7 | 10 |
| metadata_read | WV Pan J2K (679MB) | 114 | 122 | 116 | 116 | 2 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr Local

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Synth Small TIFF | single tile | 3 | 5 | 4 | 4 | 1 | 10 |
| zarr_read | Synth COG Pyramid | single tile | 3 | 6 | 4 | 4 | 1 | 10 |
| zarr_read | Synth Small NC | single tile | 3 | 6 | 4 | 4 | 1 | 10 |
| zarr_read | Tiny NITF (1MB) | single tile | 4 | 6 | 5 | 4 | 1 | 10 |
| zarr_read | Synth Large NC | single tile | 3 | 9 | 5 | 5 | 2 | 10 |
| zarr_read | Tiny NITF (1MB) | small roi | 4 | 8 | 6 | 6 | 1 | 10 |
| zarr_read | Synth Small TIFF | small roi | 4 | 8 | 6 | 7 | 1 | 10 |
| zarr_read | Synth Medium C3 | single tile | 5 | 14 | 6 | 5 | 3 | 10 |
| zarr_read | Synth COG Pyramid | small roi | 6 | 7 | 7 | 6 | 0 | 10 |
| zarr_read | Synth Small NC | small roi | 5 | 10 | 7 | 7 | 1 | 10 |
| zarr_read | Synth NITF R-set Pyramid | small roi | 7 | 11 | 8 | 7 | 1 | 10 |
| zarr_read | Synth Medium C8 | single tile | 6 | 11 | 9 | 9 | 1 | 10 |
| zarr_read | Synth Medium C3 | small roi | 8 | 12 | 10 | 10 | 1 | 10 |
| zarr_read | Synth NITF R-set Pyramid | single tile | 3 | 178 | 21 | 4 | 55 | 10 |
| zarr_read | Synth Medium C8 | small roi | 26 | 33 | 29 | 28 | 2 | 10 |
| zarr_read | Synth Large NC | small roi | 11 | 180 | 30 | 13 | 53 | 10 |
| zarr_read | WV Pan J2K (679MB) | single tile | 21 | 119 | 33 | 23 | 30 | 10 |
| zarr_read | Umbra SIDD | small roi | 43 | 49 | 45 | 45 | 2 | 10 |
| zarr_read | Umbra SIDD | single tile | 43 | 55 | 47 | 46 | 3 | 10 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 48 | 57 | 50 | 49 | 2 | 10 |
| zarr_read | WV Pan J2K (679MB) | small roi | 109 | 219 | 125 | 112 | 34 | 10 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 406 | 453 | 417 | 416 | 14 | 10 |
| zarr_read | WV Pan J2K (679MB) | large roi | 1088 | 1191 | 1147 | 1149 | 32 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr S3

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Synth COG Pyramid | single tile | 173 | 186 | 180 | 181 | 6 | 3 |
| zarr_read | Synth Medium C8 | single tile | 154 | 225 | 182 | 167 | 38 | 3 |
| zarr_read | Synth COG Pyramid | small roi | 170 | 200 | 184 | 184 | 15 | 3 |
| zarr_read | Synth Small NC | single tile | 160 | 242 | 196 | 187 | 42 | 3 |
| zarr_read | Synth NITF R-set Pyramid | small roi | 182 | 212 | 199 | 202 | 15 | 3 |
| zarr_read | Synth NITF R-set Pyramid | single tile | 173 | 265 | 207 | 183 | 51 | 3 |
| zarr_read | Synth Medium C3 | small roi | 195 | 231 | 213 | 212 | 18 | 3 |
| zarr_read | Synth Small TIFF | small roi | 202 | 231 | 216 | 216 | 15 | 3 |
| zarr_read | Tiny NITF (1MB) | small roi | 207 | 240 | 219 | 211 | 18 | 3 |
| zarr_read | Synth Medium C8 | small roi | 180 | 348 | 244 | 206 | 91 | 3 |
| zarr_read | Synth Medium C3 | single tile | 179 | 336 | 246 | 225 | 81 | 3 |
| zarr_read | Synth Small TIFF | single tile | 168 | 315 | 253 | 276 | 77 | 3 |
| zarr_read | Synth Large NC | single tile | 238 | 289 | 257 | 244 | 28 | 3 |
| zarr_read | WV Pan J2K (679MB) | single tile | 218 | 336 | 284 | 299 | 61 | 3 |
| zarr_read | Tiny NITF (1MB) | single tile | 222 | 772 | 411 | 239 | 313 | 3 |
| zarr_read | Synth Small NC | small roi | 239 | 858 | 508 | 428 | 317 | 3 |
| zarr_read | Synth Large NC | small roi | 498 | 663 | 568 | 543 | 85 | 3 |
| zarr_read | WV Pan J2K (679MB) | small roi | 433 | 850 | 677 | 748 | 217 | 3 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 524 | 840 | 683 | 686 | 158 | 3 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 1991 | 2267 | 2144 | 2175 | 140 | 3 |
| zarr_read | WV Pan J2K (679MB) | large roi | 2965 | 3517 | 3178 | 3053 | 297 | 3 |
| zarr_read | Umbra SIDD | small roi | 4808 | 5095 | 4912 | 4833 | 159 | 3 |
| zarr_read | Umbra SIDD | single tile | 4084 | 6116 | 5062 | 4986 | 1018 | 3 |

All times in milliseconds (ms).
