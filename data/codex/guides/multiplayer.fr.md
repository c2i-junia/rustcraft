# Multijoueur

Rustcraft se joue à plusieurs sur un **serveur dédié** que quelqu’un fait tourner, ou sur un **monde hébergé** que le hub fait tourner pour toi. On y accède par **Multijoueur** dans le menu principal, qui a deux onglets : *Serveurs* et *Mondes hébergés*.

## Ton nom et ton apparence

Le jeu te demande ton nom au premier lancement. Le bouton portant ton nom, dans le menu principal, permet de le changer. Un nom compte de 1 à 32 caractères. Les autres joueurs le voient, et chaque monde garde ton inventaire et ta position sous ce nom : un nouveau nom repart de zéro.

Le bouton **Apparence** du menu principal choisit l’une des 12 apparences fournies avec le jeu. Pour porter la tienne, dépose un PNG de 64x64 sur la fenêtre pendant que l’écran Apparence est ouvert. Les autres joueurs voient ton apparence, importée comprise.

## Rejoindre un serveur

1. Ouvre **Multijoueur**, puis l’onglet *Serveurs*.
2. Appuie sur **Ajouter un serveur** (Ctrl+N), donne-lui un nom et son adresse : un nom d’hôte ou une adresse IPv4, avec ou sans port, comme play.example.com ou 10.0.0.2:8000. Sans port, c’est 8000 qui est utilisé.
3. Sélectionne le serveur et appuie sur **Rejoindre** (Entrée).

Chaque serveur affiche son état : *En ligne* avec ses joueurs (par exemple 3 / 20) et sa latence, *Hors ligne*, *Hôte inconnu*, *Adresse invalide* ou *Incompatible* (une autre version du jeu). Un serveur affiché Hors ligne peut quand même être joignable : il se peut qu’il cache seulement son port d’état. Utilise les flèches Haut et Bas pour choisir un serveur, Suppr pour le retirer, F5 pour actualiser la liste et Échap pour revenir.

Les serveurs ne sont joignables qu’en IPv4.

### Quand la connexion échoue

| Message | Que faire |
|---|---|
| Impossible de joindre « … » | Le serveur n’a pas répondu en 10 secondes. Vérifie qu’il tourne et que ses deux ports UDP sont ouverts. |
| « … » a une autre version | Rejoins avec la même version de Rustcraft que le serveur. |
| « … » est plein | Le serveur n’a plus de place pour un joueur de plus. Réessaie plus tard. |
| « … » vous a refusé | Le plus souvent, un joueur portant ton nom est déjà sur le serveur. Change de nom et réessaie. |
| Votre nom n’est pas accepté | Un nom compte de 1 à 32 caractères, sans caractère de contrôle. |
| Connexion à « … » perdue, « … » arrêté | Le serveur a cessé de répondre, ou s’est éteint. |
| Déconnecté de « … » | Le serveur t’a renvoyé, et dit pourquoi : tu as été expulsé ou banni, ou tu n’es pas sur sa liste blanche. |

## Comptes

Tu n’as pas besoin de compte pour jouer seul ou sur un serveur qui accepte tout le monde. Un compte sur le hub réserve ton nom sur les serveurs qui demandent des comptes, et il est nécessaire pour les mondes hébergés. Ouvre-le depuis le bouton de nom du menu principal :

- **Créer un compte** : un nom d’utilisateur de 3 à 20 caractères et un mot de passe d’au moins 10. Le hub affiche alors un **code de récupération, une seule fois**. Note-le : c’est le seul moyen de retrouver le compte si tu oublies ton mot de passe.
- **Se connecter** : une session dure 90 jours après sa dernière utilisation.
- **Récupérer un compte** : ton nom d’utilisateur, le code de récupération et un nouveau mot de passe.
- **Jouer hors ligne en tant que…** change le nom sous lequel tu joues, sans compte.

Sur un serveur qui accepte tout le monde, tu es un joueur différent connecté et déconnecté, chacun avec son propre inventaire et sa propre position.

## Mondes hébergés

L’onglet *Mondes hébergés*, une fois connecté, liste les mondes du hub dont tu es propriétaire ou membre. Tu n’as besoin ni d’un serveur ni d’un port ouvert.

- **Rejoindre** démarre un monde arrêté, l’attend jusqu’à une minute, et te connecte. N’importe quel membre peut démarrer un monde.
- Seuls le propriétaire, les opérateurs et les membres peuvent rejoindre. Le propriétaire ajoute des joueurs par nom d’utilisateur (**Membres**), les fait opérateurs ou de nouveau membres, et les retire.
- **Créer un monde** (nom, graine facultative, mode de jeu, difficulté, type de monde) est réservé aux comptes à qui celui qui gère le hub a donné le rôle d’hôte.
- **Arrêter** sauvegarde et arrête le monde. **Supprimer** le retire de ta liste ; le hub garde ses fichiers 30 jours.
- Un monde où personne ne joue pendant 10 minutes se sauvegarde et s’arrête de lui-même. Le hub officiel fait tourner 3 mondes à la fois : quand ils sont tous pris, réessaie plus tard.

## Faire tourner ton propre serveur

L’archive de la version pour Windows et Linux contient le jeu et le serveur dédié côte à côte : bin/rustcraft-server (bin\rustcraft-server.exe sous Windows). Garde le dossier data à côté de bin. Lance-le depuis un terminal :

**rustcraft-server --world survival --op TonNom**

L’option --op fait de toi le premier opérateur. Ouvre ensuite **deux ports UDP** dans le pare-feu et redirige-les sur le routeur : le port du jeu (8000 par défaut) et le suivant (8001), qui répond à la liste des serveurs. Les joueurs te rejoignent à ton adresse et sur le port du jeu.

Arrête le serveur avec Ctrl+C : il sauvegarde le monde et les joueurs, puis quitte. Le monde est aussi sauvegardé toutes les 60 secondes et quand un joueur part.

### Options du serveur

rustcraft-server --help les liste toutes. La graine, le type de monde, le mode de jeu et la difficulté ne s’appliquent qu’à la création du monde : un monde existant garde les siens.

| Option | Par défaut | Ce qu’elle fait |
|---|---|---|
| --port <port> | 8000 | Le port UDP du jeu. La liste des serveurs demande l’état sur le port + 1. |
| --world <name> | default | Le monde à charger depuis saves/<name>, créé s’il n’existe pas. |
| --seed <number> | aléatoire | La graine d’un nouveau monde. |
| --world-type <type> | default | Pour un nouveau monde : default, large-biomes, archipelago ou single-biome. |
| --biome <key> | | Le biome d’un monde à biome unique, comme plains. |
| --game-mode <mode> | survival | Le mode de jeu des joueurs d’un nouveau monde : survival ou creative. |
| --difficulty <level> | normal | Pour un nouveau monde : peaceful, easy, normal ou hard. |
| --max-players <count> | 64 | Le nombre maximal de joueurs en même temps, de 1 à 64. |
| --max-render-distance <chunks> | 64 | La plus grande distance d’affichage qu’un joueur peut demander. Les valeurs élevées coûtent de la mémoire au serveur. |
| --op <name> | | Fait de ce joueur un opérateur avant le démarrage du serveur. Peut être répétée. |
| --auth <mode> | offline | offline accepte n’importe qui sous le nom qu’il donne ; hub n’accepte que les joueurs connectés à un compte du hub. |
| --public-address <host[:port]> | | L’adresse que les joueurs saisissent pour rejoindre. Nécessaire avec --auth hub. |
| --game-folder-path <path> | voir plus bas | L’endroit où le serveur garde ses sauvegardes et ses listes. |

Le dossier du jeu contient les sauvegardes et les listes. Par défaut, c’est ~/.local/share/rustcraft sous Linux, %APPDATA%\rustcraft sous Windows et ~/Library/Application Support/rustcraft sous macOS. Chaque monde est un dossier dans saves/.

### Avec Docker

Chaque version est livrée avec une image du serveur. Le monde va dans le volume :

**docker run -d --name rustcraft --read-only -v rustcraft-data:/data -p 8000:8000/udp -p 8001:8001/udp ghcr.io/c2i-junia/rustcraft/rustcraft-server**

Les arguments placés après le nom de l’image remplacent les valeurs par défaut --world world --port 8000, par exemple --world survival --seed 42 --max-players 8 --op Alice. Les journaux vont dans docker logs, et docker stop sauvegarde le monde avant de quitter.

## Opérateurs et permissions

Sur un serveur dédié, toutes les commandes sauf /list sont réservées aux opérateurs, celles des mods comprises. Tout autre joueur reçoit « Vous devez être opérateur pour utiliser /… ». En solo, tu es toujours opérateur. Les commandes sont sur la [[guide:commands|page des commandes]].

- /op <player> et /deop <player> font d’un joueur un opérateur, ou lui retirent ce statut.
- /kick <player> [reason] déconnecte un joueur, qui voit la raison. /ban <player> [reason] le refuse en plus, jusqu’à /pardon <player>.
- /whitelist on n’accepte que les joueurs de la liste blanche, et les opérateurs. /whitelist add, remove et list la gèrent ; /whitelist off laisse de nouveau entrer tout le monde.

Les noms se comparent sans tenir compte de la casse. Un nom avec des espaces se met entre guillemets : /kick "Bad Guy". Le serveur garde les listes dans son dossier de jeu, sous les noms ops.ron, banned.ron et whitelist.ron. Tu peux aussi créer un opérateur en écrivant ["name"] dans ops.ron pendant que le serveur est arrêté.

Sur un serveur lancé avec --auth hub, les listes contiennent des comptes plutôt que des noms : un opérateur le reste après avoir changé de nom. Sur un **monde hébergé**, le propriétaire gère les membres et les opérateurs depuis l’onglet Mondes hébergés ; là, /op, /deop, /ban, /pardon et /whitelist ne font que le dire. /kick fonctionne toujours.

## Ce que les joueurs partagent

- **L’heure et la météo** sont les mêmes pour tous. /time et /weather les changent pour tout le serveur.
- **La difficulté** appartient au monde.
- **Le mode de jeu** appartient à chaque joueur : /gamemode ne change que le tien. Les nouveaux joueurs commencent dans le mode de jeu du monde.
- **Dormir** : une nuit n’est passée que lorsque tous les joueurs en ligne ont utilisé un [[Bedroll|couchage]] cette nuit-là. Chaque joueur a son propre point de réapparition.
- **L’inventaire, la position, l’expérience et les succès** sont propres à chaque joueur, gardés par le monde sous son nom (ou son compte sur un serveur de hub).
- **Se battre contre d’autres joueurs** : les flèches, les carreaux d’arbalète et les haches lancées blessent les autres joueurs ; les coups de mêlée n’atteignent que les créatures.
- **Discussion** : appuie sur T, écris, puis appuie sur Entrée. Tous les joueurs en ligne la lisent ; la réponse à une commande ne va qu’à celui qui l’a tapée.

Chaque joueur choisit une distance d’affichage, de 1 à 64 chunks, dans Paramètres > Vidéo ou avec O et P. Un serveur peut en accorder moins, jusqu’à son --max-render-distance, et l’onglet Vidéo le signale.
