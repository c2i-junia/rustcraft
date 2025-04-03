remove-game-folders:
    rm -rf appdata

create-game-folders:
    mkdir -p appdata

    # client 1
    mkdir -p appdata/client-1 appdata/client-1/saves
    touch appdata/client-1/servers.ron

    # client 2
    mkdir -p appdata/client-2 appdata/client-2/saves
    touch appdata/client-2/servers.ron

    # server
    mkdir -p appdata/server appdata/server/saves

debug: create-game-folders
    cargo build --features=bevy/dynamic_linking

release: create-game-folders
    cargo build --release

generate-release-folder:
    cargo build --release

    # create folder
    mkdir -p release
    mkdir -p release release/saves release/data release/bin

    # copy paste data folder 
    cp -r data release/

    # add config files 
    touch release/servers.ron

    # add other files 
    cp CHANGELOG.txt release/
    cp LICENSE.txt release/

    # copy paste binaries 
    cp target/release/client release/bin/rustcraft
    cp target/release/server release/bin/rustcraft-server

generate-release-folder-server:
    cargo build --release --bin server

    # create folder
    mkdir -p release
    mkdir -p release release/bin

    # add other files 
    cp CHANGELOG.txt release/
    cp LICENSE.txt release/

    # copy paste binaries 
    cp target/release/server release/bin/rustcraft-server

