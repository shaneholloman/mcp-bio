"""Shared types for ticket-369 source API scoring experiments."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class CommandProbe:
    """A current-BioMCP CLI probe."""

    group: str
    label: str
    args: list[str]
    expect_json: bool = True


@dataclass(frozen=True)
class HttpProbe:
    """A public HTTP API probe."""

    group: str
    service: str
    label: str
    method: str
    url: str
    body: dict[str, Any] | None = None
    headers: dict[str, str] | None = None
