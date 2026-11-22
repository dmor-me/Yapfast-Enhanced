#!/usr/bin/env python3
"""Render this checkout's README as an offline, GitHub-style browser preview."""
import html
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
LOCAL_RENDERER = ROOT / '.local/preview-tools'
if LOCAL_RENDERER.is_dir():
    sys.path.insert(0, str(LOCAL_RENDERER))

try:
    import markdown
except ImportError:
    raise SystemExit('Install the optional renderer with: python -m pip install Markdown==3.7')

OUTPUT = ROOT / '.local/readme-preview/index.html'
STYLES = """
:root { color-scheme: light dark; }
* { box-sizing: border-box; }
body {
    margin: 0; background: light-dark(#ffffff, #0d1117);
    color: light-dark(#1f2328, #e6edf3);
    font: 16px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
article { max-width: 980px; margin: 30px auto; padding: 32px; }
h1, h2 { padding-bottom: .3em; border-bottom: 1px solid light-dark(#d1d9e0, #30363d); }
h1 { font-size: 2em; } h2 { margin-top: 1.4em; font-size: 1.5em; }
a { color: light-dark(#0969da, #58a6ff); text-decoration: none; }
a:hover { text-decoration: underline; }
code { background: light-dark(#eff1f3, #21262d); padding: .15em .35em; border-radius: 5px; }
pre { padding: 16px; overflow-x: auto; background: light-dark(#f6f8fa, #161b22); border-radius: 6px; }
pre code { padding: 0; background: transparent; }
img { max-width: 100%; }
li + li { margin-top: .3em; }
table { border-collapse: collapse; } td, th { border: 1px solid #7777; padding: 6px 13px; }
@media (max-width: 600px) { article { padding: 18px; margin: 0; } }
"""


def main():
    source = (ROOT / 'README.md').read_text(encoding='utf-8')
    content = markdown.markdown(
        source, extensions=['extra', 'toc', 'sane_lists'], output_format='html',
    )
    content = content.replace('href="#', f'href="{OUTPUT.as_uri()}#')
    # Keep the local logo and documentation links relative to the source checkout.
    base = html.escape(ROOT.as_uri() + '/', quote=True)
    document = (
        '<!doctype html><html lang="en"><head><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width, initial-scale=1">'
        '<title>Yapfast Enhanced — README preview</title>'
        f'<base href="{base}"><style>{STYLES}</style></head>'
        f'<body><article>{content}</article></body></html>'
    )
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(document, encoding='utf-8')
    print(OUTPUT)


if __name__ == '__main__':
    main()
