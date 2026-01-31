"""SWAT recall utilities backed by wepp_interchange_rust."""

from __future__ import annotations

from ..wepp_interchange import wepp_interchange_rust as _rust

wepp_hillslope_pass_to_swat_recall = _rust.wepp_hillslope_pass_to_swat_recall

__all__ = ["wepp_hillslope_pass_to_swat_recall"]
