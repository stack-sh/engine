#!/usr/bin/env python3

from pathlib import Path
import re
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
SNAPSHOTS = ROOT / "crates" / "stack-engine" / "tests" / "snapshots" / "render"
SVG_NAMESPACE = "http://www.w3.org/2000/svg"
FORBIDDEN_ELEMENTS = {"script", "foreignObject", "iframe", "object", "embed"}
URL_PATTERN = re.compile(r"url\(([^)]+)\)", re.IGNORECASE)


def local_name(name: str) -> str:
    return name.rsplit("}", 1)[-1]


def validate(path: Path) -> ET.Element:
    document = ET.parse(path)
    root = document.getroot()
    assert root.tag == f"{{{SVG_NAMESPACE}}}svg", f"{path}: root is not SVG"
    assert root.attrib.get("role") == "img", f"{path}: missing image role"
    assert root.attrib.get("aria-labelledby") == "stack-title stack-description"

    identifiers: set[str] = set()
    for element in root.iter():
        name = local_name(element.tag)
        assert name not in FORBIDDEN_ELEMENTS, f"{path}: forbidden {name} element"
        identifier = element.attrib.get("id")
        if identifier is not None:
            assert identifier not in identifiers, f"{path}: duplicate id {identifier}"
            identifiers.add(identifier)
        for attribute, value in element.attrib.items():
            attribute_name = local_name(attribute).lower()
            assert not attribute_name.startswith("on"), (
                f"{path}: event attribute {attribute_name}"
            )
            assert attribute_name not in {"href", "src"}, (
                f"{path}: external-reference attribute {attribute_name}"
            )
            for reference in URL_PATTERN.findall(value):
                assert reference == "#stack-arrow", (
                    f"{path}: non-local URL reference {reference}"
                )

    assert "stack-title" in identifiers, f"{path}: missing title"
    assert "stack-description" in identifiers, f"{path}: missing description"
    return root


def values(root: ET.Element, attribute: str) -> set[str]:
    return {
        value
        for element in root.iter()
        if (value := element.attrib.get(attribute)) is not None
    }


def main() -> None:
    snapshots = sorted(SNAPSHOTS.glob("*.svg"))
    assert snapshots, "no render snapshots found"
    documents = {path.stem: validate(path) for path in snapshots}
    complete = documents["complete-semantics"]
    explicit_icon = documents["explicit-core-icon"]
    assert values(complete, "data-node-kind") == {
        "actor",
        "client",
        "service",
        "function",
        "worker",
        "database",
        "cache",
        "queue",
        "storage",
        "external",
    }
    assert values(complete, "data-edge-kind") == {
        "flow",
        "request",
        "event",
        "data",
        "dependency",
    }
    assert values(complete, "data-edge-direction") == {
        "forward",
        "bidirectional",
        "association",
    }
    assert values(explicit_icon, "data-icon-id") == {"gateway"}
    assert explicit_icon.attrib.get("data-theme-version") == "0.5.0"
    assert (
        explicit_icon.attrib.get("data-theme-revision")
        == "sha256:3bfd66e1a96628b29b95b7273b54373bcce952f7285aefa506b4255a629eaf53"
    )


if __name__ == "__main__":
    main()
