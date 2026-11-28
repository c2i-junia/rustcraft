# Multiplayer

Rustcraft is played with others on a **dedicated server** that someone runs, or on a **hosted world** that the hub runs for you. Both are reached from **Multiplayer** on the main menu, which has two tabs: *Servers* and *Hosted worlds*.

## Your name and skin

The game asks for your name on the first launch. The button with your name on the main menu changes it. A name has 1 to 32 characters. Other players see it, and every world keeps your inventory and position under it: a new name starts afresh.

The **Skin** button of the main menu chooses one of the 12 skins that come with the game. To wear your own, drop a 64x64 PNG on the window while the Skin screen is open. The other players see your skin, an imported one included.

## Joining a server

1. Open **Multiplayer**, then the *Servers* tab.
2. Press **Add server** (Ctrl+N), give it a name and its address: a host name or an IPv4 address, with or without a port, such as play.example.com or 10.0.0.2:8000. Without a port, 8000 is used.
3. Select the server and press **Join** (Enter).

Each server shows its status: *Online* with its players (such as 3 / 20) and its latency, *Offline*, *Unknown host*, *Bad address* or *Incompatible* (another version of the game). A server that shows Offline may still be joinable: it may only be hiding its status port. Use the Up and Down arrows to pick a server, Del to remove it, F5 to refresh the list and Esc to go back.

Servers are reached over IPv4 only.

### When joining fails

| Message | What to do |
|---|---|
| Cannot reach "…" | The server did not answer within 10 seconds. Check that it is running and that both its UDP ports are open. |
| "…" runs another version | Join with the same version of Rustcraft as the server. |
| "…" is full | The server has no room for another player. Try again later. |
| "…" refused you | Most often, a player with your name is already on the server. Change your name and try again. |
| Your name is not accepted | A name has 1 to 32 characters and no control characters. |
| Connection to "…" lost, "…" stopped | The server stopped answering, or shut down. |
| Disconnected from "…" | The server dismissed you, and says why: you were kicked or banned, or you are not on its whitelist. |

## Accounts

You do not need an account to play alone or on a server that takes anyone. An account on the hub reserves your name on the servers that ask for accounts, and is needed for hosted worlds. Open it from the name button of the main menu:

- **Create account**: a username of 3 to 20 characters and a password of at least 10. The hub then shows a **recovery code, once**. Write it down: it is the only way back into the account if you forget the password.
- **Log in**: a session lasts 90 days after its last use.
- **Recover account**: your username, the recovery code and a new password.
- **Play offline as...** changes the name you play under without an account.

On a server that takes anyone, you are a different player logged in and logged out, each with its own inventory and position.

## Hosted worlds

The *Hosted worlds* tab, once you are logged in, lists the worlds of the hub you own or are a member of. You need neither a server nor an open port.

- **Join** starts a stopped world, waits up to a minute for it, and joins. Any member can start a world.
- Only the owner, the operators and the members can join. The owner adds players by username (**Members**), makes them operators or members again, and removes them.
- **Create a world** (name, optional seed, game mode, difficulty, world type) is only for accounts given the host role by whoever runs the hub.
- **Stop** saves and stops the world. **Delete** takes it off your list; the hub keeps its files for 30 days.
- A world nobody plays for 10 minutes saves and stops by itself. The official hub runs 3 worlds at a time: when they are all taken, try again later.

## Running your own server

The release archive for Windows and Linux holds the game and the dedicated server side by side: bin/rustcraft-server (bin\rustcraft-server.exe on Windows). Keep the data folder next to bin. Start it from a terminal:

**rustcraft-server --world survival --op YourName**

The --op option makes you the first operator. Then open **two UDP ports** in the firewall and forward them on the router: the game port (8000 by default) and the next one (8001), which answers the server list. Players join at your address and game port.

Stop the server with Ctrl+C: it saves the world and the players, then exits. The world is also saved every 60 seconds and when a player leaves.

### Server options

rustcraft-server --help lists them all. The seed, world type, game mode and difficulty only apply when the world is created: an existing world keeps its own.

| Option | Default | What it does |
|---|---|---|
| --port <port> | 8000 | The UDP port of the game. The server list asks for the status on port + 1. |
| --world <name> | default | The world to load from saves/<name>, created if it does not exist. |
| --seed <number> | random | The seed of a new world. |
| --world-type <type> | default | For a new world: default, large-biomes, archipelago or single-biome. |
| --biome <key> | | The biome of a single-biome world, such as plains. |
| --game-mode <mode> | survival | The game mode of the players of a new world: survival or creative. |
| --difficulty <level> | normal | For a new world: peaceful, easy, normal or hard. |
| --max-players <count> | 64 | The most players at once, from 1 to 64. |
| --max-render-distance <chunks> | 64 | The largest render distance a player may ask for. High values cost the server memory. |
| --op <name> | | Makes this player an operator before the server starts. May be repeated. |
| --auth <mode> | offline | offline takes anyone under the name they give; hub takes only players logged in to a hub account. |
| --public-address <host[:port]> | | The address players type to join. Needed with --auth hub. |
| --game-folder-path <path> | see below | Where the server keeps its saves and lists. |

The game folder holds the saves and the lists. By default it is ~/.local/share/rustcraft on Linux, %APPDATA%\rustcraft on Windows and ~/Library/Application Support/rustcraft on macOS. Each world is a folder under saves/.

### With Docker

Each release comes with an image of the server. The world goes to the volume:

**docker run -d --name rustcraft --read-only -v rustcraft-data:/data -p 8000:8000/udp -p 8001:8001/udp ghcr.io/c2i-junia/rustcraft/rustcraft-server**

Arguments after the image name replace the default --world world --port 8000, for example --world survival --seed 42 --max-players 8 --op Alice. The logs go to docker logs, and docker stop saves the world before exiting.

## Operators and permissions

On a dedicated server, every command but /list is for operators, the commands of mods included. Anyone else is answered "You must be an operator to use /…". In singleplayer you are always an operator. The commands are on the [[guide:commands|commands page]].

- /op <player> and /deop <player> make a player an operator, or no longer one.
- /kick <player> [reason] disconnects a player, who sees the reason. /ban <player> [reason] also refuses them until /pardon <player>.
- /whitelist on lets only the players on the whitelist, and the operators, join. /whitelist add, remove and list manage it; /whitelist off lets anyone in again.

Names are compared without their case. A name with spaces goes between quotes: /kick "Bad Guy". The server keeps the lists in its game folder as ops.ron, banned.ron and whitelist.ron. You can also make an operator by writing ["name"] in ops.ron while the server is stopped.

On a server started with --auth hub, the lists hold accounts rather than names: an operator stays one after changing their name. On a **hosted world** the owner manages members and operators from the Hosted worlds tab; there, /op, /deop, /ban, /pardon and /whitelist only answer so. /kick still works.

## What players share

- **The time and the weather** are the same for everyone. /time and /weather change them for the whole server.
- **The difficulty** belongs to the world.
- **The game mode** belongs to each player: /gamemode changes only your own. New players start in the world's game mode.
- **Sleeping**: a night is skipped only once every player online has used a [[Bedroll]] that night. Each player has their own respawn point.
- **Inventory, position, experience and achievements** are each player's own, kept by the world under their name (or their account on a hub server).
- **Fighting other players**: arrows, crossbow bolts and thrown axes hurt other players; melee blows only reach creatures.
- **Chat**: press T, type, and press Enter. Every player online reads it; the answer to a command goes only to whoever typed it.

Each player picks a render distance, from 1 to 64 chunks, in Settings > Video or with O and P. A server may grant less, up to its --max-render-distance, and the Video tab says so.
