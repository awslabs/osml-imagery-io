# Configuration file for the Sphinx documentation builder.
# https://www.sphinx-doc.org/en/master/usage/configuration.html

project = "osml-imagery-io"
copyright = "Amazon.com, Inc."
author = "AWS OSML"

extensions = [
    "myst_parser",
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
    "sphinx.ext.todo",
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

# GitHub Pages base URL (Pages serves from /osml-imagery-io/)
html_baseurl = "https://awslabs.github.io/osml-imagery-io/"

# Static files (images, custom CSS, etc.)
html_static_path = ["_static"]

# Exclude internal working notes and included fragments from the published site
exclude_patterns = ["internal", "_benchmark_results.md"]

# Suppress warnings from PyO3-generated docstrings (RST formatting issues
# in Rust doc comments that we cannot easily fix at the source).
suppress_warnings = ["docutils", "myst.xref_missing", "autodoc.duplicate_object"]

# -- LaTeX / PDF output configuration ----------------------------------------

latex_engine = "pdflatex"

latex_documents = [
    (
        "index",                          # startdocname
        "osml-imagery-io.tex",            # targetname
        "osml-imagery-io Documentation",  # title
        "AWS OSML",                       # author
        "manual",                         # theme ('manual' or 'howto')
    ),
    (
        "user-guide/index",               # startdocname
        "osml-imagery-io-user-guide.tex", # targetname
        "osml-imagery-io User Guide",     # title
        "AWS OSML",                       # author
        "manual",                         # theme
    ),
]

latex_elements = {
    "papersize": "letterpaper",
    "pointsize": "11pt",
    # Remove blank pages between chapters for a more compact PDF
    "extraclassoptions": "openany,oneside",
    # Custom preamble: handle Unicode chars that pdflatex can't render natively
    "preamble": r"""
\usepackage{enumitem}
\setlistdepth{99}
\usepackage{newunicodechar}
\newunicodechar{✅}{\checkmark}
\newunicodechar{❌}{\texttimes}
\newunicodechar{🚧}{\textbf{[WIP]}}
\newunicodechar{✗}{\texttimes}
\newunicodechar{≤}{$\leq$}
\newunicodechar{≥}{$\geq$}
\newunicodechar{≈}{$\approx$}
\newunicodechar{↔}{$\leftrightarrow$}
\newunicodechar{→}{$\rightarrow$}
\newunicodechar{←}{$\leftarrow$}
\newunicodechar{—}{---}
\newunicodechar{–}{--}
\newunicodechar{×}{$\times$}
\newunicodechar{├}{|}
\newunicodechar{└}{|}
\newunicodechar{│}{|}
\newunicodechar{─}{-}
\newunicodechar{⚠}{\textbf{!}}
\DeclareUnicodeCharacter{FE0F}{}
""",
}
