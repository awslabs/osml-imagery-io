### Read Performance Comparison

| Dataset | Access Pattern | Native | Zarr Local | Zarr S3 |
| --- | --- | --- | --- | --- |
| Synth Small TIFF | single tile | 1 | 3 | 150 |
| Synth COG Pyramid | single tile | 2 | 4 | 134 |
| Synth Small NC | small roi | 3 | 4 | 239 |
| Synth Medium C3 | single tile | 3 | 3 | 192 |
| Synth Small TIFF | small roi | 3 | 5 | 268 |
| Synth COG Pyramid | small roi | 4 | 5 | 181 |
| Synth Small NC | single tile | 4 | 3 | 158 |
| Tiny NITF (1MB) | small roi | 4 | 4 | 192 |
| Tiny NITF (1MB) | single tile | 5 | 8 | 450 |
| Synth NITF R-set Pyramid | single tile | 5 | 25 | 157 |
| Synth Large NC | single tile | 5 | 4 | 283 |
| Synth Medium C3 | small roi | 5 | 5 | 175 |
| Synth NITF R-set Pyramid | small roi | 6 | 5 | 203 |
| Synth Medium C8 | single tile | 6 | 5 | 139 |
| Synth Large NC | small roi | 7 | 28 | 516 |
| Umbra SIDD | small roi | 20 | 50 | 5222 |
| Synth Medium C8 | small roi | 22 | 21 | 270 |
| WV Pan J2K (679MB) | single tile | 24 | 34 | 308 |
| Umbra SIDD | single tile | 41 | 52 | 6515 |
| WV Pan J2K (679MB) | small roi | 128 | 128 | 624 |
| WV 8-band J2K (354MB) | single tile | 154 | 51 | 750 |
| WV 8-band J2K (354MB) | small roi | 1048 | 446 | 2377 |
| WV Pan J2K (679MB) | large roi | 1316 | 1085 | 3347 |

All times in milliseconds (ms).

### Tile Read Native

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| native_read | Synth Small TIFF | single tile | 1 | 2 | 1 | 1 | 0 | 10 |
| native_read | Synth COG Pyramid | single tile | 1 | 2 | 2 | 2 | 0 | 10 |
| native_read | Synth Small NC | small roi | 3 | 5 | 3 | 3 | 1 | 10 |
| native_read | Synth Medium C3 | single tile | 3 | 4 | 3 | 3 | 0 | 10 |
| native_read | Synth Small TIFF | small roi | 2 | 11 | 3 | 3 | 3 | 10 |
| native_read | Synth COG Pyramid | small roi | 3 | 4 | 4 | 4 | 0 | 10 |
| native_read | Synth Small NC | single tile | 2 | 7 | 4 | 3 | 1 | 10 |
| native_read | Tiny NITF (1MB) | small roi | 2 | 5 | 4 | 4 | 1 | 10 |
| native_read | Tiny NITF (1MB) | single tile | 3 | 7 | 5 | 4 | 1 | 10 |
| native_read | Synth NITF R-set Pyramid | single tile | 4 | 7 | 5 | 5 | 1 | 10 |
| native_read | Synth Large NC | single tile | 5 | 7 | 5 | 5 | 1 | 10 |
| native_read | Synth Medium C3 | small roi | 5 | 6 | 5 | 5 | 0 | 10 |
| native_read | Synth NITF R-set Pyramid | small roi | 5 | 8 | 6 | 5 | 1 | 10 |
| native_read | Synth Medium C8 | single tile | 5 | 7 | 6 | 6 | 1 | 10 |
| native_read | Synth Large NC | small roi | 4 | 21 | 7 | 5 | 5 | 10 |
| native_read | Umbra SIDD | small roi | 18 | 22 | 20 | 21 | 1 | 10 |
| native_read | Synth Medium C8 | small roi | 21 | 27 | 22 | 22 | 2 | 10 |
| native_read | WV Pan J2K (679MB) | single tile | 21 | 32 | 24 | 23 | 3 | 10 |
| native_read | Umbra SIDD | single tile | 21 | 201 | 41 | 23 | 56 | 10 |
| native_read | WV Pan J2K (679MB) | small roi | 113 | 142 | 128 | 128 | 11 | 10 |
| native_read | WV 8-band J2K (354MB) | single tile | 135 | 183 | 154 | 152 | 15 | 10 |
| native_read | WV 8-band J2K (354MB) | small roi | 837 | 1553 | 1048 | 953 | 232 | 10 |
| native_read | WV Pan J2K (679MB) | large roi | 1225 | 1437 | 1316 | 1307 | 70 | 10 |

All times in milliseconds (ms).

### Dted Parse

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| dted_open_and_parse | test_bench_dted_open_and_parse | 1 | 1 | 1 | 1 | 0 | 20 |

All times in milliseconds (ms).

### Dted Full Read

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| dted_full_read | test_bench_dted_full_read | 3 | 5 | 4 | 4 | 1 | 10 |

All times in milliseconds (ms).

### Index Generation

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| index_generation | Synth Small TIFF | 3 | 4 | 4 | 4 | 0 | 5 |
| index_generation | Synth COG Pyramid | 4 | 5 | 5 | 5 | 0 | 5 |
| index_generation | Umbra SIDD | 7 | 8 | 7 | 7 | 1 | 5 |
| index_generation | Synth Small NC | 7 | 10 | 8 | 8 | 1 | 5 |
| index_generation | Synth Medium C8 | 7 | 11 | 8 | 8 | 1 | 5 |
| index_generation | Synth Large NC | 9 | 11 | 10 | 9 | 1 | 5 |
| index_generation | Synth NITF R-set Pyramid | 7 | 13 | 10 | 10 | 3 | 5 |
| index_generation | Tiny NITF (1MB) | 6 | 18 | 11 | 9 | 4 | 5 |
| index_generation | Synth Medium C3 | 8 | 16 | 11 | 10 | 3 | 5 |
| index_generation | WV 8-band J2K (354MB) | 16 | 24 | 19 | 19 | 3 | 5 |
| index_generation | WV Pan J2K (679MB) | 66 | 103 | 76 | 71 | 15 | 5 |

All times in milliseconds (ms).

### Metadata

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| metadata_read | Synth Small TIFF | 1 | 2 | 1 | 1 | 0 | 10 |
| metadata_read | Synth COG Pyramid | 2 | 2 | 2 | 2 | 0 | 10 |
| metadata_read | Tiny NITF (1MB) | 3 | 7 | 4 | 4 | 1 | 10 |
| metadata_read | Synth Large NC | 4 | 8 | 6 | 6 | 1 | 10 |
| metadata_read | Umbra SIDD | 5 | 9 | 6 | 6 | 1 | 10 |
| metadata_read | Synth Small NC | 4 | 8 | 6 | 7 | 1 | 10 |
| metadata_read | Synth Medium C3 | 5 | 8 | 7 | 7 | 1 | 10 |
| metadata_read | Synth NITF R-set Pyramid | 6 | 8 | 7 | 7 | 1 | 10 |
| metadata_read | Synth Medium C8 | 6 | 9 | 7 | 7 | 1 | 10 |
| metadata_read | WV 8-band J2K (354MB) | 12 | 29 | 17 | 15 | 6 | 10 |
| metadata_read | WV Pan J2K (679MB) | 14 | 29 | 21 | 21 | 5 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr Local

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Synth Small TIFF | single tile | 2 | 4 | 3 | 3 | 1 | 10 |
| zarr_read | Synth Small NC | single tile | 2 | 4 | 3 | 3 | 1 | 10 |
| zarr_read | Synth Medium C3 | single tile | 3 | 5 | 3 | 3 | 1 | 10 |
| zarr_read | Synth COG Pyramid | single tile | 3 | 5 | 4 | 4 | 1 | 10 |
| zarr_read | Synth Large NC | single tile | 3 | 8 | 4 | 4 | 1 | 10 |
| zarr_read | Tiny NITF (1MB) | small roi | 3 | 8 | 4 | 4 | 1 | 10 |
| zarr_read | Synth Small NC | small roi | 4 | 5 | 4 | 4 | 0 | 10 |
| zarr_read | Synth Small TIFF | small roi | 4 | 6 | 5 | 4 | 0 | 10 |
| zarr_read | Synth COG Pyramid | small roi | 4 | 5 | 5 | 5 | 0 | 10 |
| zarr_read | Synth Medium C8 | single tile | 4 | 5 | 5 | 5 | 0 | 10 |
| zarr_read | Synth NITF R-set Pyramid | small roi | 4 | 7 | 5 | 5 | 1 | 10 |
| zarr_read | Synth Medium C3 | small roi | 5 | 6 | 5 | 5 | 0 | 10 |
| zarr_read | Tiny NITF (1MB) | single tile | 6 | 11 | 8 | 7 | 2 | 10 |
| zarr_read | Synth Medium C8 | small roi | 19 | 27 | 21 | 21 | 2 | 10 |
| zarr_read | Synth NITF R-set Pyramid | single tile | 3 | 208 | 25 | 4 | 64 | 10 |
| zarr_read | Synth Large NC | small roi | 9 | 180 | 28 | 11 | 53 | 10 |
| zarr_read | WV Pan J2K (679MB) | single tile | 20 | 133 | 34 | 23 | 35 | 10 |
| zarr_read | Umbra SIDD | small roi | 46 | 57 | 50 | 48 | 3 | 10 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 47 | 57 | 51 | 50 | 4 | 10 |
| zarr_read | Umbra SIDD | single tile | 43 | 107 | 52 | 46 | 20 | 10 |
| zarr_read | WV Pan J2K (679MB) | small roi | 108 | 222 | 128 | 119 | 34 | 10 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 397 | 517 | 446 | 431 | 38 | 10 |
| zarr_read | WV Pan J2K (679MB) | large roi | 980 | 1289 | 1085 | 1070 | 85 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr S3

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Synth COG Pyramid | single tile | 128 | 140 | 134 | 133 | 6 | 3 |
| zarr_read | Synth Medium C8 | single tile | 122 | 167 | 139 | 128 | 24 | 3 |
| zarr_read | Synth Small TIFF | single tile | 144 | 154 | 150 | 151 | 5 | 3 |
| zarr_read | Synth NITF R-set Pyramid | single tile | 129 | 193 | 157 | 148 | 33 | 3 |
| zarr_read | Synth Small NC | single tile | 125 | 218 | 158 | 131 | 52 | 3 |
| zarr_read | Synth Medium C3 | small roi | 156 | 185 | 175 | 184 | 16 | 3 |
| zarr_read | Synth COG Pyramid | small roi | 150 | 237 | 181 | 155 | 49 | 3 |
| zarr_read | Synth Medium C3 | single tile | 123 | 310 | 192 | 141 | 103 | 3 |
| zarr_read | Tiny NITF (1MB) | small roi | 177 | 207 | 192 | 193 | 15 | 3 |
| zarr_read | Synth NITF R-set Pyramid | small roi | 194 | 219 | 203 | 197 | 14 | 3 |
| zarr_read | Synth Small NC | small roi | 161 | 351 | 239 | 205 | 100 | 3 |
| zarr_read | Synth Small TIFF | small roi | 191 | 378 | 268 | 234 | 98 | 3 |
| zarr_read | Synth Medium C8 | small roi | 168 | 414 | 270 | 227 | 129 | 3 |
| zarr_read | Synth Large NC | single tile | 209 | 362 | 283 | 279 | 76 | 3 |
| zarr_read | WV Pan J2K (679MB) | single tile | 240 | 371 | 308 | 313 | 66 | 3 |
| zarr_read | Tiny NITF (1MB) | single tile | 213 | 911 | 450 | 227 | 399 | 3 |
| zarr_read | Synth Large NC | small roi | 451 | 562 | 516 | 536 | 58 | 3 |
| zarr_read | WV Pan J2K (679MB) | small roi | 512 | 721 | 624 | 638 | 105 | 3 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 547 | 1086 | 750 | 617 | 293 | 3 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 2241 | 2537 | 2377 | 2352 | 150 | 3 |
| zarr_read | WV Pan J2K (679MB) | large roi | 3136 | 3685 | 3347 | 3220 | 296 | 3 |
| zarr_read | Umbra SIDD | small roi | 4121 | 5776 | 5222 | 5771 | 954 | 3 |
| zarr_read | Umbra SIDD | single tile | 4629 | 7498 | 6515 | 7419 | 1634 | 3 |

All times in milliseconds (ms).
