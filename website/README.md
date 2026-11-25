# rustcraft.dev

The project's website, built with SvelteKit and Bun. Every page is prerendered: the build is a
folder of static files that any web server can serve.

```sh
bun install
bun run dev        # http://localhost:5173, reloads on save
bun run check      # type-check
bun run build      # writes the site to build/
bun run preview    # serves build/ locally
```

To publish it, copy `build/` to the server and serve it as the root of `rustcraft.dev`.
