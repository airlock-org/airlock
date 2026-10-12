# Airlock website

This directory contains the Next.js landing page and Fumadocs application.
The output is a static site with a browser-side search index. It needs no Node
server, database, or search service after building.

## Install and preview

Use Node.js 24 or newer, pnpm 11.21.0, and Python 3.11 or newer. Install the
website dependencies explicitly, from the repository root:

```sh
pnpm --dir website install --frozen-lockfile
make website-dev
```

The development server listens on `http://127.0.0.1:3000`. The development
wrapper watches the selected Markdown sources and publication manifest, then
regenerates the collection when they change. Next.js handles application edits.

## Documentation ownership

Edit the canonical Markdown in `../docs/` and the existing root contributor,
security, and attribution guides. `docs.config.json` is the explicit publication
list and sidebar order. It supplies titles, descriptions, groups, and URL slugs.
Add new pages there deliberately; adding a repository file alone does not publish it.

`scripts/prepare-docs.py` creates the ignored `.generated-docs/` collection.
It removes the source's first H1 in favor of Fumadocs' page title, adds
frontmatter, and maps links to selected documents onto website URLs. Links to
other repository files/directories use GitHub source URLs; machine-component
links use the machine repository and work without checking out that component.
Their file targets are checked when the submodule is initialized. Those source links require repository access
while the repositories remain private. Relative links are validated before
replacing the generated collection. Code examples keep their literal paths.

Website assets belong in `public/`; components and styling belong here.
Documentation diagrams live in `public/diagrams/`. Link to them from repository
Markdown using a relative path; preparation maps these public assets to their
local website URLs so static exports do not depend on GitHub image requests.
`public/logo.svg` is the editable geometric recreation of the supplied logo,
with a transparent background. The header, hero, documentation, and footer
share that asset. `public/favicon.svg` uses the same geometry with solid colors
and a darker frame for light browser tabs; dark mode uses a lighter frame and
brighter orange details to keep the small icon visible.
The landing page follows the supplied navy/orange
mockup; its desktop preview is an illustration rather than a lab capture.
The locally served Inter variable font and its license live in `public/fonts/`;
provenance is recorded in [third-party attribution](../THIRD_PARTY.md).
Keep navigation and publication metadata in the manifest, and prose in Markdown.

## Check and build

```sh
pnpm --dir website check
make website-build
```

The static output is `website/out/`. Serve it with an ordinary static server:

```sh
python3 -m http.server 3000 --bind 127.0.0.1 --directory website/out
```

The export contains the landing page, every selected documentation page,
`404.html`, assets, and `docs/search.json`. Search loads this file and runs in
the browser. Directory-style URLs are emitted with trailing slashes.

The pytest preparation checks run in `make test-quick` without Node.js or website
dependency installation. To check the exported routes, internal links, and
search artifact after a build:

```sh
AIRLOCK_WEBSITE_OUT="$PWD/website/out" uv run --no-sync pytest tests/test_website_docs.py
```

Website CI has its own workflow and dependencies. It checks preparation,
TypeScript, the static build, and exported artifacts without checking out the
machine submodule. Website builds remain separate from Nix and machine deployment.

## Hosting

The verified deployment artifact is `out/`; upload its contents to static hosting.
The project expects deployment at the domain root, such as a `pages.dev` subdomain
or a custom domain. CI retains the static output as a downloadable artifact.

For a future Cloudflare Pages Git build, use `website` as the root directory,
`pnpm install --frozen-lockfile && pnpm build` as the build command, and `out`
as the output directory. Pin `NODE_VERSION=24`, `PNPM_VERSION=11.21.0`, and
`SKIP_DEPENDENCY_INSTALL=1` in its build settings. The build also needs Python
3.11 or newer. See [Cloudflare's build image configuration](https://developers.cloudflare.com/pages/configuration/build-image/).

Provider-managed checkout must account for the private SSH machine submodule
before enabling Git builds. The website itself does not need that checkout;
the GitHub website workflow explicitly disables submodule initialization.
Uploading the prebuilt artifact avoids provider-specific checkout setup.
See [Cloudflare's checkout troubleshooting](https://developers.cloudflare.com/pages/configuration/debugging-pages/#cloning-git-repository).

Cloudflare's Git integration requires an authorized connection to the repository.
This source change creates no hosted project, changes no account settings, and
publishes no site. Images, VM deployment, and website deployment are independent.
