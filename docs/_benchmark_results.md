### Read Performance Comparison

| Dataset | Access Pattern | Native | Zarr Local | Zarr S3 |
| --- | --- | --- | --- | --- |
| Synth COG Pyramid | single tile | 1 | 5 | 169 |
| Synth Small TIFF | single tile | 2 | 3 | 175 |
| Synth Small TIFF | small roi | 2 | 5 | 193 |
| Synth COG Pyramid | small roi | 3 | 23 | 189 |
| Synth Small NC | single tile | 3 | 5 | 205 |
| Tiny NITF (1MB) | small roi | 4 | 6 | 316 |
| Synth Medium C3 | single tile | 4 | 27 | 170 |
| Synth NITF R-set Pyramid | single tile | 4 | 4 | 198 |
| Synth Medium C8 | single tile | 5 | 5 | 158 |
| Tiny NITF (1MB) | single tile | 5 | 5 | 425 |
| Synth NITF R-set Pyramid | small roi | 5 | 6 | 193 |
| Synth Small NC | small roi | 5 | 6 | 278 |
| Synth Medium C3 | small roi | 8 | 7 | 190 |
| Synth Large NC | small roi | 21 | 11 | 618 |
| Synth Medium C8 | small roi | 22 | 45 | 204 |
| Synth Large NC | single tile | 26 | 5 | 389 |
| Umbra SIDD | small roi | 34 | 48 | 5690 |
| Umbra SIDD | single tile | 49 | 49 | 5124 |
| WV 8-band J2K (354MB) | single tile | 193 | 66 | 564 |
| WV Pan J2K (679MB) | single tile | 209 | 25 | 382 |
| WV Pan J2K (679MB) | small roi | 321 | 136 | 611 |
| WV 8-band J2K (354MB) | small roi | 1069 | 532 | 2347 |
| WV Pan J2K (679MB) | large roi | 1733 | 1339 | 3422 |

All times in milliseconds (ms).

### Tile Read Native

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| native_read | Synth COG Pyramid | single tile | 1 | 2 | 1 | 1 | 0 | 10 |
| native_read | Synth Small TIFF | single tile | 1 | 11 | 2 | 1 | 3 | 10 |
| native_read | Synth Small TIFF | small roi | 2 | 3 | 2 | 2 | 0 | 10 |
| native_read | Synth COG Pyramid | small roi | 2 | 4 | 3 | 3 | 1 | 10 |
| native_read | Synth Small NC | single tile | 2 | 7 | 3 | 3 | 1 | 10 |
| native_read | Tiny NITF (1MB) | small roi | 3 | 4 | 4 | 4 | 0 | 10 |
| native_read | Synth Medium C3 | single tile | 3 | 7 | 4 | 4 | 1 | 10 |
| native_read | Synth NITF R-set Pyramid | single tile | 3 | 9 | 4 | 4 | 2 | 10 |
| native_read | Synth Medium C8 | single tile | 4 | 6 | 5 | 5 | 1 | 10 |
| native_read | Tiny NITF (1MB) | single tile | 3 | 6 | 5 | 5 | 1 | 10 |
| native_read | Synth NITF R-set Pyramid | small roi | 4 | 6 | 5 | 5 | 1 | 10 |
| native_read | Synth Small NC | small roi | 4 | 7 | 5 | 5 | 1 | 10 |
| native_read | Synth Medium C3 | small roi | 6 | 16 | 8 | 7 | 3 | 10 |
| native_read | Synth Large NC | small roi | 19 | 22 | 21 | 21 | 1 | 10 |
| native_read | Synth Medium C8 | small roi | 20 | 24 | 22 | 22 | 1 | 10 |
| native_read | Synth Large NC | single tile | 17 | 90 | 26 | 20 | 22 | 10 |
| native_read | Umbra SIDD | small roi | 33 | 36 | 34 | 34 | 1 | 10 |
| native_read | Umbra SIDD | single tile | 34 | 156 | 49 | 37 | 38 | 10 |
| native_read | WV 8-band J2K (354MB) | single tile | 180 | 230 | 193 | 189 | 14 | 10 |
| native_read | WV Pan J2K (679MB) | single tile | 195 | 241 | 209 | 206 | 14 | 10 |
| native_read | WV Pan J2K (679MB) | small roi | 305 | 353 | 321 | 318 | 15 | 10 |
| native_read | WV 8-band J2K (354MB) | small roi | 1044 | 1113 | 1069 | 1061 | 24 | 10 |
| native_read | WV Pan J2K (679MB) | large roi | 1674 | 1791 | 1733 | 1734 | 34 | 10 |

All times in milliseconds (ms).

### Index Generation

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| index_generation | Synth Small TIFF | 2 | 2 | 2 | 2 | 0 | 5 |
| index_generation | Synth COG Pyramid | 4 | 5 | 5 | 5 | 0 | 5 |
| index_generation | Synth Medium C8 | 5 | 6 | 6 | 6 | 0 | 5 |
| index_generation | Tiny NITF (1MB) | 6 | 9 | 7 | 6 | 1 | 5 |
| index_generation | Synth Small NC | 6 | 8 | 7 | 7 | 1 | 5 |
| index_generation | Synth NITF R-set Pyramid | 7 | 8 | 7 | 7 | 0 | 5 |
| index_generation | Synth Medium C3 | 9 | 20 | 11 | 9 | 5 | 5 |
| index_generation | Synth Large NC | 20 | 22 | 22 | 22 | 1 | 5 |
| index_generation | Umbra SIDD | 27 | 31 | 28 | 27 | 2 | 5 |
| index_generation | WV 8-band J2K (354MB) | 99 | 494 | 180 | 103 | 176 | 5 |
| index_generation | WV Pan J2K (679MB) | 239 | 291 | 252 | 246 | 22 | 5 |

All times in milliseconds (ms).

### Metadata

| Operation | Dataset | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- |
| metadata_read | Synth Small TIFF | 1 | 1 | 1 | 1 | 0 | 10 |
| metadata_read | Synth COG Pyramid | 1 | 2 | 1 | 1 | 0 | 10 |
| metadata_read | Tiny NITF (1MB) | 2 | 3 | 3 | 3 | 0 | 10 |
| metadata_read | Synth Medium C8 | 3 | 5 | 4 | 4 | 0 | 10 |
| metadata_read | Synth Small NC | 3 | 6 | 4 | 4 | 1 | 10 |
| metadata_read | Synth Medium C3 | 3 | 7 | 4 | 4 | 1 | 10 |
| metadata_read | Synth NITF R-set Pyramid | 4 | 5 | 4 | 4 | 0 | 10 |
| metadata_read | Synth Large NC | 13 | 15 | 14 | 14 | 1 | 10 |
| metadata_read | Umbra SIDD | 15 | 17 | 16 | 16 | 1 | 10 |
| metadata_read | WV 8-band J2K (354MB) | 61 | 68 | 65 | 64 | 2 | 10 |
| metadata_read | WV Pan J2K (679MB) | 118 | 135 | 125 | 123 | 5 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr Local

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Synth Small TIFF | single tile | 2 | 3 | 3 | 3 | 0 | 10 |
| zarr_read | Synth NITF R-set Pyramid | single tile | 3 | 9 | 4 | 3 | 2 | 10 |
| zarr_read | Synth Large NC | single tile | 3 | 10 | 5 | 4 | 2 | 10 |
| zarr_read | Synth Small NC | single tile | 4 | 10 | 5 | 4 | 2 | 10 |
| zarr_read | Synth Small TIFF | small roi | 4 | 8 | 5 | 5 | 1 | 10 |
| zarr_read | Synth COG Pyramid | single tile | 4 | 7 | 5 | 5 | 1 | 10 |
| zarr_read | Synth Medium C8 | single tile | 4 | 7 | 5 | 5 | 1 | 10 |
| zarr_read | Tiny NITF (1MB) | single tile | 5 | 7 | 5 | 5 | 1 | 10 |
| zarr_read | Synth NITF R-set Pyramid | small roi | 5 | 10 | 6 | 5 | 1 | 10 |
| zarr_read | Synth Small NC | small roi | 5 | 12 | 6 | 6 | 2 | 10 |
| zarr_read | Tiny NITF (1MB) | small roi | 4 | 15 | 6 | 5 | 3 | 10 |
| zarr_read | Synth Medium C3 | small roi | 5 | 12 | 7 | 7 | 2 | 10 |
| zarr_read | Synth Large NC | small roi | 9 | 13 | 11 | 12 | 2 | 10 |
| zarr_read | Synth COG Pyramid | small roi | 5 | 178 | 23 | 6 | 54 | 10 |
| zarr_read | WV Pan J2K (679MB) | single tile | 22 | 30 | 25 | 25 | 3 | 10 |
| zarr_read | Synth Medium C3 | single tile | 4 | 217 | 27 | 5 | 67 | 10 |
| zarr_read | Synth Medium C8 | small roi | 21 | 231 | 45 | 24 | 66 | 10 |
| zarr_read | Umbra SIDD | small roi | 43 | 58 | 48 | 47 | 4 | 10 |
| zarr_read | Umbra SIDD | single tile | 45 | 61 | 49 | 47 | 5 | 10 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 59 | 76 | 66 | 65 | 5 | 10 |
| zarr_read | WV Pan J2K (679MB) | small roi | 124 | 162 | 136 | 133 | 11 | 10 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 518 | 544 | 532 | 533 | 9 | 10 |
| zarr_read | WV Pan J2K (679MB) | large roi | 1285 | 1406 | 1339 | 1338 | 34 | 10 |

All times in milliseconds (ms).

### Tile Read Zarr S3

| Operation | Dataset | Access Pattern | Min | Max | Mean | Median | StdDev | Rounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| zarr_read | Synth Medium C8 | single tile | 149 | 171 | 158 | 156 | 11 | 3 |
| zarr_read | Synth COG Pyramid | single tile | 148 | 195 | 169 | 164 | 24 | 3 |
| zarr_read | Synth Medium C3 | single tile | 139 | 222 | 170 | 150 | 45 | 3 |
| zarr_read | Synth Small TIFF | single tile | 169 | 183 | 175 | 173 | 7 | 3 |
| zarr_read | Synth COG Pyramid | small roi | 177 | 210 | 189 | 180 | 18 | 3 |
| zarr_read | Synth Medium C3 | small roi | 174 | 221 | 190 | 176 | 26 | 3 |
| zarr_read | Synth NITF R-set Pyramid | small roi | 182 | 210 | 193 | 186 | 15 | 3 |
| zarr_read | Synth Small TIFF | small roi | 179 | 212 | 193 | 188 | 17 | 3 |
| zarr_read | Synth NITF R-set Pyramid | single tile | 161 | 260 | 198 | 173 | 54 | 3 |
| zarr_read | Synth Medium C8 | small roi | 187 | 215 | 204 | 209 | 15 | 3 |
| zarr_read | Synth Small NC | single tile | 171 | 268 | 205 | 177 | 54 | 3 |
| zarr_read | Synth Small NC | small roi | 207 | 388 | 278 | 239 | 97 | 3 |
| zarr_read | Tiny NITF (1MB) | small roi | 236 | 392 | 316 | 320 | 78 | 3 |
| zarr_read | WV Pan J2K (679MB) | single tile | 265 | 498 | 382 | 384 | 116 | 3 |
| zarr_read | Synth Large NC | single tile | 253 | 628 | 389 | 285 | 208 | 3 |
| zarr_read | Tiny NITF (1MB) | single tile | 227 | 778 | 425 | 269 | 306 | 3 |
| zarr_read | WV 8-band J2K (354MB) | single tile | 440 | 762 | 564 | 489 | 173 | 3 |
| zarr_read | WV Pan J2K (679MB) | small roi | 434 | 869 | 611 | 531 | 229 | 3 |
| zarr_read | Synth Large NC | small roi | 585 | 643 | 618 | 628 | 30 | 3 |
| zarr_read | WV 8-band J2K (354MB) | small roi | 2217 | 2506 | 2347 | 2316 | 147 | 3 |
| zarr_read | WV Pan J2K (679MB) | large roi | 3162 | 3937 | 3422 | 3166 | 446 | 3 |
| zarr_read | Umbra SIDD | single tile | 4624 | 5470 | 5124 | 5277 | 443 | 3 |
| zarr_read | Umbra SIDD | small roi | 5319 | 6265 | 5690 | 5485 | 505 | 3 |

All times in milliseconds (ms).
