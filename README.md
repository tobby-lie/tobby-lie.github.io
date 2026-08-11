# tobby-lie.github.io

Personal site, built with Zola.

## Local development

    zola serve

## Deployment

Cloudflare Pages builds and deploys straight from master, no separate build branch. Build command:

    if [ "$CF_PAGES_BRANCH" = "master" ]; then zola build; else zola build --base-url $CF_PAGES_URL; fi

Output directory is `public`, declared in `wrangler.toml`. Custom domain is `tobbylie.me`.

## CI

`.github/workflows/site-ci.yaml` runs a Zola build check on every PR.

Note: CI builds with whatever Zola version `shalzz/zola-deploy-action@master` currently bundles (not pinned to a specific release), which can differ from your local `zola` install. A local `zola build` passing doesn't guarantee CI will — Tera template syntax has changed across Zola versions before (e.g. array indexing).
