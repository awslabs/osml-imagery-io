### Read Performance Comparison

| Dataset | Access Pattern | IO (local) | IO (virtual) | IO (s3) | Zarr Local | Zarr S3 |
| --- | --- | --- | --- | --- | --- | --- |
| Synth COG Pyramid | single tile | 1 | 1 | 245 | 4 | 169 |
| Synth Small TIFF | single tile | 1 | 0 | 499 | 21 | 192 |
| Synth COG Pyramid | small roi | 3 | 2 | 409 | 5 | 183 |
| Synth Small TIFF | small roi | 3 | 2 | 886 | 6 | 224 |
| Tiny NITF (1MB) | single tile | 4 | 4 | 508 | 5 | 274 |
| Synth Small NC | single tile | 5 | 5 | 303 | 4 | 192 |
| Synth NITF R-set Pyramid | single tile | 6 | 5 | 310 | 3 | 254 |
| Tiny NITF (1MB) | small roi | 6 | 4 | 336 | 3 | 252 |
| Synth NITF R-set Pyramid | small roi | 8 | 7 | 478 | 6 | 241 |
| Synth Medium C8 | single tile | 8 | 6 | 375 | 5 | 175 |
| Synth Large NC | single tile | 8 | 5 | 620 | 4 | 273 |
| Synth Medium C3 | single tile | 9 | 7 | 286 | 4 | 183 |
| Synth Small NC | small roi | 9 | 8 | 473 | 6 | 234 |
| Synth Large NC | small roi | 11 | 9 | 705 | 8 | 527 |
| Synth Medium C3 | small roi | 12 | 10 | 357 | 21 | 204 |
| Synth Medium C8 | small roi | 17 | 20 | 223 | 19 | 197 |
| WV Pan J2K (679MB) | single tile | 29 | 27 | 575 | 34 | 247 |
| Umbra SIDD | small roi | 33 | 32 | 3783 | 47 | 3181 |
| Umbra SIDD | single tile | 36 | 33 | 3578 | 50 | 3185 |
| WV Pan J2K (679MB) | small roi | 90 | 114 | 864 | 98 | 518 |
| WV 8-band J2K (354MB) | single tile | 118 | 113 | 804 | 48 | 765 |
| WV 8-band J2K (354MB) | small roi | 820 | 921 | 2800 | 400 | 1882 |
| WV Pan J2K (679MB) | large roi | 871 | 1176 | 4631 | 865 | 2742 |

All times in milliseconds (ms).

### Tile Read IO

| Operation | Dataset | Access Pattern | Source | Fetch % | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| io_read | Synth COG Pyramid | single tile | local |  | 1 | 1 | 1 | 1 | 0 | 10 |
| io_read | Synth COG Pyramid | single tile | s3 | 100.0 | 236 | 256 | 245 | 243 | 10 | 3 |
| io_read | Synth COG Pyramid | single tile | virtual | 100.0 | 0 | 1 | 1 | 1 | 0 | 10 |
| io_read | Synth COG Pyramid | small roi | local |  | 2 | 3 | 3 | 3 | 0 | 10 |
| io_read | Synth COG Pyramid | small roi | s3 | 100.0 | 229 | 744 | 409 | 255 | 290 | 3 |
| io_read | Synth COG Pyramid | small roi | virtual | 100.0 | 2 | 3 | 2 | 2 | 0 | 10 |
| io_read | Synth Large NC | single tile | local |  | 5 | 18 | 8 | 7 | 4 | 10 |
| io_read | Synth Large NC | single tile | s3 | 1.7 | 334 | 1138 | 620 | 390 | 449 | 3 |
| io_read | Synth Large NC | single tile | virtual | 1.7 | 4 | 6 | 5 | 4 | 1 | 10 |
| io_read | Synth Large NC | small roi | local |  | 8 | 15 | 11 | 10 | 2 | 10 |
| io_read | Synth Large NC | small roi | s3 | 14.2 | 642 | 770 | 705 | 702 | 64 | 3 |
| io_read | Synth Large NC | small roi | virtual | 14.2 | 8 | 10 | 9 | 9 | 1 | 10 |
| io_read | Synth Medium C3 | single tile | local |  | 7 | 15 | 9 | 8 | 2 | 10 |
| io_read | Synth Medium C3 | single tile | s3 | 127.8 | 272 | 293 | 286 | 293 | 12 | 3 |
| io_read | Synth Medium C3 | single tile | virtual | 127.8 | 6 | 8 | 7 | 7 | 1 | 10 |
| io_read | Synth Medium C3 | small roi | local |  | 9 | 17 | 12 | 11 | 3 | 10 |
| io_read | Synth Medium C3 | small roi | s3 | 127.8 | 354 | 360 | 357 | 358 | 3 | 3 |
| io_read | Synth Medium C3 | small roi | virtual | 127.8 | 10 | 11 | 10 | 10 | 0 | 10 |
| io_read | Synth Medium C8 | single tile | local |  | 6 | 12 | 8 | 7 | 2 | 10 |
| io_read | Synth Medium C8 | single tile | s3 | 100.0 | 223 | 668 | 375 | 235 | 254 | 3 |
| io_read | Synth Medium C8 | single tile | virtual | 100.0 | 5 | 7 | 6 | 6 | 1 | 10 |
| io_read | Synth Medium C8 | small roi | local |  | 15 | 19 | 17 | 17 | 1 | 10 |
| io_read | Synth Medium C8 | small roi | s3 | 100.0 | 210 | 231 | 223 | 228 | 11 | 3 |
| io_read | Synth Medium C8 | small roi | virtual | 100.0 | 19 | 22 | 20 | 20 | 1 | 10 |
| io_read | Synth NITF R-set Pyramid | single tile | local |  | 5 | 8 | 6 | 5 | 1 | 10 |
| io_read | Synth NITF R-set Pyramid | single tile | s3 | 3.1 | 291 | 331 | 310 | 309 | 20 | 3 |
| io_read | Synth NITF R-set Pyramid | single tile | virtual | 3.1 | 4 | 5 | 5 | 4 | 0 | 10 |
| io_read | Synth NITF R-set Pyramid | small roi | local |  | 6 | 13 | 8 | 7 | 2 | 10 |
| io_read | Synth NITF R-set Pyramid | small roi | s3 | 15.6 | 473 | 485 | 478 | 475 | 7 | 3 |
| io_read | Synth NITF R-set Pyramid | small roi | virtual | 15.6 | 7 | 7 | 7 | 7 | 0 | 10 |
| io_read | Synth Small NC | single tile | local |  | 4 | 9 | 5 | 5 | 1 | 10 |
| io_read | Synth Small NC | single tile | s3 | 12.5 | 291 | 310 | 303 | 309 | 10 | 3 |
| io_read | Synth Small NC | single tile | virtual | 12.5 | 4 | 5 | 5 | 5 | 0 | 10 |
| io_read | Synth Small NC | small roi | local |  | 8 | 15 | 9 | 9 | 2 | 10 |
| io_read | Synth Small NC | small roi | s3 | 62.4 | 454 | 482 | 473 | 482 | 16 | 3 |
| io_read | Synth Small NC | small roi | virtual | 62.4 | 7 | 9 | 8 | 8 | 1 | 10 |
| io_read | Synth Small TIFF | single tile | local |  | 1 | 3 | 1 | 1 | 1 | 10 |
| io_read | Synth Small TIFF | single tile | s3 | 12.5 | 468 | 535 | 499 | 493 | 34 | 3 |
| io_read | Synth Small TIFF | single tile | virtual | 12.5 | 0 | 0 | 0 | 0 | 0 | 10 |
| io_read | Synth Small TIFF | small roi | local |  | 2 | 8 | 3 | 3 | 2 | 10 |
| io_read | Synth Small TIFF | small roi | s3 | 62.5 | 862 | 933 | 886 | 862 | 41 | 3 |
| io_read | Synth Small TIFF | small roi | virtual | 62.5 | 2 | 3 | 2 | 2 | 0 | 10 |
| io_read | Tiny NITF (1MB) | single tile | local |  | 3 | 6 | 4 | 4 | 1 | 10 |
| io_read | Tiny NITF (1MB) | single tile | s3 | 106.2 | 341 | 826 | 508 | 357 | 276 | 3 |
| io_read | Tiny NITF (1MB) | single tile | virtual | 106.2 | 4 | 4 | 4 | 4 | 0 | 10 |
| io_read | Tiny NITF (1MB) | small roi | local |  | 4 | 11 | 6 | 5 | 2 | 10 |
| io_read | Tiny NITF (1MB) | small roi | s3 | 106.2 | 323 | 348 | 336 | 338 | 12 | 3 |
| io_read | Tiny NITF (1MB) | small roi | virtual | 106.2 | 4 | 5 | 4 | 4 | 0 | 10 |
| io_read | Umbra SIDD | single tile | local |  | 19 | 171 | 36 | 21 | 48 | 10 |
| io_read | Umbra SIDD | single tile | s3 | 100.1 | 3564 | 3605 | 3578 | 3565 | 24 | 3 |
| io_read | Umbra SIDD | single tile | virtual | 100.1 | 29 | 41 | 33 | 32 | 4 | 10 |
| io_read | Umbra SIDD | small roi | local |  | 19 | 146 | 33 | 21 | 40 | 10 |
| io_read | Umbra SIDD | small roi | s3 | 100.1 | 3528 | 3942 | 3783 | 3879 | 223 | 3 |
| io_read | Umbra SIDD | small roi | virtual | 100.1 | 28 | 35 | 32 | 32 | 3 | 10 |
| io_read | WV 8-band J2K (354MB) | single tile | local |  | 106 | 140 | 118 | 112 | 11 | 10 |
| io_read | WV 8-band J2K (354MB) | single tile | s3 | 1.6 | 681 | 1029 | 804 | 702 | 195 | 3 |
| io_read | WV 8-band J2K (354MB) | single tile | virtual | 1.6 | 106 | 129 | 113 | 112 | 6 | 10 |
| io_read | WV 8-band J2K (354MB) | small roi | local |  | 784 | 884 | 820 | 816 | 25 | 10 |
| io_read | WV 8-band J2K (354MB) | small roi | s3 | 13.9 | 2675 | 3009 | 2800 | 2715 | 182 | 3 |
| io_read | WV 8-band J2K (354MB) | small roi | virtual | 13.9 | 875 | 973 | 921 | 931 | 38 | 10 |
| io_read | WV Pan J2K (679MB) | large roi | local |  | 815 | 943 | 871 | 854 | 47 | 10 |
| io_read | WV Pan J2K (679MB) | large roi | s3 | 6.1 | 4052 | 5581 | 4631 | 4261 | 829 | 3 |
| io_read | WV Pan J2K (679MB) | large roi | virtual | 6.1 | 1110 | 1251 | 1176 | 1180 | 51 | 10 |
| io_read | WV Pan J2K (679MB) | single tile | local |  | 26 | 36 | 29 | 28 | 3 | 10 |
| io_read | WV Pan J2K (679MB) | single tile | s3 | 0.1 | 499 | 686 | 575 | 541 | 98 | 3 |
| io_read | WV Pan J2K (679MB) | single tile | virtual | 0.1 | 26 | 29 | 27 | 27 | 1 | 10 |
| io_read | WV Pan J2K (679MB) | small roi | local |  | 86 | 97 | 90 | 90 | 3 | 10 |
| io_read | WV Pan J2K (679MB) | small roi | s3 | 0.5 | 788 | 963 | 864 | 840 | 90 | 3 |
| io_read | WV Pan J2K (679MB) | small roi | virtual | 0.5 | 107 | 122 | 114 | 113 | 5 | 10 |

All times in milliseconds (ms).

### Dted Parse

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| dted_open_and_parse | test_bench_dted_open_and_parse | 0 | 1 | 1 | 1 | 0 | 20 |

All times in milliseconds (ms).

### Dted Full Read

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| dted_full_read | test_bench_dted_full_read | 3 | 4 | 4 | 4 | 0 | 10 |

All times in milliseconds (ms).

### Index Generation

| Operation | Dataset | Source | Fetch % | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| index_generation | Synth COG Pyramid | local |  | 5 | 7 | 5 | 5 | 1 | 5 |
| index_generation | Synth COG Pyramid | s3 | 100.0 | 254 | 267 | 258 | 255 | 7 | 3 |
| index_generation | Synth COG Pyramid | virtual | 100.0 | 4 | 9 | 5 | 4 | 2 | 5 |
| index_generation | Synth Large NC | local |  | 11 | 20 | 15 | 14 | 3 | 5 |
| index_generation | Synth Large NC | s3 | 0.1 | 261 | 316 | 286 | 280 | 28 | 3 |
| index_generation | Synth Large NC | virtual | 0.1 | 11 | 13 | 12 | 12 | 1 | 5 |
| index_generation | Synth Medium C3 | local |  | 13 | 18 | 15 | 14 | 2 | 5 |
| index_generation | Synth Medium C3 | s3 | 127.8 | 326 | 450 | 377 | 353 | 65 | 3 |
| index_generation | Synth Medium C3 | virtual | 127.8 | 13 | 15 | 14 | 14 | 1 | 5 |
| index_generation | Synth Medium C8 | local |  | 10 | 18 | 13 | 11 | 4 | 5 |
| index_generation | Synth Medium C8 | s3 | 100.0 | 235 | 266 | 249 | 246 | 16 | 3 |
| index_generation | Synth Medium C8 | virtual | 100.0 | 9 | 10 | 10 | 10 | 0 | 5 |
| index_generation | Synth NITF R-set Pyramid | local |  | 19 | 26 | 22 | 22 | 3 | 5 |
| index_generation | Synth NITF R-set Pyramid | s3 | 4.7 | 357 | 878 | 623 | 634 | 261 | 3 |
| index_generation | Synth NITF R-set Pyramid | virtual | 4.7 | 20 | 26 | 24 | 24 | 2 | 5 |
| index_generation | Synth Small NC | local |  | 9 | 12 | 10 | 9 | 1 | 5 |
| index_generation | Synth Small NC | s3 | 6.2 | 267 | 278 | 274 | 276 | 6 | 3 |
| index_generation | Synth Small NC | virtual | 6.2 | 9 | 10 | 10 | 10 | 1 | 5 |
| index_generation | Synth Small TIFF | local |  | 3 | 4 | 4 | 3 | 1 | 5 |
| index_generation | Synth Small TIFF | s3 | 6.3 | 456 | 734 | 557 | 482 | 154 | 3 |
| index_generation | Synth Small TIFF | virtual | 6.3 | 3 | 4 | 4 | 3 | 0 | 5 |
| index_generation | Tiny NITF (1MB) | local |  | 10 | 31 | 15 | 10 | 9 | 5 |
| index_generation | Tiny NITF (1MB) | s3 | 6.2 | 269 | 284 | 274 | 270 | 9 | 3 |
| index_generation | Tiny NITF (1MB) | virtual | 6.2 | 9 | 11 | 10 | 10 | 1 | 5 |
| index_generation | Umbra SIDD | local |  | 9 | 12 | 10 | 11 | 1 | 5 |
| index_generation | Umbra SIDD | s3 | 0.1 | 271 | 306 | 284 | 277 | 19 | 3 |
| index_generation | Umbra SIDD | virtual | 0.1 | 9 | 11 | 11 | 11 | 1 | 5 |
| index_generation | WV 8-band J2K (354MB) | local |  | 38 | 44 | 40 | 39 | 3 | 5 |
| index_generation | WV 8-band J2K (354MB) | s3 | 0.0 | 413 | 500 | 453 | 445 | 44 | 3 |
| index_generation | WV 8-band J2K (354MB) | virtual | 0.0 | 34 | 534 | 136 | 39 | 222 | 5 |
| index_generation | WV Pan J2K (679MB) | local |  | 66 | 361 | 126 | 69 | 131 | 5 |
| index_generation | WV Pan J2K (679MB) | s3 | 0.0 | 535 | 548 | 541 | 540 | 7 | 3 |
| index_generation | WV Pan J2K (679MB) | virtual | 0.0 | 56 | 115 | 71 | 61 | 25 | 5 |

All times in milliseconds (ms).

### Metadata

| Operation | Dataset | Source | Fetch % | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| metadata_read | Synth COG Pyramid | local |  | 0 | 1 | 0 | 0 | 0 | 10 |
| metadata_read | Synth COG Pyramid | s3 | 100.0 | 205 | 229 | 220 | 224 | 13 | 3 |
| metadata_read | Synth COG Pyramid | virtual | 100.0 | 0 | 1 | 0 | 0 | 0 | 10 |
| metadata_read | Synth Large NC | local |  | 5 | 10 | 6 | 6 | 2 | 10 |
| metadata_read | Synth Large NC | s3 | 0.1 | 239 | 245 | 241 | 241 | 3 | 3 |
| metadata_read | Synth Large NC | virtual | 0.1 | 3 | 4 | 4 | 4 | 0 | 10 |
| metadata_read | Synth Medium C3 | local |  | 4 | 7 | 5 | 5 | 1 | 10 |
| metadata_read | Synth Medium C3 | s3 | 27.8 | 237 | 268 | 253 | 253 | 15 | 3 |
| metadata_read | Synth Medium C3 | virtual | 27.8 | 3 | 4 | 4 | 4 | 0 | 10 |
| metadata_read | Synth Medium C8 | local |  | 3 | 7 | 4 | 4 | 1 | 10 |
| metadata_read | Synth Medium C8 | s3 | 100.0 | 195 | 231 | 217 | 225 | 19 | 3 |
| metadata_read | Synth Medium C8 | virtual | 100.0 | 3 | 4 | 3 | 3 | 0 | 10 |
| metadata_read | Synth NITF R-set Pyramid | local |  | 4 | 6 | 5 | 5 | 1 | 10 |
| metadata_read | Synth NITF R-set Pyramid | s3 | 1.6 | 232 | 824 | 435 | 250 | 337 | 3 |
| metadata_read | Synth NITF R-set Pyramid | virtual | 1.6 | 4 | 5 | 4 | 4 | 0 | 10 |
| metadata_read | Synth Small NC | local |  | 3 | 5 | 3 | 3 | 1 | 10 |
| metadata_read | Synth Small NC | s3 | 6.2 | 237 | 271 | 249 | 240 | 19 | 3 |
| metadata_read | Synth Small NC | virtual | 6.2 | 3 | 3 | 3 | 3 | 0 | 10 |
| metadata_read | Synth Small TIFF | local |  | 1 | 1 | 1 | 1 | 0 | 10 |
| metadata_read | Synth Small TIFF | s3 | 6.3 | 429 | 483 | 451 | 440 | 29 | 3 |
| metadata_read | Synth Small TIFF | virtual | 6.3 | 0 | 1 | 0 | 0 | 0 | 10 |
| metadata_read | Tiny NITF (1MB) | local |  | 4 | 10 | 5 | 5 | 2 | 10 |
| metadata_read | Tiny NITF (1MB) | s3 | 6.2 | 235 | 360 | 278 | 239 | 71 | 3 |
| metadata_read | Tiny NITF (1MB) | virtual | 6.2 | 4 | 5 | 4 | 4 | 0 | 10 |
| metadata_read | Umbra SIDD | local |  | 4 | 9 | 5 | 5 | 2 | 10 |
| metadata_read | Umbra SIDD | s3 | 0.1 | 237 | 683 | 396 | 269 | 249 | 3 |
| metadata_read | Umbra SIDD | virtual | 0.1 | 4 | 5 | 4 | 4 | 0 | 10 |
| metadata_read | WV 8-band J2K (354MB) | local |  | 18 | 24 | 21 | 21 | 2 | 10 |
| metadata_read | WV 8-band J2K (354MB) | s3 | 0.0 | 268 | 302 | 287 | 290 | 17 | 3 |
| metadata_read | WV 8-band J2K (354MB) | virtual | 0.0 | 14 | 19 | 16 | 15 | 2 | 10 |
| metadata_read | WV Pan J2K (679MB) | local |  | 14 | 23 | 19 | 19 | 3 | 10 |
| metadata_read | WV Pan J2K (679MB) | s3 | 0.0 | 258 | 280 | 271 | 274 | 11 | 3 |
| metadata_read | WV Pan J2K (679MB) | virtual | 0.0 | 11 | 16 | 14 | 14 | 2 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr Local

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Synth COG Pyramid | single tile | 3 | 10 | 4 | 4 | 2 | 10 |
| zarr_read | Synth COG Pyramid | small roi | 4 | 9 | 5 | 4 | 1 | 10 |
| zarr_read | Synth Large NC | single tile | 3 | 5 | 4 | 3 | 1 | 10 |
| zarr_read | Synth Large NC | small roi | 7 | 10 | 8 | 8 | 1 | 10 |
| zarr_read | Synth Medium C3 | single tile | 3 | 5 | 4 | 4 | 1 | 10 |
| zarr_read | Synth Medium C3 | small roi | 5 | 160 | 21 | 6 | 49 | 10 |
| zarr_read | Synth Medium C8 | single tile | 4 | 6 | 5 | 5 | 0 | 10 |
| zarr_read | Synth Medium C8 | small roi | 17 | 22 | 19 | 18 | 1 | 10 |
| zarr_read | Synth NITF R-set Pyramid | single tile | 3 | 4 | 3 | 3 | 1 | 10 |
| zarr_read | Synth NITF R-set Pyramid | small roi | 5 | 8 | 6 | 6 | 1 | 10 |
| zarr_read | Synth Small NC | single tile | 3 | 6 | 4 | 4 | 1 | 10 |
| zarr_read | Synth Small NC | small roi | 5 | 7 | 6 | 6 | 1 | 10 |
| zarr_read | Synth Small TIFF | single tile | 3 | 183 | 21 | 3 | 57 | 10 |
| zarr_read | Synth Small TIFF | small roi | 5 | 7 | 6 | 6 | 1 | 10 |
| zarr_read | Tiny NITF (1MB) | single tile | 3 | 8 | 5 | 4 | 1 | 10 |
| zarr_read | Tiny NITF (1MB) | small roi | 3 | 5 | 3 | 3 | 1 | 10 |
| zarr_read | Umbra SIDD | single tile | 41 | 101 | 50 | 44 | 18 | 10 |
| zarr_read | Umbra SIDD | small roi | 41 | 72 | 47 | 44 | 9 | 10 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 44 | 55 | 48 | 48 | 3 | 10 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 368 | 440 | 400 | 395 | 22 | 10 |
| zarr_read | WV Pan J2K (679MB) | large roi | 816 | 912 | 865 | 862 | 29 | 10 |
| zarr_read | WV Pan J2K (679MB) | single tile | 18 | 163 | 34 | 20 | 45 | 10 |
| zarr_read | WV Pan J2K (679MB) | small roi | 85 | 195 | 98 | 87 | 34 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr S3

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Synth COG Pyramid | single tile | 160 | 176 | 169 | 170 | 8 | 3 |
| zarr_read | Synth COG Pyramid | small roi | 178 | 191 | 183 | 179 | 7 | 3 |
| zarr_read | Synth Large NC | single tile | 258 | 293 | 273 | 268 | 18 | 3 |
| zarr_read | Synth Large NC | small roi | 362 | 791 | 527 | 428 | 231 | 3 |
| zarr_read | Synth Medium C3 | single tile | 179 | 188 | 183 | 182 | 4 | 3 |
| zarr_read | Synth Medium C3 | small roi | 192 | 220 | 204 | 201 | 14 | 3 |
| zarr_read | Synth Medium C8 | single tile | 172 | 179 | 175 | 175 | 3 | 3 |
| zarr_read | Synth Medium C8 | small roi | 196 | 199 | 197 | 196 | 2 | 3 |
| zarr_read | Synth NITF R-set Pyramid | single tile | 189 | 381 | 254 | 193 | 109 | 3 |
| zarr_read | Synth NITF R-set Pyramid | small roi | 228 | 253 | 241 | 241 | 13 | 3 |
| zarr_read | Synth Small NC | single tile | 190 | 196 | 192 | 191 | 3 | 3 |
| zarr_read | Synth Small NC | small roi | 217 | 258 | 234 | 229 | 21 | 3 |
| zarr_read | Synth Small TIFF | single tile | 188 | 195 | 192 | 193 | 4 | 3 |
| zarr_read | Synth Small TIFF | small roi | 209 | 235 | 224 | 228 | 14 | 3 |
| zarr_read | Tiny NITF (1MB) | single tile | 265 | 284 | 274 | 272 | 10 | 3 |
| zarr_read | Tiny NITF (1MB) | small roi | 238 | 272 | 252 | 245 | 18 | 3 |
| zarr_read | Umbra SIDD | single tile | 3020 | 3381 | 3185 | 3155 | 182 | 3 |
| zarr_read | Umbra SIDD | small roi | 2929 | 3659 | 3181 | 2956 | 414 | 3 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 504 | 1141 | 765 | 651 | 334 | 3 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 1812 | 1944 | 1882 | 1890 | 67 | 3 |
| zarr_read | WV Pan J2K (679MB) | large roi | 2649 | 2837 | 2742 | 2739 | 94 | 3 |
| zarr_read | WV Pan J2K (679MB) | single tile | 238 | 257 | 247 | 247 | 9 | 3 |
| zarr_read | WV Pan J2K (679MB) | small roi | 457 | 630 | 518 | 468 | 97 | 3 |

All times in milliseconds (ms).
