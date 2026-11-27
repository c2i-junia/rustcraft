# wiki.rustcraft.dev

The players' wiki of Rustcraft, built with SvelteKit and Bun like the [website](../website). Every
page is prerendered: the build is a folder of static files that any web server can serve.

```sh
bun install
bun run dev        # http://localhost:5173, reloads on save
bun run check      # type-check
bun run build      # writes the wiki to build/
bun run data       # reads the game's tables again into src/lib/data/game.json (needs cargo)
```

## Where the facts come from

- `src/lib/data/game.json` is written by `shared/examples/wiki_export.rs` from the game's own code:
  items, blocks, recipes, smelting, alchemy, creatures, biomes, enchantments, effects, trades,
  achievements, and the English and French names. Run `bun run data` after changing the game.
- The textures are copied from `data/graphics/base_textures` into `static/textures` by
  `scripts/textures.ts`, before `dev` and `build`. They are not committed twice.
- The words of the item, creature and biome pages are the game's own, those its Codex shows:
  the `codex.*` keys of `data/lang/en.ron`, written in the Codex's markup
  (`shared/src/codex/markup.rs`), exported as blocks and lines that `src/lib/components/markup/`
  draws. Edit them there, in English and in French, and run `bun run data`.
- What the game rolls in code rather than in a table is written again in `src/lib/server/`:
  fishing (`fishing.ts`). The guides in `src/routes/guides/` are written by hand from the code.
  Keep them with the game when it changes.

## Writing a page

- A page is `src/routes/<path>/+page.svelte`, wrapped in `<Page title description>` with an
  optional `icon` snippet. Its `+page.server.ts` returns its data through `withRefs(data, ids)`:
  every item id or creature id found in the data, or listed in `ids`, gets the ref its links and
  icons need. A page that names an item only in its markup lists it in `ids`.
- Name an item with `<ItemLink id="IronIngot" />` (its icon, its name, a link to its page), show
  one in a slot with `<Slot id="IronIngot" count={3} />`, a recipe with `<Recipe {recipe} />`.
  Creatures are linked by their id too: `<ItemLink id="Gnawer" />`.
- Numbers are numbers of the game: read them from the code, give them in seconds and blocks, and
  write for a player, not a programmer. Facts the code does not settle are left out.
- Content is in English, in sentence case.

## Creature portraits

`static/creatures/*.png` are rendered from the game's models by Blender, as found in the wild
(without saddle, armour or chest). Render them again when a model changes:

```sh
blender -b -P wiki/scripts/render_creatures.py -- data/models/animated wiki/static/creatures [Kind…]
```

## Publishing

`https://wiki.rustcraft.dev` is served by Caddy on the VPS (see `hub/README.md`) from
`/srv/rustcraft-wiki`, behind Cloudflare. To publish, build here and copy the folder:

```sh
bun run build
rsync -az --delete build/ root@<vps>:/srv/rustcraft-wiki/
```

The block of `/etc/caddy/Caddyfile` (Caddy 2.6):

```caddy
wiki.rustcraft.dev {
	header Strict-Transport-Security "max-age=31536000"
	root * /srv/rustcraft-wiki
	encode zstd gzip
	@immutable path /_app/immutable/*
	header @immutable Cache-Control "public, max-age=31536000, immutable"
	try_files {path} {path}.html {path}/index.html
	file_server
	handle_errors {
		@missing expression {err.status_code} == 404
		rewrite @missing /404.html
		file_server
	}
}
```

`try_files` serves `/items/iron-ingot` from `items/iron-ingot.html`; a page that does not exist
gets `404.html`, which shows the wiki's own "No page here".
