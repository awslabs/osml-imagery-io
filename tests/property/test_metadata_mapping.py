"""Property-based tests for the Mapping/MutableMapping metadata protocol.

This module tests universal correctness properties of the metadata provider
dictionary interface. It verifies round-trip symmetry, Mapping invariants,
and type preservation for the BufferedMetadataProvider.

Feature: metadata-mapping-api
"""

import collections.abc

import pytest
from aws.osml.io import BufferedMetadataProvider
from hypothesis import given
from hypothesis import strategies as st

from .conftest import pbt_settings

json_primitives = st.one_of(
    st.text(min_size=0, max_size=50, alphabet=st.characters(categories=("L", "N", "P", "S", "Z"))),
    st.integers(min_value=-(2**53), max_value=2**53),
    st.floats(allow_nan=False, allow_infinity=False),
    st.booleans(),
    st.none(),
)

json_values = st.recursive(
    json_primitives,
    lambda children: st.one_of(
        st.lists(children, max_size=5),
        st.dictionaries(
            st.text(min_size=1, max_size=10, alphabet=st.characters(categories=("L", "N"))),
            children,
            max_size=5,
        ),
    ),
    max_leaves=10,
)

metadata_keys = st.text(min_size=1, max_size=20, alphabet=st.characters(categories=("L", "N", "Pd")))


# =============================================================================
# Property 1: Round-trip symmetry — set then get returns same value
# =============================================================================


@pytest.mark.property
class TestRoundtripSymmetry:
    """Property: Any value set via __setitem__ is retrievable via __getitem__.

    *For any* key (non-empty string) and any JSON-compatible Python value,
    ``provider[key] = value`` followed by ``provider[key]`` SHALL return a
    value equal to the original.

    **Validates: Design §Testing Plan item 3 — round-trip symmetry invariants**
    """

    @given(key=metadata_keys, value=json_primitives)
    @pbt_settings
    def test_primitive_roundtrip(self, key, value):
        """Primitive values round-trip through set/get."""
        provider = BufferedMetadataProvider()
        provider[key] = value
        retrieved = provider[key]

        if isinstance(value, float):
            assert abs(retrieved - value) < 1e-10
        else:
            assert retrieved == value

    @given(key=metadata_keys, value=json_values)
    @pbt_settings
    def test_json_value_roundtrip(self, key, value):
        """Arbitrary JSON-compatible values round-trip through set/get."""
        provider = BufferedMetadataProvider()
        provider[key] = value
        retrieved = provider[key]

        _assert_json_equal(retrieved, value)


# =============================================================================
# Property 2: dict(metadata) == {k: metadata[k] for k in metadata}
# =============================================================================


@pytest.mark.property
class TestDictEquivalence:
    """Property: dict(metadata) equals manual iteration over keys.

    *For any* set of key-value pairs stored in a BufferedMetadataProvider,
    ``dict(metadata)`` SHALL equal ``{k: metadata[k] for k in metadata}``.

    **Validates: Design §Testing Plan item 3 — dict invariant**
    """

    @given(entries=st.dictionaries(metadata_keys, json_primitives, min_size=0, max_size=20))
    @pbt_settings
    def test_dict_equals_comprehension(self, entries):
        """dict(provider) equals {k: provider[k] for k in provider}."""
        provider = BufferedMetadataProvider()
        for k, v in entries.items():
            provider[k] = v

        from_dict = dict(provider)
        from_comprehension = {k: provider[k] for k in provider}

        assert set(from_dict.keys()) == set(from_comprehension.keys())
        for k in from_dict:
            _assert_json_equal(from_dict[k], from_comprehension[k])


# =============================================================================
# Property 3: len(metadata) == len(list(metadata))
# =============================================================================


@pytest.mark.property
class TestLenConsistency:
    """Property: len(metadata) equals len(list(metadata)).

    *For any* set of stored key-value pairs, ``len(metadata)`` SHALL equal
    ``len(list(metadata))``.

    **Validates: Design §Testing Plan item 3 — len invariant**
    """

    @given(entries=st.dictionaries(metadata_keys, json_primitives, min_size=0, max_size=30))
    @pbt_settings
    def test_len_equals_iteration_count(self, entries):
        """len(provider) equals the number of keys yielded by iteration."""
        provider = BufferedMetadataProvider()
        for k, v in entries.items():
            provider[k] = v

        assert len(provider) == len(list(provider))
        assert len(provider) == len(entries)


# =============================================================================
# Property 4: Copy preserves all entries
# =============================================================================


@pytest.mark.property
class TestCopyPreservesEntries:
    """Property: BufferedMetadataProvider(source=provider) copies all entries.

    *For any* populated metadata provider, constructing a new provider with
    ``source=`` SHALL produce a provider with identical entries.

    **Validates: Design §Testing Plan item 2 — round-trip via copy**
    """

    @given(entries=st.dictionaries(metadata_keys, json_primitives, min_size=1, max_size=15))
    @pbt_settings
    def test_copy_preserves_all_entries(self, entries):
        """Copying via source= constructor preserves all key-value pairs."""
        source = BufferedMetadataProvider()
        for k, v in entries.items():
            source[k] = v

        copied = BufferedMetadataProvider(source=source)

        assert len(copied) == len(source)
        for k in source:
            _assert_json_equal(copied[k], source[k])


# =============================================================================
# Property 5: entries() equals dict(provider)
# =============================================================================


@pytest.mark.property
class TestEntriesEqualsDict:
    """Property: entries() returns the same dict as dict(provider).

    **Validates: Design §Proposed Design — entries() as fast path for dict()**
    """

    @given(entries=st.dictionaries(metadata_keys, json_primitives, min_size=0, max_size=15))
    @pbt_settings
    def test_entries_equals_dict_conversion(self, entries):
        """provider.entries() returns same content as dict(provider)."""
        provider = BufferedMetadataProvider()
        for k, v in entries.items():
            provider[k] = v

        entries_result = provider.entries()
        dict_result = dict(provider)

        assert set(entries_result.keys()) == set(dict_result.keys())
        for k in entries_result:
            _assert_json_equal(entries_result[k], dict_result[k])


# =============================================================================
# Property 6: ABC registration
# =============================================================================


@pytest.mark.property
class TestABCRegistration:
    """Property: isinstance checks match the registered ABCs."""

    def test_metadata_provider_is_mapping(self):
        """MetadataProvider instances are recognized as Mapping."""
        provider = BufferedMetadataProvider()
        assert isinstance(provider, collections.abc.Mapping)

    def test_buffered_provider_is_mutable_mapping(self):
        """BufferedMetadataProvider instances are recognized as MutableMapping."""
        provider = BufferedMetadataProvider()
        assert isinstance(provider, collections.abc.MutableMapping)


# =============================================================================
# Helpers
# =============================================================================


def _assert_json_equal(actual, expected):
    """Assert two JSON-compatible values are equal, handling float tolerance."""
    if isinstance(expected, float) and isinstance(actual, float):
        assert abs(actual - expected) < 1e-10, f"Float mismatch: {actual} != {expected}"
    elif isinstance(expected, list) and isinstance(actual, list):
        assert len(actual) == len(expected), f"List length mismatch: {len(actual)} != {len(expected)}"
        for a, e in zip(actual, expected):
            _assert_json_equal(a, e)
    elif isinstance(expected, dict) and isinstance(actual, dict):
        assert set(actual.keys()) == set(expected.keys()), (
            f"Dict keys mismatch: {set(actual.keys())} != {set(expected.keys())}"
        )
        for k in expected:
            _assert_json_equal(actual[k], expected[k])
    else:
        assert actual == expected, f"Value mismatch: {actual!r} != {expected!r}"
