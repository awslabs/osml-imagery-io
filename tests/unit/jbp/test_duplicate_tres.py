"""Unit tests for repeated TREs through the public Python API.

A NITF container holds a *sequence* of tagged record extensions and nothing in the
corpus requires the CETAG to be unique — STDI-0002 Volume 1 §2, and JBP §5.9.2
adds that extensions may appear "in any order". Before this was fixed, the
providers keyed TREs into a dict, so a repeated CETAG collapsed to the last
instance parsed and the earlier ones were unreachable through any public API
(awslabs/osml-imagery-io#12).

``i_3128b.ntf`` from the NITF 2.1 conformance set advertises the condition in its
own ``FTITLE`` — "3 PIAPE_tags" — and carries three ``PIAPEA`` envelopes in the
image subheader at offsets 2693, 2796, and 2899. Each identifies a different
person, so two thirds of the extension data used to be lost.

The fixture lives under ``data/integration/`` rather than ``data/unit/`` because
every checked-in unit fixture is produced by ``scripts/generate_test_data.py``,
and none of them repeats a CETAG. These tests skip when the integration corpus is
absent.
"""

from pathlib import Path

import pytest
from aws.osml.io import IO, BufferedImageAssetProvider, BufferedMetadataProvider

DUPLICATE_TRE_FIXTURE = Path("data/integration/Codice/JitcNitf21Samples/i_3128b.ntf")

requires_fixture = pytest.mark.skipif(
    not DUPLICATE_TRE_FIXTURE.exists(),
    reason=f"Integration fixture not found: {DUPLICATE_TRE_FIXTURE}",
)

# The three PIAPEA records in the fixture, in file order (STDI-0002 Vol 1 App C §C.4
# defines PIAPEA as one instance per person identified).
EXPECTED_PEOPLE = [
    ("DURHAM", "JAMES", "031260"),
    ("DAILEY", "RICHARD", "062146"),
    ("WEBB", "DAVE", "061856"),
]


@pytest.fixture
def image_metadata():
    """Image-segment metadata from the repeated-PIAPEA conformance file."""
    with IO.open([str(DUPLICATE_TRE_FIXTURE)], "r") as dataset:
        yield dataset.get_asset("image:0").metadata


@requires_fixture
def test_all_three_piapea_instances_are_reachable(image_metadata):
    """Every instance of a repeated TRE is returned, in file order."""
    instances = image_metadata.get_all("PIAPEA")

    assert len(instances) == 3, "PIAPEA appears three times in the subheader"
    assert [inst["LASTNME"].strip() for inst in instances] == [p[0] for p in EXPECTED_PEOPLE]
    assert [inst["FIRSTNME"].strip() for inst in instances] == [p[1] for p in EXPECTED_PEOPLE]
    assert [inst["DOB"] for inst in instances] == [p[2] for p in EXPECTED_PEOPLE]


@requires_fixture
def test_item_access_returns_the_first_instance(image_metadata):
    """``md[tag]`` is the first instance, not the last one parsed.

    First rather than last is what makes the behavior change visible: last-wins is
    precisely what the defect produced.
    """
    assert image_metadata["PIAPEA"] == image_metadata.get_all("PIAPEA")[0]
    assert image_metadata["PIAPEA"]["LASTNME"].strip() == "DURHAM"


@requires_fixture
def test_repeated_tag_counts_once_on_the_dict_surface(image_metadata):
    """The mapping surface stays monomorphic: one key per tag, value is a dict."""
    keys = list(image_metadata)

    assert keys.count("PIAPEA") == 1
    assert isinstance(image_metadata["PIAPEA"], dict)
    assert isinstance(image_metadata.entries()["PIAPEA"], dict)
    assert image_metadata.entries()["PIAPEA"] == image_metadata.get_all("PIAPEA")[0]
    assert len(keys) == len(image_metadata)


@requires_fixture
def test_tres_shape_is_uniform_regardless_of_count(image_metadata):
    """``get_all()`` always returns a list, so callers never branch on type."""
    # Present exactly once in this fixture's image subheader.
    assert len(image_metadata.get_all("PIAIMB")) == 1
    # Absent — an empty list, not None and not a KeyError.
    assert image_metadata.get_all("NOTHERE") == []


@requires_fixture
def test_keys_and_get_all_are_a_complete_enumeration(image_metadata):
    """``keys()`` + ``get_all()`` reaches every value, repeats included.

    This is the enumeration path, and it needs no notion of which keys are
    extensions: ``get_all`` is total over ``keys()``, so a repeated CETAG shows up as
    a longer list under its own key and everything else as a one-element list.
    """
    keys = image_metadata.keys()

    assert "PIAPEA" in keys
    assert keys.count("PIAPEA") == 1, "a repeated tag is still one key"
    assert "IID1" in keys, "plain subheader fields enumerate too"

    repeated = {key: image_metadata.get_all(key) for key in keys}
    assert all(instances for instances in repeated.values()), "get_all is total over keys()"
    assert len(repeated["PIAPEA"]) == 3
    assert len(repeated["IID1"]) == 1
    assert [key for key, values in repeated.items() if len(values) > 1] == ["PIAPEA"]


def test_tres_default_covers_providers_without_repeats():
    """The trait default gives every provider the same accessor semantics."""
    provider = BufferedMetadataProvider()
    provider["ICAT"] = "VIS"
    provider["RPC00B"] = {"SUCCESS": "1", "ERR_BIAS": "0000.00"}

    assert provider.get_all("RPC00B") == [{"SUCCESS": "1", "ERR_BIAS": "0000.00"}]
    assert provider.get_all("RPC00B")[0] == provider["RPC00B"]
    assert provider.get_all("MISSING") == []


# ---------------------------------------------------------------------------
# Round trip
# ---------------------------------------------------------------------------


@pytest.fixture
def round_tripped_piapea(tmp_path):
    """Copy ``i_3128b.ntf`` through the writer and read the result back.

    The copy applies no transformation: metadata moves across with
    ``BufferedMetadataProvider.from_provider``, which is the duplicate-safe path.
    A hand-rolled copy is not equivalent — ``update()`` is bulk ``__setitem__``
    and obeys the whole-slot rule, so ``update(src.entries())`` after
    ``set_all`` would discard every instance but the first.

    Yields ``(source_instances, round_tripped_metadata)``.
    """
    destination = tmp_path / "piapea-round-trip.ntf"

    with IO.open([str(DUPLICATE_TRE_FIXTURE)], "r") as source:
        image = source.get_asset("image:0")
        source_instances = image.metadata.get_all("PIAPEA")

        writer = IO.open([str(destination)], "w", "nitf")
        writer.metadata = BufferedMetadataProvider.from_provider(source.metadata)
        writer.add_asset(
            "image:0",
            BufferedImageAssetProvider.from_provider(
                image, metadata=BufferedMetadataProvider.from_provider(image.metadata)
            ),
            image.title,
            image.description,
            image.roles,
        )
        writer.close()

    with IO.open([str(destination)], "r") as reloaded:
        yield source_instances, reloaded.get_asset("image:0").metadata


@requires_fixture
def test_round_trip_preserves_every_instance_in_order(round_tripped_piapea):
    """Three instances in, three instances out — same order, same field values.

    This is the assertion that fails on *both* sides of the defect: the reader
    collapsed the three envelopes to one, and the writer could not have emitted
    more than one even if handed three. PIAPEA is a *known* TRE, so this exercises
    the fix rather than the pre-existing unknown-TRE write gap (unknown TREs are
    readable but still dropped on write — see awslabs/osml-imagery-io#11).
    """
    source_instances, metadata = round_tripped_piapea
    instances = metadata.get_all("PIAPEA")

    assert len(source_instances) == 3, "fixture precondition: three PIAPEA envelopes"
    assert len(instances) == 3, "the writer emitted every instance it was given"
    assert [inst["LASTNME"].strip() for inst in instances] == [p[0] for p in EXPECTED_PEOPLE]
    assert instances == source_instances, "every field survives untransformed"


@requires_fixture
def test_round_trip_keeps_the_instance_zero_invariant(round_tripped_piapea):
    """``md[tag] == md.get_all(tag)[0]`` holds on the reloaded file too."""
    _, metadata = round_tripped_piapea

    assert metadata["PIAPEA"] == metadata.get_all("PIAPEA")[0]
    assert metadata["PIAPEA"]["LASTNME"].strip() == "DURHAM"
    assert "PIAPEA" in metadata.keys()


# ---------------------------------------------------------------------------
# Overflow into a TRE_OVERFLOW DES
# ---------------------------------------------------------------------------
#
# An inline TRE container field is capped by its 5-digit length field: UDIDL/IXSHDL/
# XHDL max out at 99999, of which 3 bytes are the *OFL subfield, leaving 99996 bytes
# of payload. Anything beyond that goes into a TRE_OVERFLOW DES, and a single TRE is
# never split across the two (JBP-2021.2-037). Repeating one CETAG enough times is
# the easiest way to exceed the cap, which makes repeats and overflow interact: the
# reader rebuilds the container as inline-then-DES, so the writer has to place
# instances such that concatenation restores the authored order.


def piapea(last: str) -> dict:
    """A valid PIAPEA record (STDI-0002 Vol 1 App C §C.4), 92 bytes of CEDATA.

    Field widths: LASTNME/FIRSTNME/MIDNME 28 each, DOB 6, ASSOCTRY 2.
    """
    return {
        "LASTNME": last.ljust(28),
        "FIRSTNME": "TEST".ljust(28),
        "MIDNME": "Q.".ljust(28),
        "DOB": "010170",
        "ASSOCTRY": "US",
    }


# CETAG (6) + CEL (5) + CEDATA (92).
PIAPEA_ENVELOPE_SIZE = 6 + 5 + 92
MAX_INLINE_TRE_BYTES = 99996
PIAPEA_FIT_INLINE = MAX_INLINE_TRE_BYTES // PIAPEA_ENVELOPE_SIZE  # 970


def secura(seclen: int, marker: str) -> dict:
    """A SECURA record whose size is driven by SECLEN (STDI-0002 Vol 1 App AI).

    SECURA is the one shipped TRE whose length is caller-controlled, which is what
    makes a *mixed-size* repeat expressible. ``SECSTD`` carries an order marker.
    """
    return {
        "FDATTIM": "20240101120000",
        "FORMATVER": "NITF02.10",
        "SECFLDS": " " * 207,
        "SECSTD": marker.ljust(7)[:7],
        "SECCOMP": "",
        "SECLEN": f"{seclen:05d}",
        # `bytes`-typed fields round-trip as lowercase hex; 0x41 is 'A'.
        "SECURITY": "41" * seclen,
    }


def secura_envelope_size(seclen: int) -> int:
    """CETAG + CEL + the fixed SECURA fields + SECLEN bytes of SECURITY."""
    return 6 + 5 + 251 + seclen


def write_image_with_tres(path, metadata) -> None:
    """Write a minimal 8x8 single-band NITF carrying *metadata* on the image."""
    numpy = pytest.importorskip("numpy")
    from aws.osml.io import PixelType

    provider = BufferedImageAssetProvider.create(
        key="image:0",
        num_columns=8,
        num_rows=8,
        num_bands=1,
        block_width=8,
        block_height=8,
        pixel_type=PixelType.UInt8,
        metadata=metadata,
    )
    provider.set_full_image(numpy.zeros((8, 8, 1), dtype=numpy.uint8))

    with IO.open([str(path)], "w", "nitf") as writer:
        writer.add_asset("image:0", provider, "Image", "", ["data"])


@pytest.fixture
def overflowing_piapea(tmp_path):
    """Author more PIAPEA instances than fit inline, write, and read back.

    1000 instances at 103 bytes each is 103,000 bytes — past the 99,996-byte inline
    cap, so 970 stay in ``UDID`` and the remaining 30 spill into a TRE_OVERFLOW DES.
    Each instance is tagged in ``LASTNME`` so order is checkable across the boundary.

    Yields ``(path, instance_count, reloaded_dataset_reader_results)``.
    """
    count = 1000
    authored = [piapea(f"P{i:05d}") for i in range(count)]

    metadata = BufferedMetadataProvider()
    metadata["ICAT"] = "VIS"
    metadata.set_all("PIAPEA", authored)

    path = tmp_path / "piapea-overflow.ntf"
    write_image_with_tres(path, metadata)

    with IO.open([str(path)], "r") as reloaded:
        image = reloaded.get_asset("image:0")
        yield path, count, image.metadata, reloaded.get_asset_keys()


def test_overflowing_repeat_survives_the_round_trip(overflowing_piapea):
    """Every instance comes back, in order, across the inline/DES boundary."""
    _, count, metadata, _ = overflowing_piapea
    instances = metadata.get_all("PIAPEA")

    assert len(instances) == count, "no instance is lost to the overflow split"
    assert [inst["LASTNME"].strip() for inst in instances] == [
        f"P{i:05d}" for i in range(count)
    ], "inline instances then DES instances must reconstruct the authored order"
    assert metadata["PIAPEA"] == instances[0], "the instance-0 invariant holds"


def test_overflowing_repeat_actually_spills_into_a_des(overflowing_piapea):
    """The test above would pass vacuously if everything had stayed inline."""
    path, _, metadata, asset_keys = overflowing_piapea

    # UDIDL counts the 3-byte UDOFL subfield plus the inline envelopes.
    inline_bytes = int(metadata["UDIDL"]) - 3
    assert inline_bytes == PIAPEA_FIT_INLINE * PIAPEA_ENVELOPE_SIZE
    assert inline_bytes <= MAX_INLINE_TRE_BYTES

    # UDOFL is the 1-based index of the DES holding the remainder.
    assert int(metadata["UDOFL"]) > 0, "UDOFL must point at the overflow DES"
    assert any(key.startswith("des:") for key in asset_keys), "a DES segment was added"
    assert b"TRE_OVERFLOW" in path.read_bytes()


def test_overflow_des_identifies_the_container_it_continues(overflowing_piapea):
    """DESOFLW/DESITEM name the field and segment the DES continues."""
    path, _, _, _ = overflowing_piapea

    with IO.open([str(path)], "r") as reloaded:
        des = reloaded.get_asset("des:0").metadata

        assert des["DESID"].strip() == "TRE_OVERFLOW"
        assert des["DESOFLW"].strip() == "UDID", "the image user-defined field overflowed"
        assert int(des["DESITEM"]) == 1, "1-based index of the image segment"


def test_mixed_size_repeat_keeps_order_across_the_boundary(tmp_path):
    """A big instance that overflows must not be overtaken by a smaller follower.

    The inline field is packed as a *prefix*: the split happens at the first
    envelope that does not fit, and everything from there spills. Packing greedily
    instead — giving the leftover room to a later, smaller envelope — reorders the
    sequence, because the reader concatenates inline then DES. That is silent
    corruption for a repeated CETAG whose instance order carries meaning: `CSEPHA`
    is defined in time-sequence order (Vol 1 App D) and `BCHIPA` instances form a
    UUID-linked series (Vol 1 App AR).
    """
    small = 1000
    small_size = secura_envelope_size(small)
    # Fill inline with small instances, leaving room that the big one cannot use.
    fill = (MAX_INLINE_TRE_BYTES - small_size - 100) // small_size
    big = MAX_INLINE_TRE_BYTES - fill * small_size + 50
    assert secura_envelope_size(big) > MAX_INLINE_TRE_BYTES - fill * small_size
    assert small_size <= MAX_INLINE_TRE_BYTES - fill * small_size, (
        "precondition: a small follower *would* fit in the leftover room"
    )

    authored = [secura(small, f"S{i:04d}") for i in range(fill)]
    authored.append(secura(big, "BIG"))
    authored.append(secura(small, "LAST"))

    metadata = BufferedMetadataProvider()
    metadata["FTITLE"] = "SECURA mixed-size overflow"
    metadata.set_all("SECURA", authored)

    path = tmp_path / "secura-mixed-overflow.ntf"
    writer = IO.open([str(path)], "w", "nitf")
    writer.metadata = metadata
    writer.close()

    with IO.open([str(path)], "r") as reloaded:
        instances = reloaded.metadata.get_all("SECURA")

        assert len(instances) == len(authored)
        assert [inst["SECSTD"].strip() for inst in instances] == [
            f"S{i:04d}" for i in range(fill)
        ] + ["BIG", "LAST"], "BIG must stay ahead of LAST"
        assert int(reloaded.metadata["XHDLOFL"]) > 0, "the file header overflowed"
