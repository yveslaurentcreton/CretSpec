"""Check internal links and heading anchors in the built documentation."""

from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urljoin, urlsplit

ROOT = Path(__file__).resolve().parents[1] / "website/dist"
BASE = "https://yveslaurentcreton.github.io/CretSpec/"


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__()
        self.links = []
        self.ids = set()
        self.feed(path.read_text(encoding="utf-8"))

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        if "id" in values:
            self.ids.add(values["id"])
        if tag == "a" and "href" in values:
            self.links.append(values["href"])


def main():
    pages = {path: Page(path) for path in ROOT.rglob("*.html")}
    if not pages:
        raise ValueError("Build the documentation before checking links")
    failures = []
    count = 0
    for source, page in pages.items():
        location = BASE + source.relative_to(ROOT).as_posix().removesuffix("index.html")
        for href in page.links:
            resolved = urljoin(location, href)
            if not resolved.startswith(BASE):
                continue
            url = urlsplit(resolved)
            relative = unquote(url.path.removeprefix("/CretSpec/"))
            target = ROOT / relative
            if target.is_dir():
                target /= "index.html"
            count += 1
            if not target.is_file():
                failures.append(f"{source.relative_to(ROOT)}: missing {href}")
            elif url.fragment and target in pages and unquote(url.fragment) not in pages[target].ids:
                failures.append(f"{source.relative_to(ROOT)}: missing anchor {href}")
    if failures:
        raise ValueError("\n".join(failures))
    print(f"Checked {count} internal links across {len(pages)} pages")


if __name__ == "__main__":
    main()
