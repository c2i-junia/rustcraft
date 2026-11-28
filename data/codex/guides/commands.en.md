# Commands

Commands are typed in the chat: press **T**, or **/** to open the chat with the slash already typed. As you type, the chat suggests command names, arguments and item names: take a suggestion with **Tab**, move between them with **Up** and **Down**. A line it already knows to be wrong shows what the server would answer.

## Who can use them

- **In singleplayer** you can run every command.
- **On a server**, only */list* is open to everyone. Every other command, those added by mods included, is for **operators**; others get "You must be an operator to use /…". See [[guide:multiplayer|multiplayer]] to become one.
- The answer to a command goes only to whoever typed it.

## All commands

- */time set <day|night>*: jumps forward to the next noon or midnight.
- */give <item> <count>*: gives you items.
- */gamemode <creative|survival>*: changes your own game mode.
- */weather <clear|rain|thunder>*: sets the weather.
- */difficulty <peaceful|easy|normal|hard>*: sets the difficulty of the world.
- */locate <place>*: finds the nearest biome, village, dungeon, shipwreck or cave biome.
- */list*: names the players online. Open to everyone.
- */op <player>*, */deop <player>*: makes a player an operator, or no longer one.
- */kick <player> [reason]*: disconnects a player.
- */ban <player> [reason]*, */pardon <player>*: bans a player, or lifts the ban.
- */whitelist <on|off|list>*, */whitelist <add|remove> <player>*: manages the whitelist.

Words in *<angle brackets>* are required; *[reason]* may be left out. *a|b* means one of the words.

## /give

- */give oak_log 64*
- */give iron_ingot 10*
- */give diamond_sword 1*

The count is required, and must be a whole number above 0. The items go to you, the player who typed the command. What does not fit in your inventory falls at your feet, up to another 36 stacks; anything beyond that is lost, and the answer says how many.

### Item names

An item's name is its English name in lower case, with underscores for spaces: [[OakLog]] is *oak_log*, [[IronIngot]] is *iron_ingot*. Case does not matter, and *OakLog* works too. The chat suggests names as you type them.

A few items have a command name that differs from the name you see:

{{table commands.renamed | Item | Command name}}

Every item is on the [[page:items|items page]].

## /time

*/time set day* jumps to the next noon, */time set night* to the next midnight. The clock never goes back, and on a server the time changes for everyone. See [[guide:weather|day, night and weather]].

## /weather

*/weather clear*, */weather rain* or */weather thunder* starts that weather for the length it would last on its own, then the cycle goes on as usual. It applies to the whole world.

## /gamemode

*/gamemode creative* or */gamemode survival* changes your own game mode only; there is no adventure mode. Case does not matter. Leaving creative while flying lands you. See [[guide:game-modes|game modes]].

## /difficulty

*/difficulty peaceful*, *easy*, *normal* or *hard* sets the difficulty of the world. Peaceful removes every hostile creature on the next tick. How difficulty changes the damage of hostiles is on the [[guide:combat|combat page]].

## /locate

*/locate* finds the nearest place of a kind to you and answers with its coordinates and distance, for example "The nearest village is at 412 -880, 976 blocks away". A search may take a moment; the answer comes when it is done.

| Command | Finds | The coordinates of |
|---|---|---|
| */locate biome <biome>* | A biome, within 4096 blocks | A column of it (X and Z) |
| */locate village* | A village, within 4096 blocks | Its centre (X and Z) |
| */locate ruin* | A ruin, within 2048 blocks | Its door (X, Y and Z) |
| */locate crypt* | A crypt, within 2048 blocks | Its door (X, Y and Z) |
| */locate sanctuary* | A sanctuary, within 2048 blocks | Its door (X, Y and Z) |
| */locate shipwreck* | A shipwreck, within 2048 blocks | Its chest (X, Y and Z) |
| */locate lush_caves* | Lush caves, within 2048 blocks | A spot in them (X, Y and Z) |
| */locate crystal_caves* | Crystal caves, within 2048 blocks | A spot in them (X, Y and Z) |
| */locate basalt_galleries* | Basalt galleries, within 2048 blocks | A spot in them (X, Y and Z) |

- Lush caves, crystal caves and basalt galleries exist only in deep worlds.
- A world created without villages or dungeons answers that it has none.
- See [[guide:structures|structures]] for what villages and dungeons hold.

### Biome keys

*/locate biome* takes the key of a biome, such as */locate biome jungle*. A biome added by a mod is written with its namespace, such as *mymod:name*; the base game's can also be written *rustcraft:jungle*.

{{table commands.biomes | Biome | Key}}

## /list

*/list* answers with the players online, in alphabetical order: "2 players online: Alex, Steve". Anyone can run it.

## Operator commands

These manage who may play on a dedicated server.

- */op Alex*, */deop Alex*: make Alex an operator, or no longer one.
- */kick Alex too loud*: disconnects Alex, who sees "You were kicked from the server: too loud".
- */ban Alex griefing*: disconnects Alex and refuses them from then on, with the reason. */pardon Alex* lifts the ban.
- */whitelist on* and */whitelist off*: when on, only the players on the whitelist and the operators can join. */whitelist add Alex*, */whitelist remove Alex* and */whitelist list* manage it.

Names are compared without their case. Except for */kick*, the player need not be online. A name that holds spaces goes between quotes: */ban "Bad Guy"*. On a hosted world, members are managed from the Hosted worlds tab instead, and only */kick* works.
