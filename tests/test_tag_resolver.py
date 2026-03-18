"""Unit tests for TagNameResolver.

Tests cover default mappings, custom mappings, name-based lookup,
numeric access, error handling, iteration, and containment checks.
"""

import pytest

from aws.osml.io import TagNameResolver


# Sample Tag_Dictionary mimicking TIFFMetadataProvider output
SAMPLE_TAG_DICT = {
    "256": 512,
    "257": 512,
    "258": 8,
    "259": 1,
    "277": 3,
    "33550": [0.5, 0.5, 0.0],
    "34735": [1, 1, 1, 2, 1024, 0, 1, 1, 3072, 0, 1, 32618],
    "42113": "nan",
}


class TestDefaultMapping:
    """Verify the built-in DEFAULT_MAPPING contains expected entries."""

    def test_baseline_tiff_tags_present(self):
        mapping = TagNameResolver.DEFAULT_MAPPING
        assert mapping["ImageWidth"] == 256
        assert mapping["ImageLength"] == 257
        assert mapping["BitsPerSample"] == 258
        assert mapping["Compression"] == 259
        assert mapping["SamplesPerPixel"] == 277
        assert mapping["TileWidth"] == 322
        assert mapping["TileLength"] == 323
        assert mapping["SampleFormat"] == 339

    def test_geotiff_tags_present(self):
        mapping = TagNameResolver.DEFAULT_MAPPING
        assert mapping["ModelPixelScale"] == 33550
        assert mapping["ModelTiepoint"] == 33922
        assert mapping["ModelTransformation"] == 34264
        assert mapping["GeoKeyDirectory"] == 34735
        assert mapping["GeoDoubleParams"] == 34736
        assert mapping["GeoAsciiParams"] == 34737

    def test_gdal_tags_present(self):
        mapping = TagNameResolver.DEFAULT_MAPPING
        assert mapping["GDALMetadata"] == 42112
        assert mapping["GDALNoData"] == 42113


class TestNameLookup:
    """Test __getitem__ name-based lookup via mapping."""

    def test_lookup_by_name(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert resolver["ImageWidth"] == 512
        assert resolver["ImageLength"] == 512
        assert resolver["BitsPerSample"] == 8
        assert resolver["Compression"] == 1

    def test_lookup_geotiff_tag_by_name(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert resolver["ModelPixelScale"] == [0.5, 0.5, 0.0]
        assert resolver["GeoKeyDirectory"] == [1, 1, 1, 2, 1024, 0, 1, 1, 3072, 0, 1, 32618]

    def test_lookup_gdal_tag_by_name(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert resolver["GDALNoData"] == "nan"


class TestMissingNameError:
    """Test that missing names raise KeyError with descriptive messages."""

    def test_unknown_name_raises_key_error(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        with pytest.raises(KeyError, match="Unknown tag name"):
            resolver["CompletelyFakeTag"]

    def test_known_name_absent_tag_raises_key_error(self):
        # "Predictor" (317) is in the default mapping but not in our sample dict
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        with pytest.raises(KeyError, match="not present in metadata"):
            resolver["Predictor"]

    def test_unknown_name_error_includes_name(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        with pytest.raises(KeyError, match="'NoSuchTag'"):
            resolver["NoSuchTag"]


class TestCustomMapping:
    """Test that custom mappings override defaults."""

    def test_custom_mapping_overrides_default(self):
        # Override ImageWidth to point to tag 999
        custom = {"ImageWidth": 999}
        tag_dict = {"999": 1024, "256": 512}
        resolver = TagNameResolver(tag_dict, custom_mapping=custom)
        # Should use the custom mapping (999), not the default (256)
        assert resolver["ImageWidth"] == 1024

    def test_custom_mapping_adds_new_names(self):
        custom = {"MyCustomTag": 50000}
        tag_dict = {"50000": "custom_value"}
        resolver = TagNameResolver(tag_dict, custom_mapping=custom)
        assert resolver["MyCustomTag"] == "custom_value"

    def test_default_names_still_work_with_custom(self):
        custom = {"MyCustomTag": 50000}
        resolver = TagNameResolver(SAMPLE_TAG_DICT, custom_mapping=custom)
        # Default mapping should still resolve
        assert resolver["ImageWidth"] == 512


class TestByNumber:
    """Test by_number for direct numeric key access."""

    def test_by_number_present(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert resolver.by_number(256) == 512
        assert resolver.by_number(258) == 8
        assert resolver.by_number(42113) == "nan"

    def test_by_number_absent_raises_key_error(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        with pytest.raises(KeyError, match="99999"):
            resolver.by_number(99999)

    def test_by_number_private_tag(self):
        tag_dict = {"50000": [1, 2, 3]}
        resolver = TagNameResolver(tag_dict)
        assert resolver.by_number(50000) == [1, 2, 3]


class TestGetWithDefault:
    """Test get() with default value support."""

    def test_get_present_tag(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert resolver.get("ImageWidth") == 512

    def test_get_absent_tag_returns_none(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert resolver.get("Predictor") is None

    def test_get_absent_tag_returns_custom_default(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert resolver.get("Predictor", default=-1) == -1

    def test_get_unknown_name_returns_default(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert resolver.get("TotallyFake", default="fallback") == "fallback"


class TestContains:
    """Test __contains__ correctness."""

    def test_contains_present_name(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert "ImageWidth" in resolver
        assert "Compression" in resolver

    def test_contains_absent_tag(self):
        # Name is in mapping but tag not in dict
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert "Predictor" not in resolver

    def test_contains_unknown_name(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert "NoSuchTag" not in resolver

    def test_contains_custom_mapping(self):
        custom = {"MyTag": 50000}
        tag_dict = {"50000": "val"}
        resolver = TagNameResolver(tag_dict, custom_mapping=custom)
        assert "MyTag" in resolver


class TestIteration:
    """Test iteration resolves tag names and falls back to numeric keys."""

    def test_iter_resolves_known_names(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        items = dict(resolver)
        assert items["ImageWidth"] == 512
        assert items["ImageLength"] == 512
        assert items["BitsPerSample"] == 8
        assert items["Compression"] == 1
        assert items["SamplesPerPixel"] == 3
        assert items["ModelPixelScale"] == [0.5, 0.5, 0.0]
        assert items["GeoKeyDirectory"] == [1, 1, 1, 2, 1024, 0, 1, 1, 3072, 0, 1, 32618]
        assert items["GDALNoData"] == "nan"

    def test_iter_falls_back_to_numeric_for_unknown_tags(self):
        tag_dict = {"256": 512, "99999": "mystery"}
        resolver = TagNameResolver(tag_dict)
        items = dict(resolver)
        assert items["ImageWidth"] == 512
        assert items["99999"] == "mystery"

    def test_iter_custom_mapping_resolves(self):
        custom = {"MyTag": 50000}
        tag_dict = {"50000": "val", "256": 512}
        resolver = TagNameResolver(tag_dict, custom_mapping=custom)
        items = dict(resolver)
        assert items["MyTag"] == "val"
        assert items["ImageWidth"] == 512

    def test_iter_yields_all_pairs(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        items = list(resolver)
        assert len(items) == len(SAMPLE_TAG_DICT)

    def test_iter_empty_dict(self):
        resolver = TagNameResolver({})
        assert list(resolver) == []

    def test_len_matches_dict(self):
        resolver = TagNameResolver(SAMPLE_TAG_DICT)
        assert len(resolver) == len(SAMPLE_TAG_DICT)

    def test_len_empty(self):
        resolver = TagNameResolver({})
        assert len(resolver) == 0
