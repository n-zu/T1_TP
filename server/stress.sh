#!/bin/bash

# trap ctrl-c and call ctrl_c()
trap end INT

function end() {
    printf "\n" > /tmp/srv-input
    rm -f /tmp/srv-input
    git restore config.txt
    sleep 0.5
    exit 0
}

docker image load -i mqtt-stresser.tar
sed -i "s/^ip=.*/ip=$(hostname -I | grep -Eo '^[^ ]+' | sed 's/\./\\\./g')/" config.txt
rm -f /tmp/srv-input
mkfifo /tmp/srv-input
cargo build --release
printf "Ejecutando stress test con flags:\n\033[2;37m$SFLAGS\n"
cat /tmp/srv-input | cargo run --release > /dev/null &
docker run --rm inovex/mqtt-stresser -broker tcp://$(hostname -I | grep -Eo '^[^ ]+'):1883 \
    -username fdelu -password fdelu $SFLAGS
end