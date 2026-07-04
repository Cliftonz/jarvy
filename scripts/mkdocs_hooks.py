"""MkDocs build hooks for the Jarvy docs site.

MkDocs ignores files and directories whose names begin with a dot, so a
``docs/.well-known/`` tree never reaches the built ``site/`` on its own.
This hook copies it verbatim after the build, exposing agent- and
security-facing discovery files at ``https://jarvy.dev/.well-known/...``
(RFC 9116 ``security.txt`` and any future well-known resources).

Wired via the ``hooks:`` key in ``mkdocs.yml``. Paths there are resolved
relative to the config file (repo root).
"""

from __future__ import annotations

import shutil
from pathlib import Path


def on_post_build(config, **kwargs) -> None:
    """Copy ``docs/.well-known/`` into the built site after each build."""
    docs_dir = Path(config["docs_dir"])
    site_dir = Path(config["site_dir"])

    src = docs_dir / ".well-known"
    if not src.is_dir():
        return

    dst = site_dir / ".well-known"
    shutil.copytree(src, dst, dirs_exist_ok=True)
