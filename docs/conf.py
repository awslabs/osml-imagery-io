# Configuration file for the Sphinx documentation builder.
# https://www.sphinx-doc.org/en/master/usage/configuration.html

project = "osml-imagery-io"
copyright = "Amazon.com, Inc."
author = "AWS OSML"

extensions = [
    "myst_parser",
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
    "sphinx_autodoc_typehints",
    "sphinx.ext.intersphinx",
    "sphinxcontrib.mermaid",
]

# MyST settings
myst_enable_extensions = [
    "colon_fence",
    "fieldlist",
]
myst_fence_as_directive = ["mermaid"]

# Autodoc settings
autodoc_member_order = "bysource"

# Intersphinx for linking to NumPy docs, etc.
intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
    "numpy": ("https://numpy.org/doc/stable/", None),
}

# Theme
html_theme = "furo"

# Static files (images, custom CSS, etc.)
html_static_path = ["_static"]

# Exclude internal working notes from the published site
exclude_patterns = ["internal"]

# Suppress warnings from PyO3-generated docstrings (RST formatting issues
# in Rust doc comments that we cannot easily fix at the source).
suppress_warnings = ["docutils", "myst.xref_missing"]
