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

## TODO

- Add page sidebar table of contents
- Add tab logo/favicon
